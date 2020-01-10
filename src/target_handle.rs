use super::error::{Error, Result};
use super::flash::Page;
use super::BootloaderInfo;
use crc_any::CRC;
use rusb::{Device, DeviceHandle, UsbContext};
use std::convert::TryInto;

use super::TIMEOUT;

/// Splits the first four bytes of a slice off and interpret them as a little-endian u32.
fn read_ne_u32(input: &mut &[u8]) -> u32 {
    let (int_bytes, rest) = input.split_at(std::mem::size_of::<u32>());
    *input = rest;
    u32::from_ne_bytes(int_bytes.try_into().unwrap())
}

/// Returns the serial number string of a USB device, if it is a supported target and
/// [`Err(Error::UnsupportedTarget)`] otherwise.
///
/// [`Err(Error::UnsupportedTarget)`]: enum.Error.html#variant.UnsupportedTarget
pub(crate) fn get_serial<T: UsbContext>(device: &Device<T>) -> Result<String> {
    // Constants used to identify the device. The shared VID:PID pair used here
    // mandates a check for the manufacturer and product strings
    const PRODUCT_STRING: &str = "Punt\u{0}";
    const VENDOR_STRING: &str = "\u{0}";
    const VENDOR_ID: u16 = 0x16c0;
    const PRODUCT_ID: u16 = 0x05dc;

    let device_desc = device.device_descriptor()?;

    if device_desc.vendor_id() == VENDOR_ID && device_desc.product_id() == PRODUCT_ID {
        let device_handle = device.open()?;

        // Choose first language (the punt bootloader only supports English anyway)
        let language = device_handle.read_languages(TIMEOUT)?[0];

        let vendor_string =
            device_handle.read_manufacturer_string(language, &device_desc, TIMEOUT)?;
        let product_string = device_handle.read_product_string(language, &device_desc, TIMEOUT)?;

        if vendor_string == VENDOR_STRING && product_string == PRODUCT_STRING {
            return device_handle
                .read_serial_number_string(language, &device_desc, TIMEOUT)
                .map(|serial| serial[..(serial.len() - 1)].to_owned()) // Remove zero termination
                .map_err(std::convert::Into::into);
        } else {
            return Err(Error::UnsupportedTarget);
        }
    }
    Err(Error::UnsupportedTarget)
}

/// Struct for the raw USB access to a punt target.
pub(crate) struct TargetHandle<T: UsbContext> {
    // USB device handle for the raw communication.
    usb_device_handle: DeviceHandle<T>,

    /// USB endpoint buffer size for the data in endpoint.
    in_buffer_length: u16,

    /// USB endpoint buffer size for the data out endpoint.
    out_buffer_length: u16,
}

impl<T: UsbContext> TargetHandle<T> {
    /// Creates a target handle from a USB device. Caution: Does not check if the USB device
    /// actually is a valid bootloader target.
    pub fn from_usb_device(device: Device<T>) -> Result<Self> {
        // Fetch endpoint sizes
        let config_descriptor = device.active_config_descriptor()?;
        let interface_descriptor = config_descriptor
            .interfaces()
            .next()
            .unwrap()
            .descriptors()
            .next()
            .unwrap();
        let mut endpoint_descriptors = interface_descriptor.endpoint_descriptors();
        let in_buffer_length = endpoint_descriptors.next().unwrap().max_packet_size();
        let out_buffer_length = endpoint_descriptors.next().unwrap().max_packet_size();

        let mut device_handle = device.open()?;
        device_handle.reset()?;

        Ok(Self {
            usb_device_handle: device_handle,
            in_buffer_length,
            out_buffer_length,
        })
    }

    /// Queries bootloader information from the target.
    pub fn bootloader_info(&mut self) -> Result<BootloaderInfo> {
        let mut info_packet = [0u8; 16];
        self.send_command(Command::BootloaderInfo, &[0; 0], &mut info_packet)?;

        let mut info_packet = &info_packet[..];
        let build_date = read_ne_u32(&mut info_packet);
        let build_number = read_ne_u32(&mut info_packet);
        let application_base = read_ne_u32(&mut info_packet);
        let application_size = read_ne_u32(&mut info_packet) as usize;

        // Convert raw date integer to legible representation
        let mut build_date = build_date.to_string();
        build_date.insert(6, '-');
        build_date.insert(4, '-');

        Ok(BootloaderInfo {
            build_number,
            build_date,
            application_base,
            application_size,
        })
    }

