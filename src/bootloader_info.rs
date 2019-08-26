use std::fmt;

/// Suppository information read back from the bootloader.
#[derive(Debug)]
pub struct BootloaderInfo {
    /// Build number of the bootloader.
    pub build_number: u32,

    /// Build date of the bootloader, formatted as an ISO 8601 date (`YYYY-MM-DD`)
    pub build_date: String,

    /// Start address of the application flash.
    pub application_base: u32,

    /// Size of the flash available for the application (in bytes).
    pub application_size: usize,
}

impl fmt::Display for BootloaderInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Bootloader firmware build: {}", self.build_number)?;
        writeln!(f, "Bootloader firmware build date: {}", self.build_date)?;
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
