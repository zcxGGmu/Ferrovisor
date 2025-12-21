//! Heap memory allocator
//!
//! Provides heap allocation for dynamic memory management in the hypervisor.

use crate::core::mm::{VirtAddr, align_up, PAGE_SIZE};
use crate::core::sync::SpinLock;
use core::alloc::{GlobalAlloc, Layout};
use core::ptr::NonNull;

/// Heap block header
#[repr(C)]
struct BlockHeader {
    /// Size of this block (including header)
    size: usize,
    /// Whether this block is in use
    in_use: bool,
    /// Previous block in the list
    prev: Option<NonNull<BlockHeader>>,
    /// Next block in the list
    next: Option<NonNull<BlockHeader>>,
}

/// Simple heap allocator
pub struct SimpleHeap {
    /// Start of the heap
    start_addr: VirtAddr,
    /// Current end of the heap
    end_addr: VirtAddr,
    /// Maximum size of the heap
    max_size: usize,
    /// List of free blocks
    free_list: SpinLock<Option<NonNull<BlockHeader>>>,
    /// Lock for heap operations
    lock: SpinLock<()>,
}

impl SimpleHeap {
    /// Create a new heap
    pub const fn new(start_addr: VirtAddr, max_size: usize) -> Self {
        Self {
            start_addr,
            end_addr: start_addr,
            max_size,
            free_list: SpinLock::new(None),
            lock: SpinLock::new(()),
        }
    }

    /// Initialize the heap
    pub unsafe fn init(&mut self) {
        self.end_addr = self.start_addr;
        *self.free_list.lock() = None;
    }

    /// Allocate a block of memory
    pub fn allocate(&self, layout: Layout) -> Result<NonNull<u8>, ()> {
        let _guard = self.lock.lock();

        // Align the size up
        let size = align_up(layout.size as u64) as usize;
        let total_size = size + core::mem::size_of::<BlockHeader>();

        // Try to find a suitable free block
        {
            let mut free_list = self.free_list.lock();
            let mut current = *free_list;
            let mut prev: Option<NonNull<BlockHeader>> = None;

            while let Some(block_ptr) = current {
                let block = unsafe { block_ptr.as_ref() };

                if !block.in_use && block.size >= total_size {
                    // Found a suitable block
                    if block.size > total_size + core::mem::size_of::<BlockHeader>() * 2 {
                        // Split the block
                        let new_block_ptr = unsafe {
                            NonNull::new_unchecked(
                                (block_ptr.as_ptr() as *mut u8)
                                    .add(total_size)
                                    as *mut BlockHeader
                            )
                        };
                        let new_block = unsafe { new_block_ptr.as_mut() };
                        new_block.size = block.size - total_size;
                        new_block.in_use = false;
                        new_block.prev = Some(block_ptr);
                        new_block.next = block.next;

                        if let Some(next) = block.next {
                            unsafe {
                                next.as_mut().prev = Some(new_block_ptr);
                            }
                        }

                        block.size = total_size;
                        block.next = Some(new_block_ptr);
                    }

                    // Mark block as in use
                    let block_mut = unsafe { block_ptr.as_mut() };
                    block_mut.in_use = true;

                    // Remove from free list
                    if let Some(prev_block) = prev {
                        unsafe {
                            prev_block.as_mut().next = block.next;
                        }
                    } else {
                        *free_list = block.next;
                    }
                    if let Some(next_block) = block.next {
                        unsafe {
                            next_block.as_mut().prev = prev;
                        }
                    }

                    // Return pointer to data
                    let data_ptr = unsafe {
                        NonNull::new_unchecked(
                            (block_ptr.as_ptr() as *mut u8)
                                .add(core::mem::size_of::<BlockHeader>())
                        )
                    };

                    return Ok(data_ptr);
                }

                prev = Some(block_ptr);
                current = block.next;
            }
        }

        // No suitable free block, need to allocate from heap end
        if self.end_addr + total_size as u64 > self.start_addr + self.max_size as u64 {
            return Err(()); // Out of memory
        }

        // Allocate new block at end
        let block_ptr = unsafe {
            NonNull::new_unchecked(self.end_addr as *mut BlockHeader)
        };
        let block = unsafe { block_ptr.as_mut() };
        block.size = total_size;
        block.in_use = true;
        block.prev = None;
        block.next = None;

        self.end_addr += total_size as u64;

        // Return pointer to data
        let data_ptr = unsafe {
            NonNull::new_unchecked(
                (block_ptr.as_ptr() as *mut u8)
                    .add(core::mem::size_of::<BlockHeader>())
            )
        };

        Ok(data_ptr)
    }

    /// Deallocate a block of memory
    pub fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        let _guard = self.lock.lock();

        // Get the block header
        let block_ptr = unsafe {
            NonNull::new_unchecked(
                (ptr.as_ptr() as *mut u8)
                    .sub(core::mem::size_of::<BlockHeader>())
                    as *mut BlockHeader
            )
        };

