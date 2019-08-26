extern crate crc_any;
extern crate rusb;

mod bootloader_info;
mod context;
mod error;
mod flash;
mod operation;
mod target;
mod target_handle;

pub use bootloader_info::BootloaderInfo;
pub use context::Context;
pub use error::{Error, Result};
pub use flash::{Page, FLASH_BASE, PAGE_SIZE};
pub use operation::Operation;
pub use target::{Target, TargetInfo};

/// Timeout for all usb transactions.
const TIMEOUT: std::time::Duration = std::time::Duration::from_millis(500);
