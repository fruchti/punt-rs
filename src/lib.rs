//! This crate provides a way to interact with a microcontroller with the punt bootloader connected
//! via USB and exposes all bootloader functions.
//!
//! # Example: Basic flashing
//! ```rust, no_run
//! use punt::{Context, UsbContext, Operation};
//! use std::fs::File;
//! use std::io::{Read, Write};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Open binary file and read contents
//! let mut file = File::open("test.bin")?;
//! let mut buff = Vec::new();
//! file.read_to_end(&mut buff)?;
//!
//! // Find a bootloader target
//! let mut context = Context::new()?;
//! let mut target_handle = context.pick_target(None)?.open()?;
//!
//! // Fetch information about the target's bootloader
//! let start_address = target_handle.bootloader_info()?.application_base;
//!
//! // Erase the necessary flash area
//! target_handle.erase_area(start_address, buff.len())?.execute()?;
//!
//! // Program the buffer into flash
//! target_handle.program_at(buff.as_slice(), start_address)?.execute()?;
//!
//! // Verify flash contents
//! target_handle.verify(buff.as_slice(), start_address)?;
//!
//! println!("Done!");
//! # Ok(())
//! # }
//! ```
//!
//! In addition to this very basic API, it also provides functionality for progress feedback during
//! operations like reading, erasing and flashing. See the [`Operation`] trait for details.
//!
//! [`Operation`]: trait.Operation.html

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
pub use context::{Context, UsbContext};
pub use error::{Error, Result};
pub use flash::{Page, FLASH_BASE, PAGE_SIZE};
pub use operation::Operation;
pub use target::Target;
pub use target_handle::TargetHandle;

/// Timeout for all usb transactions.
const TIMEOUT: std::time::Duration = std::time::Duration::from_millis(500);
