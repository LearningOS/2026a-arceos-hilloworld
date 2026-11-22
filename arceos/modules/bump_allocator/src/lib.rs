#![no_std]

use allocator::{AllocError, AllocResult, BaseAllocator, ByteAllocator, PageAllocator};
use core::alloc::Layout;
use core::ptr::NonNull;

/// Early memory allocator
/// Use it before formal bytes-allocator and pages-allocator can work!
/// This is a double-end memory range:
/// - Alloc bytes forward
/// - Alloc pages backward
///
/// [ bytes-used | avail-area | pages-used ]
/// |            | -->    <-- |            |
/// start       b_pos        p_pos       end
///
/// For bytes area, 'count' records number of allocations.
/// When it goes down to ZERO, free bytes-used area.
/// For pages area, it will never be freed!
///
pub struct EarlyAllocator<const SIZE: usize> {
    start: usize,
    end: usize,
    b_pos: usize,
    p_pos: usize,
    count: usize,
}

impl<const SIZE: usize> EarlyAllocator<SIZE> {
    pub const fn new() -> Self {
        Self {
            start: 0,
            end: 0,
            b_pos: 0,
            p_pos: 0,
            count: 0,
        }
    }
}

#[inline]
const fn align_up(pos: usize, align: usize) -> usize {
    (pos + align - 1) & !(align - 1)
}

#[inline]
const fn align_down(pos: usize, align: usize) -> usize {
    pos & !(align - 1)
}

impl<const SIZE: usize> BaseAllocator for EarlyAllocator<SIZE> {
    fn init(&mut self, start: usize, size: usize) {
        self.start = start;
        self.end = start + size;
        self.b_pos = start;
        self.p_pos = align_down(self.end, SIZE);
        self.count = 0;
    }

    fn add_memory(&mut self, start: usize, size: usize) -> AllocResult {
        // A bump allocator can only be extended at the tail.
        if start != self.end {
            return Err(AllocError::NoMemory);
        }
        self.end += size;
        Ok(())
    }
}

impl<const SIZE: usize> ByteAllocator for EarlyAllocator<SIZE> {
    fn alloc(&mut self, layout: Layout) -> AllocResult<NonNull<u8>> {
        let start = align_up(self.b_pos, layout.align());
        let end = start + layout.size();
        if end > self.p_pos {
            return Err(AllocError::NoMemory);
        }
        self.b_pos = end;
        self.count += 1;
        NonNull::new(start as *mut u8).ok_or(AllocError::NoMemory)
    }

    fn dealloc(&mut self, _pos: NonNull<u8>, _layout: Layout) {
        // The bytes area is released as a whole once nothing is allocated.
        self.count = self.count.saturating_sub(1);
        if self.count == 0 {
            self.b_pos = self.start;
        }
    }

    fn total_bytes(&self) -> usize {
        self.end - self.start
    }

    fn used_bytes(&self) -> usize {
        (self.b_pos - self.start) + (self.end - self.p_pos)
    }

    fn available_bytes(&self) -> usize {
        self.p_pos - self.b_pos
    }
}

impl<const SIZE: usize> PageAllocator for EarlyAllocator<SIZE> {
    const PAGE_SIZE: usize = SIZE;

    fn alloc_pages(&mut self, num_pages: usize, align_pow2: usize) -> AllocResult<usize> {
        let size = num_pages * SIZE;
        let align = align_pow2.max(SIZE);
        if size > self.p_pos - self.b_pos {
            return Err(AllocError::NoMemory);
        }
        let start = align_down(self.p_pos - size, align);
        if start < self.b_pos {
            return Err(AllocError::NoMemory);
        }
        self.p_pos = start;
        Ok(start)
    }

    fn dealloc_pages(&mut self, _pos: usize, _num_pages: usize) {
        // Pages area is never freed.
    }

    fn total_pages(&self) -> usize {
        (self.end - self.start) / SIZE
    }

    fn used_pages(&self) -> usize {
        (self.end - self.p_pos) / SIZE
    }

    fn available_pages(&self) -> usize {
        (self.p_pos - self.b_pos) / SIZE
    }
}
