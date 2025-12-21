//! Physical frame allocator
//!
//! Manages allocation and deallocation of physical memory frames.

use crate::core::mm::{FrameNr, PhysAddr, PAGE_SIZE, align_up, align_down};
use crate::utils::bitmap::Bitmap;
use crate::core::sync::SpinLock;
use core::ptr::NonNull;

/// Physical frame allocator
pub struct FrameAllocator {
    /// Bitmap tracking allocated/free frames
    bitmap: SpinLock<Bitmap>,
    /// Total number of frames
    total_frames: FrameNr,
    /// Start physical address of managed memory
    start_addr: PhysAddr,
    /// End physical address of managed memory
    end_addr: PhysAddr,
}

impl FrameAllocator {
    /// Create a new frame allocator
    ///
    /// # Safety
    /// The caller must ensure:
    /// - bitmap_data points to valid memory for the bitmap
    /// - The bitmap has enough bits for all frames
    /// - The memory region is valid and available
    pub unsafe fn new(
        bitmap_data: *mut u64,
        start_addr: PhysAddr,
        size: u64,
    ) -> Self {
        let total_frames = align_up(size) / PAGE_SIZE;
        let bitmap = Bitmap::new(bitmap_data, total_frames as usize);

        // Mark all frames as allocated initially
        bitmap.set_all();

        Self {
            bitmap: SpinLock::new(bitmap),
            total_frames,
            start_addr,
            end_addr: start_addr + align_up(size),
        }
    }

    /// Add a free memory region
    pub fn add_free_region(&self, start: PhysAddr, size: u64) {
        let start_frame = align_down(start) / PAGE_SIZE;
        let end_frame = align_up(start + size) / PAGE_SIZE;
        let allocator_start_frame = self.start_addr / PAGE_SIZE;

        for frame in start_frame..end_frame {
            if frame >= allocator_start_frame && frame < self.end_addr / PAGE_SIZE {
                let index = (frame - allocator_start_frame) as usize;
                if index < self.bitmap.lock().bits() {
                    self.bitmap.lock().clear_bit(index);
                }
            }
        }
    }

    /// Allocate a single frame
    pub fn allocate_frame(&self) -> Option<PhysAddr> {
        let mut bitmap = self.bitmap.lock();
        if let Some(index) = bitmap.find_and_set() {
            let frame = self.start_addr / PAGE_SIZE + index as u64;
            Some(frame * PAGE_SIZE)
        } else {
            None
        }
    }

    /// Allocate a specific frame
    pub fn allocate_frame_at(&self, frame: FrameNr) -> Option<PhysAddr> {
        let allocator_start = self.start_addr / PAGE_SIZE;
        let allocator_end = self.end_addr / PAGE_SIZE;

        if frame < allocator_start || frame >= allocator_end {
            return None;
        }

        let index = (frame - allocator_start) as usize;
        let mut bitmap = self.bitmap.lock();
        if !bitmap.test(index) {
            bitmap.set_bit(index);
            Some(frame * PAGE_SIZE)
        } else {
            None
        }
    }

    /// Allocate multiple contiguous frames
    pub fn allocate_frames(&self, count: usize) -> Option<PhysAddr> {
        if count == 0 {
            return None;
        }

        let mut bitmap = self.bitmap.lock();
        let mut found_start = None;

        // Search for a free run of count frames
        for start in 0..bitmap.bits() {
            // Check if we have enough remaining bits
            if start + count > bitmap.bits() {
                break;
            }

            // Check if all frames in this range are free
            let mut all_free = true;
            for offset in 0..count {
                if bitmap.test(start + offset) {
                    all_free = false;
                    break;
                }
            }

            if all_free {
                found_start = Some(start);
                break;
            }
        }

        // Mark frames as allocated
        if let Some(start) = found_start {
            for offset in 0..count {
                bitmap.set_bit(start + offset);
            }
            let frame = self.start_addr / PAGE_SIZE + start as u64;
            Some(frame * PAGE_SIZE)
        } else {
            None
        }
    }

    /// Deallocate a frame
    pub fn deallocate_frame(&self, addr: PhysAddr) -> bool {
        let frame = align_down(addr) / PAGE_SIZE;
        let allocator_start = self.start_addr / PAGE_SIZE;
        let allocator_end = self.end_addr / PAGE_SIZE;

        if frame < allocator_start || frame >= allocator_end {
            return false;
        }

        let index = (frame - allocator_start) as usize;
        let mut bitmap = self.bitmap.lock();
        if bitmap.test(index) {
            bitmap.clear_bit(index);
            true
        } else {
            false
        }
    }

    /// Deallocate multiple contiguous frames
    pub fn deallocate_frames(&self, addr: PhysAddr, count: usize) -> bool {
        let start_frame = align_down(addr) / PAGE_SIZE;
        let allocator_start = self.start_addr / PAGE_SIZE;
        let allocator_end = self.end_addr / PAGE_SIZE;

        // Check if all frames are in range
        if start_frame < allocator_start ||
           start_frame + count as u64 > allocator_end {
            return false;
        }

        let start_index = (start_frame - allocator_start) as usize;
        let mut bitmap = self.bitmap.lock();

        // Check if all frames are allocated
        for offset in 0..count {
            if !bitmap.test(start_index + offset) {
                return false;
            }
        }

        // Free the frames
        for offset in 0..count {
            bitmap.clear_bit(start_index + offset);
        }
        true
    }