    /// Queries a CRC32 from the target for a given memory area.
    pub fn read_crc(&mut self, start: u32, length: u32) -> Result<u32> {
        let mut request_packet = vec![0u8; 8];
        request_packet[0..4].copy_from_slice(&start.to_le_bytes());
        request_packet[4..8].copy_from_slice(&length.to_le_bytes());
        let mut crc_packet = [0u8; 4];

        self.send_command(Command::ReadCrc, &request_packet, &mut crc_packet)?;

        let crc = u32::from_le_bytes(crc_packet);

        Ok(crc)
    }

    /// Returns the maximum size of a single chunk for a memory read operation (limited by the USB
    /// endpoint buffer size).
    pub fn max_read_chunk_size(&self) -> usize {
        self.in_buffer_length as usize
    }

    /// Reads a single chunk of memory, starting at the given address. The maximum chunk size can be
    /// queried with [`max_read_chunk_size`].
    ///
    /// [`max_read_chunk_size`]: #method.max_read_chunk_size
    pub fn read_chunk(&mut self, start: u32, buffer: &mut [u8]) -> Result<()> {
        let mut request_packet = vec![0u8; 8];
        request_packet[0..4].copy_from_slice(&start.to_le_bytes());
        request_packet[4..8].copy_from_slice(&(buffer.len() as u32).to_le_bytes());

        self.send_command(Command::ReadMemory, &request_packet, buffer)
    }

    /// Erases a single flash page.
    pub fn erase_page(&mut self, page: Page) -> Result<()> {
        let request_packet = [page.into()];
        let mut status_packet = [0u8];
        self.send_command(Command::ErasePage, &request_packet, &mut status_packet)?;
        // TODO: Add more fine-grained result code matching
        match status_packet[0] {
            0 => Ok(()),
            code => Err(Error::EraseError(code.into())),
        }
    }

    /// Returns the maximum size of a single chunk for a flash write operation (limited by the USB
    /// endpoint buffer size).
    pub fn max_program_chunk_size(&self) -> usize {
        // The packets written via USB include not only the payload, but also the start address. The
        // payload size is thus 4 bytes smaller than the available buffer.
        self.out_buffer_length as usize - 4
    }

    /// Programs a single chunk of memory into flash, starting at the given address. The flash has
    /// to be already erased for this operation to succeed. The maximum chunk size can be queried
    /// with [`max_program_chunk_size`].
    ///
    /// [`max_read_chunk_size`]: #method.max_program_chunk_size
    pub fn program_chunk(&mut self, start: u32, data: &[u8]) -> Result<()> {
        let mut address_packet = vec![0u8; 4];
        address_packet[0..4].copy_from_slice(&start.to_le_bytes());

        let mut packet = Vec::with_capacity(data.len() + 4);
        packet.extend(address_packet);
        packet.extend(data);
        self.send_command(Command::Program, &packet, &mut [0; 0])
    }

    /// Lets the target exit from the bootloader and start its application.
    pub fn exit_bootloader(&mut self) -> Result<()> {
        self.send_command(Command::Exit, &[0; 0], &mut [0; 0])
    }

    /// Sends a command to the target, optionally send data and optionally read data back.
    fn send_command(
        &mut self,
        cmd: Command,
        write_data: &[u8],
        read_data: &mut [u8],
    ) -> Result<()> {
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

        // If there is data to send, send it via bulk endpoint 2
        if !write_data.is_empty() {
            self.usb_device_handle
                .write_bulk(0x02, &write_data, TIMEOUT)?;
        }

        // If some bytes should be read back, read them from bulk endpoint 1
        if !read_data.is_empty() {
            self.usb_device_handle.read_bulk(0x81, read_data, TIMEOUT)?;
        }

        self.usb_device_handle.release_interface(0)?;
        Ok(())
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
