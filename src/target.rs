use super::bootloader_info::BootloaderInfo;
use super::context::Context;
use super::error::{Error, Result};
use super::flash::Page;
use super::operation::{Erase, Program, Read};
use super::target_handle::{get_serial, TargetHandle};

pub struct TargetInfo {
    pub usb_bus_number: u8,
    pub usb_bus_address: u8,
    pub serial: String,
}

pub struct Target<'a> {
    handle: TargetHandle<'a>,
}

impl<'a> TargetInfo {
    pub fn open(&self, context: &'a mut Context) -> Result<Target<'a>> {
        for device in context.usb_context.devices()?.iter() {
            if device.bus_number() == self.usb_bus_number
                && device.address() == self.usb_bus_address
            {
                // get_serial() fails if the device is unsupported. This check ensures that we don't
                // send commands to some entirely different device (e.g. if bus number and address
                // have been determined by something else than Context::find_targets() or there was
                // a reenumeration between its call and a call of open()).
                if get_serial(&device).is_ok() {
                    let handle = TargetHandle::from_usb_device(device)?;

                    return Ok(Target { handle });
                } else {
                    return Err(Error::UnsupportedTarget);
                }
            }
        }
        Err(Error::TargetNotFound)
    }
}

impl<'a, 'd> Target<'a> {
    pub fn bootloader_info(&mut self) -> Result<BootloaderInfo> {
        self.handle.bootloader_info()
    }

    pub fn erase_page(&mut self, page: Page) -> Result<()> {
        self.handle.erase_page(page)
    }

    pub fn read_crc(&mut self, address: u32, length: usize) -> Result<u32> {
        self.handle.read_crc(address, length as u32)
    }

    pub fn verify(&mut self, data: &[u8], address: u32) -> Result<()> {
        let crc = self.handle.read_crc(address, data.len() as u32)?;
        if crc == TargetHandle::crc32(data) {
            Ok(())
        } else {
            Err(Error::VerificationError)
        }
    }

    pub fn erase_pages(&mut self, pages: &[Page]) -> Erase<'a, '_> {
        Erase::pages(&mut self.handle, pages)
    }

    pub fn erase_area(&mut self, start: u32, length: usize) -> Erase<'a, '_> {
        Erase::area(&mut self.handle, start, length)
    }

    pub fn program_at(&mut self, data: &'d [u8], address: u32) -> Program<'a, 'd, '_> {
        Program::at(&mut self.handle, data, address)
    }

    pub fn read_at(&mut self, buffer: &'d mut [u8], address: u32) -> Read<'a, 'd, '_> {
        Read::at(&mut self.handle, buffer, address)
    }

    pub fn exit_bootloader(&mut self) -> Result<()> {
        self.handle.exit_bootloader()
    }
}
