//! Buddy allocator implementation
//!
//! Based on xvisor's buddy allocator system. Provides efficient memory management
//! for power-of-2 sized allocations with automatic coalescing of free blocks.
//!
//! Features:
//! - Binary buddy system for efficient memory allocation
//! - Automatic block coalescing to reduce fragmentation
//! - Multiple order sizes (2^n * PAGE_SIZE)
//! - Memory usage statistics and monitoring
//! - Integration with frame allocator for backing memory

use crate::core::mm::{PAGE_SIZE, align_up, frame::alloc_frame, frame::dealloc_frame};
use crate::utils::spinlock::SpinLock;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};

/// Maximum order supported by buddy allocator (2^MAX_ORDER * PAGE_SIZE)
pub const MAX_ORDER: usize = 11; // Up to 8MB allocations

/// Buddy allocator errors
#[derive(Debug, Clone, PartialEq)]
pub enum BuddyError {
    /// Invalid order
    InvalidOrder,
    /// Out of memory
    OutOfMemory,
    /// Invalid address
    InvalidAddress,
    /// Invalid size
    InvalidSize,
    /// Not a power of 2
    NotPowerOfTwo,
}

/// Buddy block header
#[repr(C)]
struct BuddyBlock {
    /// Order of this block (log2 of size in pages)
    order: u8,
    /// Magic number for validation
    magic: u32,
    /// Whether this block is free
    free: bool,
    /// Next block in free list
    next: Option<NonNull<BuddyBlock>>,
    /// Previous block in free list
    prev: Option<NonNull<BuddyBlock>>,
}

impl BuddyBlock {
    const MAGIC: u32 = 0xB0090C47; // "BUDDY" with some bits

    /// Create a new buddy block
    fn new(order: u8, free: bool) -> Self {
        Self {
            order,
            magic: Self::MAGIC,
            free,
            next: None,
            prev: None,
        }
    }

    /// Get the size of this block in bytes
    fn size(&self) -> usize {
        (1 << self.order) * PAGE_SIZE
    }

    /// Get the address of this block
    fn addr(&self) -> usize {
        self as *const Self as usize
    }

    /// Get the buddy address for this block
    fn buddy_addr(&self, base_addr: usize) -> usize {
        let block_size = self.size();
        let offset = self.addr() - base_addr;
        base_addr + (offset ^ block_size)
    }

    /// Validate the magic number
    fn is_valid(&self) -> bool {
        self.magic == Self::MAGIC
    }
}

/// Free list for a specific order
struct FreeList {
    /// Head of the free list
    head: Option<NonNull<BuddyBlock>>,
    /// Count of blocks in this list
    count: usize,
}

impl FreeList {
    /// Create a new empty free list
    const fn new() -> Self {
        Self {
            head: None,
            count: 0,
        }
    }

    /// Add a block to the front of the free list
    fn push_front(&mut self, block: *mut BuddyBlock) {
        unsafe {
            let block_mut = &mut *block;
            block_mut.next = self.head;
            block_mut.prev = None;

            if let Some(head) = self.head {
                head.as_mut().prev = Some(NonNull::new_unchecked(block));
            }

            self.head = Some(NonNull::new_unchecked(block));
            self.count += 1;
        }
    }

    /// Remove a block from the front of the free list
    fn pop_front(&mut self) -> Option<*mut BuddyBlock> {
        self.head.map(|head| {
            unsafe {
                let head_mut = head.as_mut();
                self.head = head_mut.next;

                if let Some(next) = head_mut.next {
                    next.as_mut().prev = None;
                }

                head_mut.next = None;
                head_mut.prev = None;
                self.count -= 1;
            }
            head.as_ptr()
        })
    }

    /// Remove a specific block from the free list
    fn remove(&mut self, block: *mut BuddyBlock) -> bool {
        unsafe {
            let block_ref = &*block;

            if let Some(prev) = block_ref.prev {
                prev.as_mut().next = block_ref.next;
            } else {
                // Block is at head
                if let Some(head) = self.head {
                    if head.as_ptr() == block {
                        self.head = block_ref.next;
                    } else {
                        return false;
                    }
                } else {
                    return false;
                }
            }

            if let Some(next) = block_ref.next {
                next.as_mut().prev = block_ref.prev;
            }

            let block_mut = &mut *block;
            block_mut.next = None;
            block_mut.prev = None;
            self.count -= 1;
        }
        true
    }

