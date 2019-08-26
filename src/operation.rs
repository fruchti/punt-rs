use std::iter::Enumerate;
use std::slice::{Chunks, ChunksMut};

use super::error::Result;
use super::flash::Page;
use super::target_handle::TargetHandle;

pub trait Operation: Iterator<Item = Result<usize>> {
    fn total(&self) -> usize;

    fn execute(&mut self) -> Result<()> {
        if let Some(Err(error)) = self.last() {
            Err(error)
        } else {
            Ok(())
        }
    }
}

pub struct Erase<'h, 'a> {
    handle: &'a mut TargetHandle<'h>,
    pages: Vec<Page>,
    count: usize,
    done: bool,
}

impl Operation for Erase<'_, '_> {
    fn total(&self) -> usize {
        self.count
    }
}

impl Iterator for Erase<'_, '_> {
    type Item = Result<(usize)>;

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

impl<'h, 'a> Erase<'h, 'a> {
    pub fn pages(handle: &'a mut TargetHandle<'h>, pages: &[Page]) -> Self {
        Self {
            handle,
            done: pages.is_empty(),
            pages: Vec::from(pages),
            count: pages.len(),
        }
    }

    pub fn area(handle: &'a mut TargetHandle<'h>, start: u32, length: usize) -> Self {
        let pages = if length == 0 {
            // No pages should be erased if the area is zero-length
            Vec::new()
        } else {
            let first_page = Page::from_address(start);
            let last_page = Page::from_address(start + length as u32 - 1);
            (first_page.into()..=last_page.into())
                .into_iter()
                .map(|num| Page::from_index(num))
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

pub struct Program<'h, 'd, 'a> {
    handle: &'a mut TargetHandle<'h>,
    address: u32,
    chunks: Enumerate<Chunks<'d, u8>>,
    length: usize,
    chunk_size: usize,
    done: bool,
}

impl Operation for Program<'_, '_, '_> {
    fn total(&self) -> usize {
        self.length
    }
}

impl Iterator for Program<'_, '_, '_> {
    type Item = Result<(usize)>;

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

impl<'h, 'd, 'a> Program<'h, 'd, 'a> {
    pub fn at(handle: &'a mut TargetHandle<'h>, data: &'d [u8], address: u32) -> Self {
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

pub struct Read<'h, 'd, 'a> {
    handle: &'a mut TargetHandle<'h>,
    address: u32,
    chunks: Enumerate<ChunksMut<'d, u8>>,
    length: usize,
    chunk_size: usize,
    done: bool,
}

impl Operation for Read<'_, '_, '_> {
    fn total(&self) -> usize {
        self.length
    }
}

impl Iterator for Read<'_, '_, '_> {
    type Item = Result<(usize)>;

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

impl<'h, 'd, 'a> Read<'h, 'd, 'a> {
    pub fn at(handle: &'a mut TargetHandle<'h>, buffer: &'d mut [u8], address: u32) -> Self {
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
