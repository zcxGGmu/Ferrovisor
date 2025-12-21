//! Unified memory allocator interface
//!
//! Provides a high-level interface that integrates all memory allocators:
//! - Buddy allocator for large allocations
//! - Slab allocator for small, frequently allocated objects
//! - Frame allocator for physical memory management
//!
//! This module provides a unified allocation interface that automatically
//! chooses the best allocator based on size and usage patterns.

use crate::core::mm::{PAGE_SIZE, buddy, slab, frame};
use crate::core::sync::SpinLock;
use core::ptr::NonNull;

/// Allocation strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationStrategy {
    /// Use buddy allocator (best for large allocations)
    Buddy,
    /// Use slab allocator (best for small, frequent allocations)
    Slab,
    /// Use frame allocator (for physical memory)
    Frame,
    /// Auto-select based on size
    Auto,
}

/// Memory allocation configuration
#[derive(Debug, Clone)]
pub struct AllocationConfig {
    /// Allocation strategy
    pub strategy: AllocationStrategy,
    /// Alignment requirement
    pub alignment: usize,
    /// Whether to zero the memory
    pub zero: bool,
    /// Whether memory is reclaimable
    pub reclaimable: bool,
    /// Purpose tag for debugging
    pub tag: &'static str,
}

impl Default for AllocationConfig {
    fn default() -> Self {
        Self {
            strategy: AllocationStrategy::Auto,
            alignment: 8,
            zero: false,
            reclaimable: true,
            tag: "general",
        }
    }
}

/// Memory allocation statistics
#[derive(Debug, Clone)]
pub struct AllocationStats {
    /// Total allocations performed
    pub total_allocations: u64,
    /// Total deallocations performed
    pub total_deallocations: u64,
    /// Total memory allocated
    pub total_allocated: u64,
    /// Total memory freed
    pub total_freed: u64,
    /// Current memory usage
    pub current_usage: u64,
    /// Peak memory usage
    pub peak_usage: u64,
    /// Number of failed allocations
    pub failed_allocations: u64,
    /// Allocation efficiency (0.0 to 1.0)
    pub efficiency: f64,
    /// Fragmentation ratio (0.0 to 1.0)
    pub fragmentation: f64,
}

/// Unified memory allocator
pub struct UnifiedAllocator {
    /// Global statistics
    stats: SpinLock<AllocationStats>,
    /// Current peak usage
    peak_usage: u64,
    /// Allocation threshold for using buddy vs slab
    buddy_threshold: usize,
}

impl UnifiedAllocator {
    /// Create a new unified allocator
    pub fn new() -> Self {
        Self {
            stats: SpinLock::new(AllocationStats {
                total_allocations: 0,
                total_deallocations: 0,
                total_allocated: 0,
                total_freed: 0,
                current_usage: 0,
                peak_usage: 0,
                failed_allocations: 0,
                efficiency: 1.0,
                fragmentation: 0.0,
            }),
            peak_usage: 0,
            buddy_threshold: 8 * PAGE_SIZE, // 32KB threshold for buddy allocator
        }
    }

    /// Allocate memory using the best strategy
    pub fn allocate(&self, size: usize, config: AllocationConfig) -> Result<NonNull<u8>, AllocationError> {
        if size == 0 {
            return Err(AllocationError::InvalidSize);
        }

        let strategy = if config.strategy == AllocationStrategy::Auto {
            self.select_strategy(size)
        } else {
            config.strategy
        };

        let result = match strategy {
            AllocationStrategy::Buddy => self.allocate_buddy(size, &config),
            AllocationStrategy::Slab => self.allocate_slab(size, &config),
            AllocationStrategy::Frame => self.allocate_frame(size, &config),
            AllocationStrategy::Auto => unreachable!(),
        };

        match result {
            Ok(ptr) => {
                if config.zero {
                    unsafe {
                        core::ptr::write_bytes(ptr.as_ptr(), 0, size);
                    }
                }

                self.update_allocation_stats(size, true);
                log::debug!("Allocated {} bytes at {:p} using {:?} strategy",
                          size, ptr.as_ptr(), strategy);
                Ok(ptr)
            }
            Err(e) => {
                self.update_failure_stats();
                log::warn!("Failed to allocate {} bytes: {:?}", size, e);
                Err(e)
            }
        }
    }