    /// Check if the list is empty
    fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    /// Get the number of blocks in the list
    fn len(&self) -> usize {
        self.count
    }
}

/// Buddy allocator instance
pub struct BuddyAllocator {
    /// Base address of managed memory
    base_addr: usize,
    /// Total size of managed memory
    total_size: usize,
    /// Free lists for each order
    free_lists: [SpinLock<FreeList>; MAX_ORDER + 1],
    /// Total allocated blocks
    total_allocated: AtomicUsize,
    /// Total free blocks
    total_free: AtomicUsize,
    /// Number of allocations performed
    allocation_count: AtomicUsize,
    /// Number of deallocations performed
    deallocation_count: AtomicUsize,
}

/// Buddy allocator statistics
#[derive(Debug, Clone)]
pub struct BuddyStats {
    /// Total memory managed
    pub total_memory: usize,
    /// Total allocated memory
    pub allocated_memory: usize,
    /// Total free memory
    pub free_memory: usize,
    /// Number of free blocks per order
    pub free_blocks_per_order: [usize; MAX_ORDER + 1],
    /// Total number of allocations
    pub allocation_count: usize,
    /// Total number of deallocations
    pub deallocation_count: usize,
    /// Memory fragmentation ratio (0.0 to 1.0)
    pub fragmentation_ratio: f64,
}

impl BuddyAllocator {
    /// Create a new buddy allocator
    pub fn new(base_addr: usize, total_size: usize) -> Result<Self, BuddyError> {
        if total_size == 0 || !total_size.is_power_of_two() {
            return Err(BuddyError::InvalidSize);
        }

        if base_addr % PAGE_SIZE != 0 {
            return Err(BuddyError::InvalidAddress);
        }

        let mut allocator = Self {
            base_addr,
            total_size,
            free_lists: [SpinLock::new(FreeList::new()); MAX_ORDER + 1],
            total_allocated: AtomicUsize::new(0),
            total_free: AtomicUsize::new(0),
            allocation_count: AtomicUsize::new(0),
            deallocation_count: AtomicUsize::new(0),
        };

        // Initialize the allocator by adding the entire memory as one large block
        allocator.initialize_memory()?;

        log::debug!("Buddy allocator initialized: base_addr={:#x}, size={}",
                   base_addr, total_size);

        Ok(allocator)
    }

    /// Initialize the memory by adding blocks to free lists
    fn initialize_memory(&mut self) -> Result<(), BuddyError> {
        let mut remaining_size = self.total_size;
        let mut current_addr = self.base_addr;

        while remaining_size > 0 {
            // Find the largest power of 2 that fits
            let mut order = 0;
            while (1 << (order + 1)) * PAGE_SIZE <= remaining_size && order < MAX_ORDER {
                order += 1;
            }

            if order == 0 && remaining_size < PAGE_SIZE {
                break;
            }

            // Add this block to the appropriate free list
            self.add_block_to_free_list(current_addr, order)?;

            remaining_size -= (1 << order) * PAGE_SIZE;
            current_addr += (1 << order) * PAGE_SIZE;
        }

        Ok(())
    }

    /// Add a block to the free list for the given order
    fn add_block_to_free_list(&mut self, addr: usize, order: u8) -> Result<(), BuddyError> {
        if order > MAX_ORDER as u8 {
            return Err(BuddyError::InvalidOrder);
        }

        let block = addr as *mut BuddyBlock;
        unsafe {
            *block = BuddyBlock::new(order, true);
        }

        self.free_lists[order as usize].lock().push_front(block);
        self.total_free.fetch_add((1 << order) * PAGE_SIZE, Ordering::Relaxed);

        Ok(())
    }

    /// Remove a block from the free list for the given order
    fn remove_block_from_free_list(&self, order: u8) -> Option<*mut BuddyBlock> {
        if order > MAX_ORDER as u8 {
            return None;
        }

        let block = self.free_lists[order as usize].lock().pop_front();

        if let Some(block) = block {
            unsafe {
                let size = (*block).size();
                self.total_free.fetch_sub(size, Ordering::Relaxed);
            }
        }

        block
    }

