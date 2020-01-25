use super::error::{Error, Result};
use super::target::TargetInfo;
use super::target_handle::get_serial;
// use rusb::UsbContext as _;

/// Base trait for a USB context. This is a small wrapper around rusb::UsbContext with a few
/// convenience functions.
pub trait UsbContext: rusb::UsbContext {
    /// Returns information about all connected targets in bootloader mode. USB devices not in
    /// bootloader mode cannot be detected, since the ir protocol for entering bootloader mode is
    /// not specified.
    ///
    /// It returns [`Error::IoError`] on USB errors during device enumeration.
    ///
    /// [`Error::IoError`]: enum.Error.html#variant.IoError
    fn find_targets(&self) -> Result<Vec<TargetInfo>> {
        let mut targets = Vec::new();

        for device in self.devices()?.iter() {
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

    /// Returns one target if either
    ///
    /// * A serial number is supplied which matches one of the connected targets' serial numbers or
    /// * Only one target is connected and either no serial number is supplied or the serial number
    ///   matches.
    ///
    /// It can return the following errors:
    /// * [`Error::TargetNotFound`] if no target is found based on the criteria above,
    /// * [`Error::TooManyMatches`] if more than one target is connected but no serial number is
    ///   supplied, and
    /// * [`Error::IoError`] for any libusb errors occurring during USB transfers.
    ///
    /// Just like with [`find_targets`], only targets in bootloader mode are considered.
    ///
    /// [`find_targets`]: #method.find_targets
    /// [`Error::IoError`]: enum.Error.html#variant.IoError
    /// [`Error::TargetNotFound`]: enum.Error.html#variant.TargetNotFound
    /// [`Error::TooManyMatches`]: enum.Error.html#variant.TooManyMatches
    fn pick_target(&self, serial: Option<&str>) -> Result<TargetInfo> {
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

pub type Context = rusb::Context;

impl UsbContext for Context {}
