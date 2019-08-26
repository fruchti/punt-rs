pub const PAGE_SIZE: u32 = 1024;
pub const FLASH_BASE: u32 = 0x0800_0000;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Page(u8);

impl Page {
    pub fn from_index(index: u8) -> Self {
        Self(index)
    }

    pub fn from_address(address: u32) -> Self {
        Self(((address - FLASH_BASE) / PAGE_SIZE) as u8)
    }

    pub fn begin(&self) -> u32 {
        self.0 as u32 * PAGE_SIZE + FLASH_BASE
    }

    pub fn end(&self) -> u32 {
        (self.0 as u32 + 1) * PAGE_SIZE + FLASH_BASE - 1
    }
}

impl Into<u8> for Page {
    fn into(self) -> u8 {
        self.0
    }
}

impl Into<u8> for &Page {
    fn into(self) -> u8 {
        self.0
    }
}