    /// Allocate a block of the given order
    pub fn allocate(&self, order: u8) -> Result<usize, BuddyError> {
        if order > MAX_ORDER as u8 {
            return Err(BuddyError::InvalidOrder);
        }

        // Try to find a free block of the required order
        if let Some(block) = self.allocate_block(order) {
            let addr = unsafe { (*block).addr() };
            self.allocation_count.fetch_add(1, Ordering::Relaxed);
            self.total_allocated.fetch_add((1 << order) * PAGE_SIZE, Ordering::Relaxed);
            return Ok(addr);
        }

        Err(BuddyError::OutOfMemory)
    }

    /// Allocate a block, splitting larger blocks if necessary
    fn allocate_block(&self, order: u8) -> Option<*mut BuddyBlock> {
        // Try to find a block of the exact order
        if let Some(block) = self.remove_block_from_free_list(order) {
            return Some(block);
        }

        // Try to find a larger block and split it
        for current_order in (order + 1)..=MAX_ORDER as u8 {
            if let Some(larger_block) = self.remove_block_from_free_list(current_order) {
                return Some(self.split_block(larger_block, current_order, order));
            }
        }

        None
    }

    /// Split a block from current_order down to target_order
    fn split_block(&self, block: *mut BuddyBlock, current_order: u8, target_order: u8) -> *mut BuddyBlock {
        let mut current_block = block;
        let mut current_order = current_order;

        while current_order > target_order {
            current_order -= 1;

            unsafe {
                let block_size = (1 << (current_order + 1)) * PAGE_SIZE;
                let buddy_addr = (*current_block).addr() + (block_size / 2);

                // Create the buddy block
                let buddy = buddy_addr as *mut BuddyBlock;
                *buddy = BuddyBlock::new(current_order, true);

                // Add the buddy to the free list
                self.free_lists[current_order as usize].lock().push_front(buddy);
                self.total_free.fetch_add((1 << current_order) * PAGE_SIZE, Ordering::Relaxed);

                // Update the current block's order
                (*current_block).order = current_order;
            }
        }

        current_block
    }

    /// Deallocate a block
    pub fn deallocate(&self, addr: usize, order: u8) -> Result<(), BuddyError> {
        if order > MAX_ORDER as u8 {
            return Err(BuddyError::InvalidOrder);
        }

        if addr < self.base_addr || addr >= self.base_addr + self.total_size {
            return Err(BuddyError::InvalidAddress);
        }

        let block = addr as *mut BuddyBlock;

        // Validate the block
        unsafe {
            if !(*block).is_valid() {
                return Err(BuddyError::InvalidAddress);
            }
        }

        self.deallocation_count.fetch_add(1, Ordering::Relaxed);
        self.total_allocated.fetch_sub((1 << order) * PAGE_SIZE, Ordering::Relaxed);

        // Try to coalesce with buddy
        self.coalesce_block(block, order);

        Ok(())
    }

    /// Coalesce a block with its buddy if possible
    fn coalesce_block(&self, block: *mut BuddyBlock, order: u8) {
        let mut current_block = block;
        let mut current_order = order;

        while current_order < MAX_ORDER as u8 {
            unsafe {
                let buddy_addr = (*current_block).buddy_addr(self.base_addr);
                let buddy = buddy_addr as *mut BuddyBlock;

                // Check if buddy exists and is free
                if self.is_valid_buddy(buddy, current_order) {
                    // Remove buddy from free list
                    let removed = self.free_lists[current_order as usize]
                        .lock()
                        .remove(buddy);

                    if removed {
                        // Coalesce the blocks
                        let block_addr = (*current_block).addr();
                        let coalesced_addr = if block_addr < buddy_addr {
                            block_addr
                        } else {
                            buddy_addr
                        };

                        let coalesced_block = coalesced_addr as *mut BuddyBlock;
                        *coalesced_block = BuddyBlock::new(current_order + 1, true);

                        current_order += 1;
                        current_block = coalesced_block;
                        continue;
                    }
                }
            }

            break;
        }

        // Add the (potentially coalesced) block to the appropriate free list
        self.free_lists[current_order as usize].lock().push_front(current_block);
        self.total_free.fetch_add((1 << current_order) * PAGE_SIZE, Ordering::Relaxed);
    }

    /// Check if a buddy block is valid and free
    fn is_valid_buddy(&self, buddy: *mut BuddyBlock, order: u8) -> bool {
        unsafe {
            (*buddy).is_valid() &&
            (*buddy).order == order &&
            (*buddy).free
        }
    }

