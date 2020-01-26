use crate::error::{Error, Result};
use crate::target::Target;
use std::convert::TryFrom;

/// Base trait for a USB context.
pub trait UsbContext: rusb::UsbContext {
    /// Returns information about all connected targets in bootloader mode. USB devices not in
    /// bootloader mode cannot be detected, since their protocol for entering bootloader mode is
    /// not specified.
    ///
    /// It returns [`Error::IoError`] on USB errors during device enumeration.
    ///
    /// [`Error::IoError`]: enum.Error.html#variant.IoError
    fn find_targets(&self) -> Result<Vec<Target<Self>>> {
        Ok(self
            .devices()?
            .iter()
            // try_from() will return Err(UnsupportedDevice) if the USB device is not a punt target
            .filter_map(|d| Target::try_from(d).ok())
            .collect())
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
    fn pick_target(&self, serial: Option<&str>) -> Result<Target<Self>> {
        let targets = self.find_targets()?;
        if let Some(serial) = serial {
            targets
                .into_iter()
                .find_map(|t| match t.serial() {
                    Ok(s) if s == serial => Some(Ok(t)),
                    Err(e) => Some(Err(e)),
                    _ => None,
                })
                .unwrap_or(Err(Error::TargetNotFound))
        } else if targets.len() > 1 {
            // More than one target and no serial given
            Err(Error::TooManyMatches)
        } else {
            // One or zero targets found and no serial given. Return first one if existant.
            targets.into_iter().next().ok_or(Error::TargetNotFound)
        }
    }
}

/// A punt context, necessary for USB communication.
pub type Context = rusb::Context;

impl UsbContext for Context {}
