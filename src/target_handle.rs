use crate::bootloader_info::{BootloaderInfo, Version};
use crate::context::UsbContext;
use crate::error::{Error, Result};
use crate::flash::Page;
use crate::operation::{Erase, Program, Read};
use crate::TIMEOUT;
use crc_any::CRC;
use rusb::DeviceHandle;
use std::convert::TryInto;

/// Splits the first four bytes of a slice off and interpret them as a little-endian u32.
fn read_ne_u32(input: &mut &[u8]) -> u32 {
    let (int_bytes, rest) = input.split_at(std::mem::size_of::<u32>());
    *input = rest;
    u32::from_ne_bytes(int_bytes.try_into().unwrap())
}

/// Splits the first three bytes off a slice and interprets them as (major, minor, patch).
fn read_version(input: &mut &[u8]) -> Version {
    let (bytes, rest) = input.split_at(3);
    *input = rest;
    Version {
        major: bytes[0],
        minor: bytes[1],
        patch: bytes[2],
    }
}

/// Contains a connected target and allows operations to be carried out.
pub struct TargetHandle<T: UsbContext> {
    // USB device handle for the raw communication.
    pub(crate) usb_device_handle: DeviceHandle<T>,

    /// USB endpoint buffer size for the data in endpoint.
    pub(crate) in_buffer_length: u16,

    /// USB endpoint buffer size for the data out endpoint.
    pub(crate) out_buffer_length: u16,
}

impl<T: UsbContext> TargetHandle<T> {
    /// Queries bootloader information from the target.
    pub fn bootloader_info(&mut self) -> Result<BootloaderInfo> {
        use std::ffi::CString;

        let mut info_packet = [0u8; 64];
        let (_, packet_length) =
            self.send_command(Command::BootloaderInfo, &[0; 0], &mut info_packet)?;

        let mut info_packet = &info_packet[..packet_length];
        let build_date = read_ne_u32(&mut info_packet);
        let build_number = read_ne_u32(&mut info_packet);
        let application_base = read_ne_u32(&mut info_packet);
        let application_size = read_ne_u32(&mut info_packet) as usize;

        // Convert raw date integer to legible representation
        let mut build_date = build_date.to_string();
        build_date.insert(6, '-');
        build_date.insert(4, '-');

        let version = read_version(&mut info_packet);

        // Convert the remainder of the packet to a String
        let identifier = CString::new(info_packet)
            .map_err(|_| Error::MalformedResponse)?
            .into_string()
            .map_err(|_| Error::MalformedResponse)?;

        Ok(BootloaderInfo {
            build_number,
            build_date,
            application_base,
            application_size,
            version,
            identifier,
        })
    }

    /// Queries a CRC32 from the target for a given memory area.
    pub fn read_crc(&mut self, start: u32, length: usize) -> Result<u32> {
        let mut request_packet = vec![0u8; 8];
        request_packet[0..4].copy_from_slice(&start.to_le_bytes());
        request_packet[4..8].copy_from_slice(&(length as u32).to_le_bytes());
        let mut crc_packet = [0u8; 4];

        self.send_command(Command::ReadCrc, &request_packet, &mut crc_packet)?;

        let crc = u32::from_le_bytes(crc_packet);

        Ok(crc)
    }

    /// Verifies the supplied buffer against the target memory region beginning at the supplied
    /// address with a CRC32 check.
    pub fn verify(&mut self, data: &[u8], address: u32) -> Result<()> {
        let crc = self.read_crc(address, data.len())?;
        if crc == crc32(data) {
            Ok(())
        } else {
            Err(Error::VerificationError)
        }
    }

    /// Returns the maximum size of a single chunk for a memory read operation (limited by the USB
    /// endpoint buffer size).
    pub(crate) fn max_read_chunk_size(&self) -> usize {
        self.in_buffer_length as usize
    }

    /// Reads a single chunk of memory, starting at the given address. The maximum chunk size can be
    /// queried with [`max_read_chunk_size`].
    ///
    /// [`max_read_chunk_size`]: #method.max_read_chunk_size
    pub(crate) fn read_chunk(&mut self, start: u32, buffer: &mut [u8]) -> Result<()> {
        let mut request_packet = vec![0u8; 8];
        request_packet[0..4].copy_from_slice(&start.to_le_bytes());
        request_packet[4..8].copy_from_slice(&(buffer.len() as u32).to_le_bytes());

        self.send_command(Command::ReadMemory, &request_packet, buffer)
            .map(|_| ())
    }

    /// Erases a single flash page. Caution: The page index is unchecked.
    pub(crate) fn erase_page(&mut self, page: Page) -> Result<()> {
        let request_packet = [page.into()];
        let mut status_packet = [0u8];
        self.send_command(Command::ErasePage, &request_packet, &mut status_packet)?;
        // TODO: Add more fine-grained result code matching
        match status_packet[0] {
            0 => Ok(()),
            code => Err(Error::EraseError(code.into())),
        }
    }

    /// Erases a number of pages.
    pub fn erase_pages(&mut self, pages: &[Page]) -> Result<Erase<'_, T>> {
        let bootloader_info = self.bootloader_info()?;
        if pages
            .iter()
            .any(|page| !bootloader_info.application_pages().contains(&page))
        {
            return Err(Error::InvalidRequest);
        }

