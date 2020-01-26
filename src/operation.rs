use std::iter::Enumerate;
use std::slice::{Chunks, ChunksMut};

use crate::context::UsbContext;
use crate::error::Result;
use crate::flash::Page;
use crate::target_handle::TargetHandle;

/// General-purpose trait for operations which take multiple command transmissions via USB, e.g.
/// reading or writing a larger section of memory in smaller blocks.
///
/// # Examples
///
/// An `Operation` is an [`Iterator`] and thus evaluated lazily. This means it has, on the one hand,
/// to be executed explicitly for it to take effect:
///
/// ```rust, no_run
/// use punt::{Context, UsbContext, Operation};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Find a bootloader target
/// let mut context = Context::new()?;
/// let mut target = context.pick_target(None)?.open(&mut context)?;
///
/// // Create an erase Operation
/// let mut erase = target.erase_area(0x0800_0c00, 1024)?;
///
/// // Execute the erase and check its result
/// erase.execute()?;
/// # Ok(())
/// # }
/// ```
///
/// … but on the other hand, this can be used to have progress feedback from the operation
///
/// ```rust, no_run
/// use punt::{Context, UsbContext, Operation};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Find a bootloader target
/// let mut context = Context::new()?;
/// let mut target = context.pick_target(None)?.open(&mut context)?;
///
/// // Create an erase Operation
/// let mut erase = target.erase_area(0x0800_0c00, 1024)?;
///
/// let total = erase.total();
/// for status in erase {
///     println!("Successfully erased {} of {} pages.", status?, total);
/// }
/// # Ok(())
/// # }
/// ```
///
/// [`Iterator`]: https://doc.rust-lang.org/std/iter/trait.Iterator.html
pub trait Operation: Iterator<Item = Result<usize>> {
    /// Returns the total value in terms of which the progress is expressed.
    ///
    /// For example, for an erase operation it would be the total number of pages, while for a flash
    /// read it would be the total number of bytes to be read.
    fn total(&self) -> usize;

    /// Consumes the iterator to execute the operation. Returns on the first error to occur.
    fn execute(&mut self) -> Result<()> {
        if let Some(Err(error)) = self.last() {
            Err(error)
        } else {
            Ok(())
        }
    }
}

/// Page-wise flash erase operation
pub struct Erase<'a, T: UsbContext> {
    handle: &'a mut TargetHandle<T>,
    pages: Vec<Page>,
    count: usize,
    done: bool,
}

impl<T: UsbContext> Operation for Erase<'_, T> {
    fn total(&self) -> usize {
        self.count
    }
}

impl<T: UsbContext> Iterator for Erase<'_, T> {
    type Item = Result<usize>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let page = self.pages.pop().unwrap();

        // Return None one the next call to `next` if this was the last page
        if self.pages.is_empty() {
            self.done = true;
        }
        Some(match self.handle.erase_page(page) {
            Ok(()) => Ok(self.count - self.pages.len()),
            Err(error) => {
                // Ensure that the iterator is fused after an error occurs
                self.done = true;
                Err(error)
            }
        })
    }
}

impl<'a, T: UsbContext> Erase<'a, T> {
    /// Erase a set of given pages (not necessarily a continuous range).
    pub(crate) fn pages(handle: &'a mut TargetHandle<T>, pages: &[Page]) -> Self {
        Self {
            handle,
            done: pages.is_empty(),
            pages: Vec::from(pages),
            count: pages.len(),
        }
    }

    /// Erase all necessary pages so that the flash area specified by a start address and length is
    /// completely erased. Due to the page-wise erase, this might erase memory outside the given
    /// area.
    pub(crate) fn area(handle: &'a mut TargetHandle<T>, start: u32, length: usize) -> Self {
        let pages = if length == 0 {
            // No pages should be erased if the area is zero-length
            Vec::new()
        } else {
            let first_page = Page::from_address(start);
            let last_page = Page::from_address(start + length as u32 - 1);
            (first_page.into()..=last_page.into())
                .map(Page::from_index)
                .collect()
        };

        Self {
            handle,
            done: pages.is_empty(),
            count: pages.len(),
            pages,
        }
    }
}

/// Flash program operation.
pub struct Program<'d, 'a, T: UsbContext> {
    handle: &'a mut TargetHandle<T>,
    address: u32,
    chunks: Enumerate<Chunks<'d, u8>>,
    length: usize,
    chunk_size: usize,
    done: bool,
}

impl<T: UsbContext> Operation for Program<'_, '_, T> {
    fn total(&self) -> usize {
        self.length
    }
}

impl<T: UsbContext> Iterator for Program<'_, '_, T> {
    type Item = Result<usize>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        if let Some((i, chunk)) = self.chunks.next() {
            Some(
                match self
                    .handle
                    .program_chunk(self.address + (i * self.chunk_size) as u32, chunk)
                {
                    Ok(()) => Ok(i * self.chunk_size + chunk.len()),
                    Err(error) => {
                        self.done = true;
                        Err(error)
                    }
                },
            )
        } else {
            self.done = true;
            None
        }
    }
}

impl<'d, 'a, T: UsbContext> Program<'d, 'a, T> {
    /// Write to flash, starting at a given memory location. The memory has to be manually erased
    /// before starting a programming operation.
    pub(crate) fn at(handle: &'a mut TargetHandle<T>, data: &'d [u8], address: u32) -> Self {
        let chunk_size = handle.max_program_chunk_size();
        Self {
            handle,
            address,
            chunk_size,
            chunks: data.chunks(chunk_size).enumerate(),
            length: data.len(),
            done: data.is_empty(),
        }
    }
}

// Memory read operation.
pub struct Read<'d, 'a, T: UsbContext> {
    handle: &'a mut TargetHandle<T>,
    address: u32,
    chunks: Enumerate<ChunksMut<'d, u8>>,
    length: usize,
    chunk_size: usize,
    done: bool,
}

impl<T: UsbContext> Operation for Read<'_, '_, T> {
    fn total(&self) -> usize {
        self.length
    }
}

impl<T: UsbContext> Iterator for Read<'_, '_, T> {
    type Item = Result<usize>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        if let Some((i, chunk)) = self.chunks.next() {
            Some(
                match self
                    .handle
                    .read_chunk(self.address + (i * self.chunk_size) as u32, chunk)
                {
                    Ok(()) => Ok(i * self.chunk_size + chunk.len()),
                    Err(error) => {
                        self.done = true;
                        Err(error)
                    }
                },
            )
        } else {
            self.done = true;
            None
        }
    }
}

impl<'d, 'a, T: UsbContext> Read<'d, 'a, T> {
    /// Read from the microcontroller's memory to a buffer, starting at the supplied address.
    pub(crate) fn at(handle: &'a mut TargetHandle<T>, buffer: &'d mut [u8], address: u32) -> Self {
        let chunk_size = handle.max_read_chunk_size();
        Self {
            handle,
            address,
            chunk_size,
            length: buffer.len(),
            done: buffer.is_empty(),
            chunks: buffer.chunks_mut(chunk_size).enumerate(),
        }
    }
}
