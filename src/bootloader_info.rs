use std::fmt;

#[derive(Debug)]
pub struct BootloaderInfo {
    pub build_number: u32,
    pub build_date: String,
    pub application_base: u32,
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
