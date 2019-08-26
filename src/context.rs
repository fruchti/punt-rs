use super::error::{Error, Result};
use super::target::TargetInfo;
use super::target_handle::get_serial;

pub struct Context {
    pub usb_context: libusb::Context,
}

impl Context {
    pub fn new() -> Result<Self> {
        let usb_context = libusb::Context::new()?;
        // usb_context.set_log_level(libusb::LogLevel::Debug);
        Ok(Context { usb_context })
    }

    pub fn find_targets(&mut self) -> Result<Vec<TargetInfo>> {
        let mut targets = Vec::new();

        for device in self.usb_context.devices()?.iter() {
            if let Ok(serial) = get_serial(&device) {
                targets.push(TargetInfo {
                    serial,
                    usb_bus_number: device.bus_number(),
                    usb_bus_address: device.address(),
                });
            }
        }

        Ok(targets)
    }

    pub fn pick_target(&mut self, serial: Option<&str>) -> Result<TargetInfo> {
        let targets = self.find_targets()?;
        if targets.is_empty() {
            Err(Error::TargetNotFound)
        } else if let Some(serial) = serial {
            if let Some(target) = targets.into_iter().find(|i| i.serial == serial) {
                Ok(target)
            } else {
                Err(Error::TargetNotFound)
            }
        } else if targets.len() == 1 {
            Ok(targets.into_iter().next().unwrap())
        } else {
            // More than one target and no serial given
            Err(Error::TooManyMatches)
        }
    }
}