        Ok(Erase::pages(self, pages))
    }

    /// Erases the minimum number of pages to ensure the supplied area is completely erased. This
    /// will, in general, erase a larger area due to the page-wise erase of the microcontroller's
    /// flash memory.
    pub fn erase_area(&mut self, start: u32, length: usize) -> Result<Erase<'_, T>> {
        // Ensure that the requested area is fully within application flash
        let bootloader_info = self.bootloader_info()?;
        if (bootloader_info.application_base > start)
            || (bootloader_info.application_base as usize + bootloader_info.application_size
                < start as usize + length)
        {
            return Err(Error::InvalidRequest);
        }

        Ok(Erase::area(self, start, length))
    }

    /// Returns the maximum size of a single chunk for a flash write operation (limited by the USB
    /// endpoint buffer size).
    pub(crate) fn max_program_chunk_size(&self) -> usize {
        // The packets written via USB include not only the payload, but also the start address. The
        // payload size is thus 4 bytes smaller than the available buffer.
        self.out_buffer_length as usize - 4
    }

    /// Programs a single chunk of memory into flash, starting at the given address. The flash has
    /// to be already erased for this operation to succeed. The maximum chunk size can be queried
    /// with [`max_program_chunk_size`].
    ///
    /// [`max_read_chunk_size`]: #method.max_program_chunk_size
    pub(crate) fn program_chunk(&mut self, start: u32, data: &[u8]) -> Result<()> {
        let mut address_packet = vec![0u8; 4];
        address_packet[0..4].copy_from_slice(&start.to_le_bytes());

        let mut packet = Vec::with_capacity(data.len() + 4);
        packet.extend(address_packet);
        packet.extend(data);
        self.send_command(Command::Program, &packet, &mut [0; 0])
            .map(|_| ())
    }

    /// Programs a buffer's contents into the microcontroller's flash at the given start address.
    /// The flash area must have been erased already for this operation to succeed.
    pub fn program_at<'d>(&mut self, data: &'d [u8], address: u32) -> Result<Program<'d, '_, T>> {
        // Ensure that the area to be written to is fully within application flash
        let bootloader_info = self.bootloader_info()?;
        if (bootloader_info.application_base > address)
            || (bootloader_info.application_base as usize + bootloader_info.application_size
                < address as usize + data.len())
        {
            return Err(Error::InvalidRequest);
        }

        // Programing works halfword-wise and will crash if the address is not aligned
        if address % 2 != 0 {
            return Err(Error::InvalidRequest);
        }

        Ok(Program::at(self, data, address))
    }

    /// Reads from the target's memory into a buffer.
    pub fn read_at<'d>(&mut self, buffer: &'d mut [u8], address: u32) -> Result<Read<'d, '_, T>> {
        // Ensure that the requested area is fully within application flash
        let bootloader_info = self.bootloader_info()?;
        if (bootloader_info.application_base > address)
            || (bootloader_info.application_base as usize + bootloader_info.application_size
                < address as usize + buffer.len())
        {
            return Err(Error::InvalidRequest);
        }

        Ok(Read::at(self, buffer, address))
    }

    /// Lets the target exit from the bootloader and start its application.
    pub fn exit_bootloader(&mut self) -> Result<()> {
        self.send_command(Command::Exit, &[0; 0], &mut [0; 0])
            .map(|_| ())
    }

    /// Sends a command to the target, optionally send data and optionally read data back. Returns a
    /// tuple with the data length written and read.
    fn send_command(
        &mut self,
        cmd: Command,
        write_data: &[u8],
        read_data: &mut [u8],
    ) -> Result<(usize, usize)> {
        self.usb_device_handle.claim_interface(0)?;
        self.usb_device_handle.write_control(
            rusb::request_type(
                rusb::Direction::Out,
                rusb::RequestType::Vendor,
                rusb::Recipient::Device,
            ),
            cmd as u8,
            0,
            0,
            &[0u8; 0],
            TIMEOUT,
        )?;

        let mut written = 0;
        let mut read = 0;

        // If there is data to send, send it via bulk endpoint 2
        if !write_data.is_empty() {
            written = self
                .usb_device_handle
                .write_bulk(0x02, &write_data, TIMEOUT)?;
        }

        // If some bytes should be read back, read them from bulk endpoint 1
        if !read_data.is_empty() {
            read = self.usb_device_handle.read_bulk(0x81, read_data, TIMEOUT)?;
        }

        self.usb_device_handle.release_interface(0)?;
        Ok((written, read))
    }
}

/// Calculates a CRC32 checksum of a byte buffer in the way the punt target does it.
pub(crate) fn crc32(buff: &[u8]) -> u32 {
    let mut crc = CRC::crc32mpeg2();
    for bytes in buff.chunks(4) {
        let mut word = vec![0u8; 4];
        word[..bytes.len()].copy_from_slice(&bytes);
        word.reverse();
        crc.digest(&word);
    }
    crc.get_crc() as u32
}

/// Commands understood by the Punt bootloader. See `commands.h` in the C implementation of the
/// bootloader for further details about each command.
enum Command {
    BootloaderInfo = 0x01,
    ReadCrc = 0x02,
    ReadMemory = 0x03,
    ErasePage = 0x04,
    Program = 0x05,
    Exit = 0xff,
}