    /// Deallocate memory
    pub fn deallocate(&self, ptr: NonNull<u8>, size: usize, strategy: AllocationStrategy) -> Result<(), AllocationError> {
        if size == 0 {
            return Err(AllocationError::InvalidSize);
        }

        let result = match strategy {
            AllocationStrategy::Buddy => self.deallocate_buddy(ptr, size),
            AllocationStrategy::Slab => self.deallocate_slab(ptr, size),
            AllocationStrategy::Frame => self.deallocate_frame(ptr, size),
            AllocationStrategy::Auto => {
                // Try to determine the strategy based on size
                if size >= self.buddy_threshold {
                    self.deallocate_buddy(ptr, size)
                } else {
                    self.deallocate_slab(ptr, size)
                }
            }
        };

        match result {
            Ok(()) => {
                self.update_allocation_stats(size, false);
                log::debug!("Deallocated {} bytes at {:p}", size, ptr.as_ptr());
                Ok(())
            }
            Err(e) => {
                log::warn!("Failed to deallocate {} bytes at {:p}: {:?}", size, ptr.as_ptr(), e);
                Err(e)
            }
        }
    }

    /// Reallocate memory
    pub fn reallocate(&self, ptr: Option<NonNull<u8>>, old_size: usize, new_size: usize, config: AllocationConfig) -> Result<NonNull<u8>, AllocationError> {
        if new_size == 0 {
            return Err(AllocationError::InvalidSize);
        }

        if old_size == new_size {
            return ptr.ok_or(AllocationError::InvalidPointer);
        }

        // If new size is smaller, just return the existing pointer
        if new_size < old_size {
            return ptr.ok_or(AllocationError::InvalidPointer);
        }

        // Allocate new memory
        let new_ptr = self.allocate(new_size, config)?;

        // Copy old data if pointer exists
        if let Some(old_ptr) = ptr {
            let copy_size = old_size.min(new_size);
            unsafe {
                core::ptr::copy_nonoverlapping(old_ptr.as_ptr(), new_ptr.as_ptr(), copy_size);
            }

            // Free old memory
            let strategy = if old_size >= self.buddy_threshold {
                AllocationStrategy::Buddy
            } else {
                AllocationStrategy::Slab
            };
            let _ = self.deallocate(old_ptr, old_size, strategy);
        }

        Ok(new_ptr)
    }

    /// Select the best allocation strategy based on size
    fn select_strategy(&self, size: usize) -> AllocationStrategy {
        if size >= self.buddy_threshold {
            AllocationStrategy::Buddy
        } else {
            AllocationStrategy::Slab
        }
    }

    /// Allocate using buddy allocator
    fn allocate_buddy(&self, size: usize, config: &AllocationConfig) -> Result<NonNull<u8>, AllocationError> {
        // Align size to page boundary for buddy allocator
        let aligned_size = (size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);

        // Convert to order
        let order = buddy::size_to_order(aligned_size)
            .map_err(|_| AllocationError::InvalidSize)?;

        // Allocate from buddy allocator
        let addr = buddy::alloc(order)
            .map_err(|_| AllocationError::OutOfMemory)?;

        Ok(NonNull::new(addr as *mut u8).unwrap())
    }

    /// Allocate using slab allocator
    fn allocate_slab(&self, size: usize, config: &AllocationConfig) -> Result<NonNull<u8>, AllocationError> {
        slab::alloc(size)
            .map_err(|_| AllocationError::OutOfMemory)
    }