    /// Get the number of free frames
    pub fn free_frames(&self) -> usize {
        self.bitmap.lock().count_zeros()
    }

    /// Get the number of allocated frames
    pub fn allocated_frames(&self) -> usize {
        self.bitmap.lock().count_ones()
    }

    /// Get the total number of frames
    pub fn total_frames(&self) -> usize {
        self.total_frames as usize
    }

    /// Get the start address of managed memory
    pub fn start_addr(&self) -> PhysAddr {
        self.start_addr
    }

    /// Get the end address of managed memory
    pub fn end_addr(&self) -> PhysAddr {
        self.end_addr
    }

    /// Check if an address is within managed memory
    pub fn contains(&self, addr: PhysAddr) -> bool {
        addr >= self.start_addr && addr < self.end_addr
    }

    /// Get memory usage statistics
    pub fn stats(&self) -> FrameStats {
        let bitmap = self.bitmap.lock();
        FrameStats {
            total_frames: self.total_frames as usize,
            free_frames: bitmap.count_zeros(),
            allocated_frames: bitmap.count_ones(),
            total_bytes: (self.end_addr - self.start_addr) as usize,
            free_bytes: bitmap.count_zeros() * PAGE_SIZE as usize,
            allocated_bytes: bitmap.count_ones() * PAGE_SIZE as usize,
        }
    }
}

/// Frame allocator statistics
#[derive(Debug, Clone, Copy)]
pub struct FrameStats {
    /// Total number of frames
    pub total_frames: usize,
    /// Number of free frames
    pub free_frames: usize,
    /// Number of allocated frames
    pub allocated_frames: usize,
    /// Total bytes managed
    pub total_bytes: usize,
    /// Free bytes
    pub free_bytes: usize,
    /// Allocated bytes
    pub allocated_bytes: usize,
}

/// Global frame allocator instance
static mut FRAME_ALLOCATOR: Option<FrameAllocator> = None;
static mut FRAME_ALLOCATOR_INITIALIZED: bool = false;

/// Initialize the global frame allocator
///
/// # Safety
/// Must be called only once during system initialization
pub unsafe fn init() -> Result<(), crate::Error> {
    // TODO: Parse memory map and set up allocator
    // For now, we'll set up a simple allocator
    Err(crate::Error::NotImplemented)
}

/// Get the global frame allocator
pub fn get_frame_allocator() -> &'static FrameAllocator {
    unsafe {
        FRAME_ALLOCATOR.as_ref().unwrap()
    }
}

/// Set up the global frame allocator
///
/// # Safety
/// Must be called during initialization before using the allocator
pub unsafe fn setup_allocator(allocator: FrameAllocator) {
    FRAME_ALLOCATOR = Some(allocator);
    FRAME_ALLOCATOR_INITIALIZED = true;
}

/// Allocate a physical frame
pub fn alloc_frame() -> Option<PhysAddr> {
    get_frame_allocator().allocate_frame()
}

/// Allocate specific physical frame
pub fn alloc_frame_at(frame: FrameNr) -> Option<PhysAddr> {
    get_frame_allocator().allocate_frame_at(frame)
}

/// Allocate multiple contiguous frames
pub fn alloc_frames(count: usize) -> Option<PhysAddr> {
    get_frame_allocator().allocate_frames(count)
}

/// Deallocate a physical frame
pub fn dealloc_frame(addr: PhysAddr) -> bool {
    get_frame_allocator().deallocate_frame(addr)
}

/// Deallocate multiple contiguous frames
pub fn dealloc_frames(addr: PhysAddr, count: usize) -> bool {
    get_frame_allocator().deallocate_frames(addr, count)
}

/// Get frame allocator statistics
pub fn get_frame_stats() -> FrameStats {
    get_frame_allocator().stats()
}

/// Reserved frame numbers for special purposes
pub mod reserved {
    /// First 4MB is typically reserved
    pub const RESERVED_FRAMES: u64 = 1024; // 4MB / 4KB
}

/// Convert from physical address to frame
pub fn phys_to_frame(addr: PhysAddr) -> FrameNr {
    addr / PAGE_SIZE
}

/// Convert from frame to physical address
pub fn frame_to_phys(frame: FrameNr) -> PhysAddr {
    frame * PAGE_SIZE
}

/// Check if a physical address is valid
pub fn is_valid_phys_addr(addr: PhysAddr) -> bool {
    // TODO: Check against memory map
    addr < (1u64 << 48) // 48-bit physical addresses
}

/// Check if a frame number is valid
pub fn is_valid_frame(frame: FrameNr) -> bool {
    is_valid_phys_addr(frame * PAGE_SIZE)
}