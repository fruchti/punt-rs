//! This crate provides a way to interact with a microcontroller with the punt bootloader connected
//! via USB and exposes all bootloader functions.
//!
//! # Example: Basic flashing
//! ```rust, no_run
//! use punt::{Context, Operation};
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
//! let mut target = context.pick_target(None)?.open(&mut context)?;
//!
//! // Fetch information about the target's bootloader
//! let start_address = target.bootloader_info.application_base;
//!
//! // Erase the necessary flash area
//! target.erase_area(start_address, buff.len())?.execute()?;
//!
//! // Program the buffer into flash
//! target.program_at(buff.as_slice(), start_address)?.execute()?;
//!
//! // Verify flash contents
//! target.verify(buff.as_slice(), start_address)?;
//!
//! println!("Done!");
//! # Ok(())
//! # }
//! ```

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