        // Add block to free list
        {
            let mut free_list = self.free_list.lock();
            let block = unsafe { block_ptr.as_mut() };
            block.in_use = false;

            // Try to coalesce with previous block if it's free
            if let Some(prev_ptr) = block.prev {
                let prev = unsafe { prev_ptr.as_ref() };
                if !prev.in_use {
                    // Coalesce with previous block
                    let prev_mut = unsafe { prev_ptr.as_mut() };
                    prev_mut.size += block.size;
                    prev_mut.next = block.next;

                    if let Some(next) = block.next {
                        unsafe {
                            next.as_mut().prev = Some(prev_ptr);
                        }
                    }

                    // Update block_ptr to point to the coalesced block
                    block_ptr = prev_ptr;
                }
            }

            // Try to coalesce with next block if it's free
            let current_block = unsafe { block_ptr.as_ref() };
            if let Some(next_ptr) = current_block.next {
                let next = unsafe { next_ptr.as_ref() };
                if !next.in_use {
                    // Coalesce with next block
                    let current_mut = unsafe { block_ptr.as_mut() };
                    current_mut.size += next.size;
                    current_mut.next = next.next;

                    if let Some(next_next) = next.next {
                        unsafe {
                            next_next.as_mut().prev = Some(block_ptr);
                        }
                    }
                }
            }

            // Insert into free list (sorted by address)
            let mut current = *free_list;
            let mut prev: Option<NonNull<BlockHeader>> = None;

            while let Some(curr) = current {
                if curr > block_ptr {
                    // Insert here
                    unsafe {
                        let block_mut = block_ptr.as_mut();
                        block_mut.prev = prev;
                        block_mut.next = Some(curr);

                        if let Some(prev_block) = prev {
                            prev_block.as_mut().next = Some(block_ptr);
                        } else {
                            *free_list = Some(block_ptr);
                        }

                        curr.as_mut().prev = Some(block_ptr);
                    }
                    return;
                }

                prev = current;
                current = unsafe { current.as_ref().next };
            }

            // Insert at end
            unsafe {
                let block_mut = block_ptr.as_mut();
                block_mut.prev = prev;
                block_mut.next = None;

                if let Some(prev_block) = prev {
                    prev_block.as_mut().next = Some(block_ptr);
                } else {
                    *free_list = Some(block_ptr);
                }
            }
        }
    }

    /// Reallocate a block
    pub fn reallocate(
        &self,
        ptr: Option<NonNull<u8>>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<u8>, ()> {
        if new_layout.size() == 0 {
            if let Some(p) = ptr {
                self.deallocate(p, old_layout);
            }
            return Ok(NonNull::dangling());
        }

        if let Some(ptr) = ptr {
            if new_layout.size() <= old_layout.size() {
                // Can reuse the same block
                return Ok(ptr);
            } else {
                // Need to allocate a new block and copy
                let new_ptr = self.allocate(new_layout)?;
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        ptr.as_ptr(),
                        new_ptr.as_ptr(),
                        old_layout.size(),
                    );
                    self.deallocate(ptr, old_layout);
                }
                Ok(new_ptr)
            }
        } else {
            self.allocate(new_layout)
        }
    }

    /// Get heap statistics
    pub fn stats(&self) -> HeapStats {
        let _guard = self.lock.lock();

        let mut total_free = 0;
        let mut free_blocks = 0;

        let free_list = self.free_list.lock();
        let mut current = *free_list;

        while let Some(block_ptr) = current {
            let block = unsafe { block_ptr.as_ref() };
            total_free += block.size;
            free_blocks += 1;
            current = block.next;
        }

        HeapStats {
            total_size: self.end_addr - self.start_addr,
            max_size: self.max_size,
            used_size: (self.end_addr - self.start_addr) - total_free,
            free_size: total_free,
            free_blocks,
        }
    }
}

/// Heap statistics
#[derive(Debug, Clone, Copy)]
pub struct HeapStats {
    /// Total size of the heap
    pub total_size: u64,
    /// Maximum size of the heap
    pub max_size: usize,
    /// Used size
    pub used_size: u64,
    /// Free size
    pub free_size: u64,
    /// Number of free blocks
    pub free_blocks: usize,
}

unsafe impl GlobalAlloc for SimpleHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.allocate(layout).map_or(core::ptr::null_mut(), |ptr| ptr.as_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if let Some(ptr) = NonNull::new(ptr) {
            self.deallocate(ptr, layout);
        }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_layout = unsafe {
            Layout::from_size_align_unchecked(
                new_size,
                layout.align(),
            )
        };

        self.reallocate(
            NonNull::new(ptr),
            layout,
            new_layout,
        ).map_or(core::ptr::null_mut(), |ptr| ptr.as_ptr())
    }
}

/// Global heap instance
#[cfg(feature = "allocator")]
static mut GLOBAL_HEAP: Option<SimpleHeap> = None;

/// Initialize the global heap allocator
#[cfg(feature = "allocator")]
pub unsafe fn init_global_heap(start_addr: VirtAddr, size: usize) {
    let mut heap = SimpleHeap::new(start_addr, size);
    heap.init();
    GLOBAL_HEAP = Some(heap);
}

/// Get the global heap allocator
#[cfg(feature = "allocator")]
pub fn get_global_heap() -> &'static SimpleHeap {
    unsafe { GLOBAL_HEAP.as_ref().unwrap() }
}

/// Set the global heap allocator
#[cfg(feature = "allocator")]
pub unsafe fn set_global_heap(heap: SimpleHeap) {
    GLOBAL_HEAP = Some(heap);
}

/// Global allocator implementation for use with #[global_allocator]
#[cfg(feature = "allocator")]
pub struct FerrovisorAllocator;

#[cfg(feature = "allocator")]
unsafe impl GlobalAlloc for FerrovisorAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        get_global_heap().alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        get_global_heap().dealloc(ptr, layout);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        get_global_heap().realloc(ptr, layout, new_size)
    }
}

/// Use the global allocator
#[cfg(feature = "allocator")]
#[global_allocator]
static ALLOCATOR: FerrovisorAllocator = FerrovisorAllocator;

/// Initialize the heap subsystem
pub fn init() -> Result<(), crate::Error> {
    // TODO: Set up the heap from available memory
    Err(crate::Error::NotImplemented)
}