    /// Allocate using frame allocator
    fn allocate_frame(&self, size: usize, config: &AllocationConfig) -> Result<NonNull<u8>, AllocationError> {
        let page_count = (size + PAGE_SIZE - 1) / PAGE_SIZE;

        let frame_addr = frame::alloc_frames(page_count)
            .ok_or(AllocationError::OutOfMemory)?;

        Ok(NonNull::new(frame_addr as *mut u8).unwrap())
    }

    /// Deallocate using buddy allocator
    fn deallocate_buddy(&self, ptr: NonNull<u8>, size: usize) -> Result<(), AllocationError> {
        let addr = ptr.as_ptr() as usize;
        let aligned_size = (size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);

        let order = buddy::size_to_order(aligned_size)
            .map_err(|_| AllocationError::InvalidSize)?;

        buddy::dealloc(addr, order)
            .map_err(|_| AllocationError::InvalidAddress)
    }

    /// Deallocate using slab allocator
    fn deallocate_slab(&self, ptr: NonNull<u8>, size: usize) -> Result<(), AllocationError> {
        slab::dealloc(ptr, size)
            .map_err(|_| AllocationError::InvalidAddress)
    }

    /// Deallocate using frame allocator
    fn deallocate_frame(&self, ptr: NonNull<u8>, size: usize) -> Result<(), AllocationError> {
        let frame_addr = ptr.as_ptr() as u64;
        let page_count = (size + PAGE_SIZE - 1) / PAGE_SIZE;

        if frame::dealloc_frames(frame_addr, page_count) {
            Ok(())
        } else {
            Err(AllocationError::InvalidAddress)
        }
    }

    /// Update allocation statistics
    fn update_allocation_stats(&self, size: usize, is_allocation: bool) {
        let mut stats = self.stats.lock();

        if is_allocation {
            stats.total_allocations += 1;
            stats.total_allocated += size as u64;
            stats.current_usage += size as u64;

            if stats.current_usage > stats.peak_usage {
                stats.peak_usage = stats.current_usage;
                self.peak_usage = stats.current_usage;
            }
        } else {
            stats.total_deallocations += 1;
            stats.total_freed += size as u64;
            stats.current_usage = stats.current_usage.saturating_sub(size as u64);
        }

        // Update efficiency and fragmentation
        self.update_derived_stats(&mut stats);
    }

    /// Update failure statistics
    fn update_failure_stats(&self) {
        let mut stats = self.stats.lock();
        stats.failed_allocations += 1;
    }

    /// Update derived statistics (efficiency, fragmentation)
    fn update_derived_stats(&self, stats: &mut AllocationStats) {
        // Calculate efficiency (allocated / requested)
        stats.efficiency = if stats.total_allocated > 0 {
            (stats.total_allocated as f64 - stats.total_freed as f64) / stats.total_allocated as f64
        } else {
            1.0
        };

        // Get fragmentation from buddy allocator
        if let Some(buddy_stats) = buddy::get_stats() {
            stats.fragmentation = buddy_stats.fragmentation_ratio;
        } else {
            stats.fragmentation = 0.0;
        }
    }

    /// Get allocation statistics
    pub fn stats(&self) -> AllocationStats {
        let mut stats = self.stats.lock();
        self.update_derived_stats(&mut stats);
        *stats
    }

    /// Get memory usage information
    pub fn memory_info(&self) -> MemoryInfo {
        MemoryInfo {
            total_allocated: self.stats.lock().total_allocated,
            current_usage: self.stats.lock().current_usage,
            peak_usage: self.peak_usage,
            buddy_stats: buddy::get_stats(),
            slab_stats: slab::get_stats(),
            frame_stats: frame::get_frame_stats(),
        }
    }

    /// Set the buddy threshold
    pub fn set_buddy_threshold(&mut self, threshold: usize) {
        self.buddy_threshold = threshold;
    }

