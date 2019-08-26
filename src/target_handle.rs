use super::error::{Error, Result};
use super::flash::Page;
use super::BootloaderInfo;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use crc_any::CRC;
use libusb::{Device, DeviceHandle};

use super::TIMEOUT;

pub fn get_serial(device: &libusb::Device) -> Result<String> {
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

pub struct TargetHandle<'a> {
    usb_device_handle: DeviceHandle<'a>,
    in_buffer_length: u16,
    out_buffer_length: u16,
}

impl<'a> TargetHandle<'a> {
    pub fn from_usb_device(device: Device<'a>) -> Result<Self> {
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
        device_handle.set_active_configuration(1)?;

        Ok(Self {
            usb_device_handle: device_handle,
            in_buffer_length,
            out_buffer_length,
        })
    }

    pub fn bootloader_info(&mut self) -> Result<BootloaderInfo> {
        let mut info_packet = [0u8; 16];
        self.send_command(Command::BootloaderInfo, &[0; 0], &mut info_packet)?;

        let build_number = (&info_packet[4..8]).read_u32::<LittleEndian>()?;
        let build_date = (&info_packet[0..4]).read_u32::<LittleEndian>()?;
        let application_base = (&info_packet[8..12]).read_u32::<LittleEndian>()?;
        let application_size = (&info_packet[12..16]).read_u32::<LittleEndian>()? as usize;

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

    pub fn read_crc(&mut self, start: u32, length: u32) -> Result<u32> {
        let mut request_packet = [0u8; 8];
        (&mut request_packet[0..4]).write_u32::<LittleEndian>(start)?;
        (&mut request_packet[4..8]).write_u32::<LittleEndian>(length)?;
        let mut crc_packet = [0u8; 4];

        self.send_command(Command::ReadCrc, &request_packet, &mut crc_packet)?;

        let crc = (&crc_packet[0..4]).read_u32::<LittleEndian>()?;

        Ok(crc)
    }

    pub fn crc32(buff: &[u8]) -> u32 {
        let mut crc = CRC::crc32mpeg2();
        for bytes in buff.chunks(4) {
            let mut word = vec![0u8; 4];
            word[..bytes.len()].copy_from_slice(&bytes);
            word.reverse();
            crc.digest(&word);
        }
        crc.get_crc() as u32
    }

    pub fn max_read_chunk_size(&self) -> usize {
        self.in_buffer_length as usize
    }

    pub fn read_chunk(&mut self, start: u32, buffer: &mut [u8]) -> Result<()> {
        let mut request_packet = [0u8; 8];
        (&mut request_packet[0..4]).write_u32::<LittleEndian>(start)?;
        (&mut request_packet[4..8]).write_u32::<LittleEndian>(buffer.len() as u32)?;

        self.send_command(Command::ReadMemory, &request_packet, buffer)
    }

    pub fn erase_page(&mut self, page: Page) -> Result<()> {
        let request_packet = [page.into()];
        let mut status_packet = [0u8];
        self.send_command(Command::ErasePage, &request_packet, &mut status_packet)?;
        // TODO: Add more fine-grained result code matching
        match status_packet[0] {
            0 => Ok(()),
            code => Err(Error::EraseError(code)),
        }
    }

    pub fn max_program_chunk_size(&self) -> usize {
        // The packets written via USB include not only the payload, but also the start address. The
        // payload size is thus 4 bytes smaller than the available buffer.
        self.out_buffer_length as usize - 4
    }

    pub fn program_chunk(&mut self, start: u32, data: &[u8]) -> Result<()> {
        let mut address_packet = vec![0u8; 4];
        (&mut address_packet[0..4]).write_u32::<LittleEndian>(start)?;

        let mut packet = Vec::with_capacity(data.len() + 4);
        packet.extend(address_packet);
        packet.extend(data);
        self.send_command(Command::Program, &packet, &mut [0; 0])
    }

    // pub fn program_at<'d, 'r, T>(
    //     &'d mut self,
    //     data: &'d [u8],
    //     start: u32,
    // ) -> impl Iterator<Item = Box<dyn Transaction + '_>> + '_
    // where
    //     'a: 'r,
    //     'd: 'r
    // {
    //     std::iter::repeat_with(|| 0).enumerate().map(move |(i, target)| {
    //         let t: Box<dyn Transaction> = match i {
    //             0 => {
    //                 let first_page = ((start - FLASH_BASE) / PAGE_SIZE) as u8;
    //                 let last_page = ((start + data.len() as u32 - FLASH_BASE - 1) / PAGE_SIZE) as u8;
    //                 let pages: Vec<u8> = (first_page..=last_page).collect();
    //                 Box::new(ErasePages::new(self, &pages))
    //             },
    //             1 => {
    //                 Box::new(WriteFlash::new(self, data, start))
    //             }
    //             _ => unreachable!()
    //         };
    //         t
    //     }).take(3)
    // }

    // pub fn program_at<'b, T>(
    //     &'b mut self,
    //     data: &'b T,
    //     start: u32,
    // ) -> impl Iterator<Item = Result<ProgramProgress>> + 'b + Captures<'a>
    // where
    //     T: ?Sized + AsRef<[u8]>,
    // {
    //     let length = data.as_ref().len();
    //     let chunk_size = self.max_program_chunk_size();

    //     let first_page = (start - FLASH_BASE) / PAGE_SIZE;
    //     let last_page = (start + length as u32 - FLASH_BASE) / PAGE_SIZE;
    //     let pages = last_page - first_page + 1;

    //     let crc = Self::crc32(data.as_ref());

    //     enum Operation<'a> {
    //         ErasePage(u8),
    //         Flash { address: u32, data: &'a [u8] },
    //         VerifyCrc,
    //     }

    //     let erase = (first_page..=last_page).map(|page| Operation::ErasePage(page as u8));
    //     let write = data
    //         .as_ref()
    //         .chunks(chunk_size)
    //         .enumerate()
    //         .map(move |(i, bytes)| Operation::Flash {
    //             address: start + (i * chunk_size) as u32,
    //             data: bytes,
    //         });
    //     let verify = std::iter::once(Operation::VerifyCrc);

    //     let status_iter = erase.chain(write).chain(verify).map(move |op| match op {
    //         Operation::ErasePage(page) => {
    //             self.erase_page(page).map(|()| ProgramProgress::Erasing {
    //                 erased: page as usize - first_page as usize + 1,
    //                 total: pages as usize,
    //             })
    //         }
    //         Operation::Flash { address, data } => {
    //             self.program_chunk(address, data)
    //                 .map(|()| ProgramProgress::Programming {
    //                     transferred: (address - start) as usize + data.len(),
    //                     total: length,
    //                 })
    //         }
    //         Operation::VerifyCrc => {
    //             let target_crc = self.read_crc(start, length as u32)?;
    //             if target_crc == crc {
    //                 Ok(ProgramProgress::Done)
    //             } else {
    //                 Err(Error::VerificationError)
    //             }
    //         }
    //     });

    //     // Stop on first error
    //     status_iter.scan(false, |error_occurred, status| {
    //         if *error_occurred {
    //             None
    //         } else {
    //             *error_occurred = status.is_err();
    //             Some(status)
    //         }
    //     })
    // }

    pub fn exit_bootloader(&mut self) -> Result<()> {
        self.send_command(Command::Exit, &[0; 0], &mut [0; 0])
    }

    fn send_command(
        &mut self,
        cmd: Command,
        write_data: &[u8],
        read_data: &mut [u8],
    ) -> Result<()> {
        self.usb_device_handle.claim_interface(0)?;
        self.usb_device_handle.write_control(
            libusb::request_type(
                libusb::Direction::Out,
                libusb::RequestType::Vendor,
                libusb::Recipient::Device,
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
