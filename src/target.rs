use crate::context::UsbContext;
use crate::error::{Error, Result};
use crate::target_handle::TargetHandle;
use crate::TIMEOUT;
use rusb::Device;
use std::convert::TryFrom;

/// Contains necessary information to connect to a target via USB.
pub struct Target<T: UsbContext> {
    /// USB device for the low-level communication
    usb_device: Device<T>,
}

impl<T: UsbContext> Target<T> {
    /// Returns the serial number string the target reports via its USB descriptor.
    pub fn serial(&self) -> Result<String> {
        let device_handle = self.usb_device.open()?;
        let device_desc = self.usb_device.device_descriptor()?;

        // Choose first language (the punt bootloader only supports English anyway)
        let language = device_handle.read_languages(TIMEOUT)?[0];

        Ok(device_handle.read_serial_number_string(language, &device_desc, TIMEOUT)?)
    }

    /// Connects to a target. Fails when errors occurr during USB communication.
    pub fn open(&self) -> Result<TargetHandle<T>> {
        // Fetch endpoint sizes
        let config_descriptor = self.usb_device.active_config_descriptor()?;
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

        // Open and reset device
        let mut device_handle = self.usb_device.open()?;
        device_handle.reset()?;

        Ok(TargetHandle {
            usb_device_handle: device_handle,
            in_buffer_length,
            out_buffer_length,
        })
    }
}

impl<T: UsbContext> TryFrom<rusb::Device<T>> for Target<T> {
    type Error = Error;

    /// Converts a raw USB device into a punt target if possible. If the USB device does not
    /// reference a punt target, this function returns [`Err(Error::UnsupportedTarget)`].
    ///
    /// [`Err(Error::UnsupportedTarget)`]: enum.Error.html#variant.UnsupportedTarget
    fn try_from(device: Device<T>) -> Result<Target<T>> {
        // Constants used to identify the device. The shared VID:PID pair used here
        // mandates a check for the manufacturer and product strings
        const VENDOR_STRING: &str = "25120";
        const PRODUCT_STRING: &str = "punt";
        const VENDOR_ID: u16 = 0x16c0;
        const PRODUCT_ID: u16 = 0x05dc;

        let device_desc = device.device_descriptor()?;

        if device_desc.vendor_id() != VENDOR_ID || device_desc.product_id() != PRODUCT_ID {
            return Err(Error::UnsupportedTarget);
        }

        let device_handle = device.open()?;

        // Choose first language (the punt bootloader only supports English anyway)
        let language = device_handle.read_languages(TIMEOUT)?[0];

        let vendor_string =
            device_handle.read_manufacturer_string(language, &device_desc, TIMEOUT)?;
        let product_string = device_handle.read_product_string(language, &device_desc, TIMEOUT)?;

        if vendor_string != VENDOR_STRING || product_string != PRODUCT_STRING {
            return Err(Error::UnsupportedTarget);
        }

        Ok(Target { usb_device: device })
    }
}
