use rusb::UsbContext;

use super::bootloader_info::BootloaderInfo;
use super::context::Context;
use super::error::{Error, Result};
use super::flash::Page;
use super::operation::{Erase, Program, Read};
use super::target_handle::{crc32, get_serial, TargetHandle};

/// Contains necessary information to connect to a target via USB.
pub struct TargetInfo {
    /// USB bus ID the target is connected to.
    pub usb_bus_number: u8,

    /// USB device address of the target.
    pub usb_bus_address: u8,

    /// Serial number string the target reported via its USB descriptor.
    pub serial: String,
}

/// Contains a connected target and allows operations to be carried out.
pub struct Target {
    /// Handle for the low-level communication
    handle: TargetHandle<rusb::Context>,
}

impl<'a> TargetInfo {
    /// Connects to a target. Fails if the USB device is not a valid punt target.
    pub fn open(&self, context: &'a mut Context) -> Result<Target> {
        for device in context.usb_context.devices()?.iter() {
            if device.bus_number() == self.usb_bus_number
                && device.address() == self.usb_bus_address
            {
                // get_serial() fails if the device is unsupported. This check ensures that we don't
                // send commands to some entirely different device (e.g. if bus number and address
                // have been determined by something else than Context::find_targets() or there was
                // a reenumeration between its call and a call of open()).
                match get_serial(&device) {
                    Ok(ref serial) if serial == &self.serial => {
                        let handle = TargetHandle::from_usb_device(device)?;
                        return Ok(Target { handle });
                    }
                    Ok(_) => return Err(Error::TargetNotFound),
                    Err(e) => return Err(e),
                }
            }
        }
        Err(Error::TargetNotFound)
    }
}

impl<'a, 'd> Target {
    /// Queries bootloader information from the target.
    pub fn bootloader_info(&mut self) -> Result<BootloaderInfo> {
        self.handle.bootloader_info()
    }

    /// Queries a CRC32 from the target for a given memory area.
    pub fn read_crc(&mut self, address: u32, length: usize) -> Result<u32> {
        self.handle.read_crc(address, length as u32)
    }

    /// Verifies the supplied buffer against the target memory region beginning at the supplied
    /// address with a CRC32 check.
    pub fn verify(&mut self, data: &[u8], address: u32) -> Result<()> {
        let crc = self.handle.read_crc(address, data.len() as u32)?;
        if crc == crc32(data) {
            Ok(())
        } else {
            Err(Error::VerificationError)
        }
    }

    /// Erases a single flash page.
    pub fn erase_page(&mut self, page: Page) -> Result<()> {
        self.handle.erase_page(page)
    }

    /// Erases a number of pages.
    pub fn erase_pages(&mut self, pages: &[Page]) -> Erase<'_> {
        Erase::pages(&mut self.handle, pages)
    }

    /// Erases the minimum number of pages to ensure the supplied area is completely erased. This
    /// will, in general, erase a larger area due to the page-wise erase of the microcontroller's
    /// flash memory.
    pub fn erase_area(&mut self, start: u32, length: usize) -> Erase<'_> {
        Erase::area(&mut self.handle, start, length)
    }

    /// Programs a buffer's contents into the microcontroller's flash at the given start address.
    /// The flash area has to be erased for this operation to succeed.
    pub fn program_at(&mut self, data: &'d [u8], address: u32) -> Program<'d, '_> {
        Program::at(&mut self.handle, data, address)
    }

    /// Reads from the target's memory into a buffer.
    pub fn read_at(&mut self, buffer: &'d mut [u8], address: u32) -> Read<'d, '_> {
        Read::at(&mut self.handle, buffer, address)
    }

    /// Signals the target to exit its bootloader and start the application.
    pub fn exit_bootloader(&mut self) -> Result<()> {
        self.handle.exit_bootloader()
    }
}
