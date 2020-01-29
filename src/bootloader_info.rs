//! Contains data structures for information the bootloader reports back to the connecting PC.

use crate::flash::Page;
use std::fmt;
use std::ops::RangeInclusive;

/// Suppository information read back from the bootloader.
#[derive(Debug)]
pub struct BootloaderInfo {
    /// Build number of the bootloader.
    pub build_number: u32,

    /// Build date of the bootloader, formatted as an ISO 8601 date (`YYYY-MM-DD`).
    pub build_date: String,

    /// Start address of the application flash.
    pub application_base: u32,

    /// Size of the flash available for the application (in bytes).
    pub application_size: usize,

    /// Bootloader firmware version.
    pub version: Version,

    /// Identifier string, usually containing the MCU MPN.
    pub identifier: String,
}

impl BootloaderInfo {
    /// Returns a range containing all application pages
    pub fn application_pages(&self) -> RangeInclusive<Page> {
        Page::from_address(self.application_base)
            ..=Page::from_address(self.application_base + self.application_size as u32 - 1)
    }
}

impl fmt::Display for BootloaderInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Firmware version: {}", self.version)?;
        writeln!(f, "Firmware build number: {}", self.build_number)?;
        writeln!(f, "Firmware build date: {}", self.build_date)?;
        writeln!(f, "Bootloader identifier: {}", self.identifier)?;
        writeln!(
            f,
            "Application flash base address: 0x{:08x}",
            self.application_base
        )?;
        writeln!(
            f,
            "Application flash size: {} KiB",
            self.application_size / 1024
        )
    }
}

/// Represents a version number as used for the bootloader firmware version.
#[derive(Debug)]
pub struct Version {
    /// The major version, incremented for breaking changes.
    pub major: u8,

    /// The minor versions, incremented when features are added in a backwards-compatible manner.
    pub minor: u8,

    /// The patch version, incremented for backwards-compatible bug fixes.
    pub patch: u8,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}