    /// Reclaim memory from all allocators
    pub fn reclaim_memory(&self) -> usize {
        // Shrink slab caches
        let slab_freed = slab::shrink_all();

        log::info!("Reclaimed {} pages from slab allocator", slab_freed);
        slab_freed
    }
}

/// Memory usage information
#[derive(Debug, Clone)]
pub struct MemoryInfo {
    /// Total memory allocated
    pub total_allocated: u64,
    /// Current memory usage
    pub current_usage: u64,
    /// Peak memory usage
    pub peak_usage: u64,
    /// Buddy allocator statistics
    pub buddy_stats: Option<buddy::BuddyStats>,
    /// Slab allocator statistics
    pub slab_stats: slab::SlabAllocatorStats,
    /// Frame allocator statistics
    pub frame_stats: frame::FrameStats,
}

/// Allocation errors
#[derive(Debug, Clone, PartialEq)]
pub enum AllocationError {
    /// Invalid size
    InvalidSize,
    /// Out of memory
    OutOfMemory,
    /// Invalid pointer
    InvalidPointer,
    /// Invalid address
    InvalidAddress,
    /// Alignment not supported
    UnsupportedAlignment,
}

/// Global unified allocator instance
static mut UNIFIED_ALLOCATOR: Option<UnifiedAllocator> = None;
static UNIFIED_ALLOCATOR_INIT: SpinLock<bool> = SpinLock::new(false);

/// Initialize the unified allocator
pub fn init() -> Result<(), AllocationError> {
    let mut init_flag = UNIFIED_ALLOCATOR_INIT.lock();

    if *init_flag {
        return Ok(());
    }

    let allocator = UnifiedAllocator::new();

    unsafe {
        UNIFIED_ALLOCATOR = Some(allocator);
    }

    *init_flag = true;
    log::info!("Unified memory allocator initialized");
    Ok(())
}

/// Get the global unified allocator
fn get_unified_allocator() -> &'static UnifiedAllocator {
    unsafe {
        UNIFIED_ALLOCATOR.as_ref().unwrap()
    }
}

/// Allocate memory using the unified allocator
pub fn allocate(size: usize) -> Result<NonNull<u8>, AllocationError> {
    get_unified_allocator().allocate(size, AllocationConfig::default())
}

/// Allocate memory with custom configuration
pub fn allocate_with_config(size: usize, config: AllocationConfig) -> Result<NonNull<u8>, AllocationError> {
    get_unified_allocator().allocate(size, config)
}

/// Deallocate memory using the unified allocator
pub fn deallocate(ptr: NonNull<u8>, size: usize) -> Result<(), AllocationError> {
    get_unified_allocator().deallocate(ptr, size, AllocationStrategy::Auto)
}

/// Reallocate memory using the unified allocator
pub fn reallocate(ptr: Option<NonNull<u8>>, old_size: usize, new_size: usize) -> Result<NonNull<u8>, AllocationError> {
    get_unified_allocator().reallocate(ptr, old_size, new_size, AllocationConfig::default())
}

/// Get unified allocator statistics
pub fn get_stats() -> AllocationStats {
    get_unified_allocator().stats()
}

/// Get memory usage information
pub fn get_memory_info() -> MemoryInfo {
    get_unified_allocator().memory_info()
}

/// Reclaim memory from all allocators
pub fn reclaim_memory() -> usize {
    get_unified_allocator().reclaim_memory()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unified_allocator() {
        let allocator = UnifiedAllocator::new();

        // Test allocation strategies
        assert_eq!(allocator.select_strategy(1024), AllocationStrategy::Slab);
        assert_eq!(allocator.select_strategy(64 * 1024), AllocationStrategy::Buddy);
    }

    #[test]
    fn test_allocation_config() {
        let config = AllocationConfig::default();
        assert_eq!(config.strategy, AllocationStrategy::Auto);
        assert_eq!(config.alignment, 8);
        assert!(!config.zero);
    }
}