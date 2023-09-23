/// Address of the first byte in the target microcontroller's flash.
pub const FLASH_BASE: u32 = 0x0800_0000;

/// Flash page size of the target microcontroller.
pub const PAGE_SIZE: u32 = 1024;

/// A page in the punt microcontroller's flash memory.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Page(u8);

impl Page {
    /// Creates a page from its index, starting at 0 at [`FLASH_BASE`], with each page being
    /// [`PAGE_SIZE`] bytes.
    ///
    /// [`FLASH_BASE`]: constant.FLASH_BASE.html
    /// [`PAGE_SIZE`]: constant.PAGE_SIZE.html
    pub fn from_index(index: u8) -> Self {
        Self(index)
    }

    /// Refers to the page containing the given address.
    pub fn from_address(address: u32) -> Self {
        Self(((address - FLASH_BASE) / PAGE_SIZE) as u8)
    }

    /// The first address in a page.
    /// # Examples
    ///
    /// ```rust
    /// use punt::FLASH_BASE;
    /// # use punt::Page;
    ///
    /// let page = Page::from_index(0);
    /// assert_eq!(page.begin(), FLASH_BASE);
    /// ```
    pub fn begin(&self) -> u32 {
        u32::from(self.0) * PAGE_SIZE + FLASH_BASE
    }

    /// Returns the address of the last byte of a page.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use punt::{FLASH_BASE, PAGE_SIZE};
    /// # use punt::Page;
    ///
    /// let page = Page::from_index(0);
    /// let end = page.end();
    /// assert_eq!(end, FLASH_BASE + PAGE_SIZE - 1);
    /// let next_page = Page::from_address(end + 1);
    /// let next_page_index: u8 = next_page.into();
    /// assert_eq!(next_page_index, 1);
    /// ```
    pub fn end(&self) -> u32 {
        (u32::from(self.0) + 1) * PAGE_SIZE + FLASH_BASE - 1
    }
}

impl From<Page> for u8 {
    fn from(val: Page) -> Self {
        val.0
    }
}

impl From<&Page> for u8 {
    fn from(val: &Page) -> Self {
        val.0
    }
}