    /// Get allocator statistics
    pub fn stats(&self) -> BuddyStats {
        let mut free_blocks_per_order = [0; MAX_ORDER + 1];
        let mut total_free_memory = 0;

        for (i, free_list) in self.free_lists.iter().enumerate() {
            let list = free_list.lock();
            free_blocks_per_order[i] = list.len();
            total_free_memory += list.len() * ((1 << i) * PAGE_SIZE);
        }

        let total_allocated = self.total_allocated.load(Ordering::Relaxed);
        let total_free = self.total_free.load(Ordering::Relaxed);
        let fragmentation_ratio = if total_free > 0 {
            1.0 - (total_free as f64 / self.total_size as f64)
        } else {
            0.0
        };

        BuddyStats {
            total_memory: self.total_size,
            allocated_memory: total_allocated,
            free_memory: total_free,
            free_blocks_per_order,
            allocation_count: self.allocation_count.load(Ordering::Relaxed),
            deallocation_count: self.deallocation_count.load(Ordering::Relaxed),
            fragmentation_ratio,
        }
    }

    /// Get the base address
    pub fn base_addr(&self) -> usize {
        self.base_addr
    }

    /// Get the total size
    pub fn total_size(&self) -> usize {
        self.total_size
    }
}

/// Global buddy allocator instance
static mut BUDDY_ALLOCATOR: Option<BuddyAllocator> = None;
static BUDDY_ALLOCATOR_INIT: SpinLock<bool> = SpinLock::new(false);

/// Initialize the global buddy allocator
pub fn init(base_addr: usize, size: usize) -> Result<(), BuddyError> {
    let mut init_flag = BUDDY_ALLOCATOR_INIT.lock();

    if *init_flag {
        return Ok(());
    }

    let allocator = BuddyAllocator::new(base_addr, size)?;

    unsafe {
        BUDDY_ALLOCATOR = Some(allocator);
    }

    *init_flag = true;
    log::info!("Global buddy allocator initialized");
    Ok(())
}

/// Get the global buddy allocator
fn get_buddy_allocator() -> Option<&'static BuddyAllocator> {
    unsafe { BUDDY_ALLOCATOR.as_ref() }
}

/// Allocate memory using the buddy allocator
pub fn alloc(order: u8) -> Result<usize, BuddyError> {
    get_buddy_allocator()
        .ok_or(BuddyError::OutOfMemory)?
        .allocate(order)
}

/// Deallocate memory using the buddy allocator
pub fn dealloc(addr: usize, order: u8) -> Result<(), BuddyError> {
    get_buddy_allocator()
        .ok_or(BuddyError::InvalidAddress)?
        .deallocate(addr, order)
}

/// Get buddy allocator statistics
pub fn get_stats() -> Option<BuddyStats> {
    get_buddy_allocator().map(|allocator| allocator.stats())
}

/// Convert size to order
pub fn size_to_order(size: usize) -> Result<u8, BuddyError> {
    if size == 0 {
        return Err(BuddyError::InvalidSize);
    }

    if size % PAGE_SIZE != 0 {
        return Err(BuddyError::NotPowerOfTwo);
    }

    let pages = size / PAGE_SIZE;
    if !pages.is_power_of_two() {
        return Err(BuddyError::NotPowerOfTwo);
    }

    let order = pages.trailing_zeros() as u8;
    if order > MAX_ORDER as u8 {
        return Err(BuddyError::InvalidOrder);
    }

    Ok(order)
}

/// Convert order to size
pub const fn order_to_size(order: u8) -> usize {
    (1 << order) * PAGE_SIZE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_order_conversion() {
        assert_eq!(size_to_order(4096).unwrap(), 0);
        assert_eq!(size_to_order(8192).unwrap(), 1);
        assert_eq!(size_to_order(16384).unwrap(), 2);
        assert_eq!(order_to_size(0), 4096);
        assert_eq!(order_to_size(1), 8192);
        assert_eq!(order_to_size(2), 16384);
    }

    #[test]
    fn test_buddy_allocator_creation() {
        let allocator = BuddyAllocator::new(0x10000000, 1024 * 1024).unwrap();
        assert_eq!(allocator.base_addr(), 0x10000000);
        assert_eq!(allocator.total_size(), 1024 * 1024);
    }

    #[test]
    fn test_free_list_operations() {
        let mut list = FreeList::new();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);

        // This is a basic test - in practice, blocks would be properly allocated
        // and initialized with actual memory addresses
    }
}