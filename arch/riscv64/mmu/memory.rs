//! RISC-V Memory Management
//!
//! This module provides memory management functionality for RISC-V including:
//! - Physical memory management
/// - Virtual memory management
/// - Memory regions and permissions
/// - Memory allocation and mapping

use crate::arch::riscv64::*;
use crate::arch::riscv64::mmu::ptable::*;
use bitflags::bitflags;

/// Memory region flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MemFlags: usize {
        const READABLE = 1 << 0;
        const WRITABLE = 1 << 1;
        const EXECUTABLE = 1 << 2;
        const USER = 1 << 3;
        const DEVICE = 1 << 4;
        const UNCACHED = 1 << 5;
        const WRITE_COMBINE = 1 << 6;
        const SHARED = 1 << 7;
    }
}

impl From<MemFlags> for PteFlags {
    fn from(flags: MemFlags) -> Self {
        let mut pte_flags = PteFlags::empty();

        if flags.contains(MemFlags::READABLE) {
            pte_flags |= PteFlags::R;
        }
        if flags.contains(MemFlags::WRITABLE) {
            pte_flags |= PteFlags::W;
        }
        if flags.contains(MemFlags::EXECUTABLE) {
            pte_flags |= PteFlags::X;
        }
        if flags.contains(MemFlags::USER) {
            pte_flags |= PteFlags::U;
        }

        // Memory type flags would need additional handling
        // RISC-V doesn't have explicit memory type bits in PTEs
        // These would be handled through platform-specific mechanisms

        pte_flags
    }
}

/// Memory region descriptor
#[derive(Debug, Clone)]
pub struct MemRegion {
    /// Start virtual address
    pub va_start: usize,
    /// Start physical address
    pub pa_start: usize,
    /// Size of the region
    pub size: usize,
    /// Memory flags
    pub flags: MemFlags,
    /// Name of the region (for debugging)
    pub name: &'static str,
}

impl MemRegion {
    /// Create a new memory region
    pub fn new(
        va_start: usize,
        pa_start: usize,
        size: usize,
        flags: MemFlags,
        name: &'static str,
    ) -> Self {
        Self {
            va_start,
            pa_start,
            size,
            flags,
            name,
        }
    }

    /// Get the end virtual address (exclusive)
    pub fn va_end(&self) -> usize {
        self.va_start + self.size
    }

    /// Get the end physical address (exclusive)
    pub fn pa_end(&self) -> usize {
        self.pa_start + self.size
    }

    /// Check if a virtual address is in this region
    pub fn contains_va(&self, va: usize) -> bool {
        va >= self.va_start && va < self.va_end()
    }

    /// Check if a physical address is in this region
    pub fn contains_pa(&self, pa: usize) -> bool {
        pa >= self.pa_start && pa < self.pa_end()
    }

    /// Split region at the given virtual address
    pub fn split_at_va(&self, va: usize) -> Option<(MemRegion, MemRegion)> {
        if !self.contains_va(va) {
            return None;
        }

        let left_size = va - self.va_start;
        let right_size = self.va_end() - va;

        Some((
            MemRegion::new(
                self.va_start,
                self.pa_start,
                left_size,
                self.flags,
                self.name,
            ),
            MemRegion::new(
                va,
                self.pa_start + left_size,
                right_size,
                self.flags,
                self.name,
            ),
        ))
    }
}

/// Physical memory frame
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    /// Physical frame number
    pub ppn: usize,
    /// Frame size (must be power of two and multiple of page size)
    pub size: usize,
}

impl Frame {
    /// Create a new frame
    pub fn new(ppn: usize, size: usize) -> Self {
        assert!(size.is_power_of_two() && size >= PAGE_SIZE);
        Self { ppn, size }
    }

    /// Get the physical address of this frame
    pub fn pa(&self) -> usize {
        self.ppn << PAGE_SHIFT
    }

    /// Get the end physical address (exclusive)
    pub fn pa_end(&self) -> usize {
        self.pa() + self.size
    }

    /// Check if this frame contains a physical address
    pub fn contains_pa(&self, pa: usize) -> bool {
        pa >= self.pa() && pa < self.pa_end()
    }

    /// Get the number of pages in this frame
    pub fn num_pages(&self) -> usize {
        self.size / PAGE_SIZE
    }
}

/// Frame allocator trait
pub trait FrameAllocator {
    /// Allocate a frame of the given size
    fn allocate(&mut self, size: usize) -> Option<Frame>;

    /// Deallocate a frame
    fn deallocate(&mut self, frame: Frame);

    /// Get total available memory
    fn available_memory(&self) -> usize;

    /// Get total memory
    fn total_memory(&self) -> usize;
}

/// Simple frame allocator implementation
pub struct SimpleFrameAllocator {
    /// Base physical address
    base_pa: usize,
    /// Total memory size
    total_size: usize,
    /// Next free frame
    next_free: usize,
    /// Free list for deallocated frames
    free_list: Vec<Frame>,
}

impl SimpleFrameAllocator {
    /// Create a new frame allocator
    pub fn new(base_pa: usize, total_size: usize) -> Self {
        // Ensure base is page-aligned
        let aligned_base = (base_pa + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        let aligned_size = total_size - (aligned_base - base_pa);

        Self {
            base_pa: aligned_base,
            total_size: aligned_size,
            next_free: aligned_base >> PAGE_SHIFT,
            free_list: Vec::new(),
        }
    }

    /// Initialize the allocator from a memory map
    pub fn init(&mut self, reserved_regions: &[MemRegion]) {
        // Mark reserved regions as used
        for region in reserved_regions {
            // TODO: Implement proper reservation handling
        }
    }
}

impl FrameAllocator for SimpleFrameAllocator {
    fn allocate(&mut self, size: usize) -> Option<Frame> {
        // Round up size to nearest page
        let aligned_size = (size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);

        // Try to reuse from free list
        for (i, frame) in self.free_list.iter().enumerate() {
            if frame.size >= aligned_size {
                let allocated = Frame::new(frame.ppn, aligned_size);
                self.free_list.remove(i);
                return Some(allocated);
            }
        }

        // Allocate from the end
        let end_ppn = (self.base_pa + self.total_size) >> PAGE_SHIFT;
        let needed_pages = aligned_size / PAGE_SIZE;

        if self.next_free + needed_pages > end_ppn {
            return None;
        }

        let ppn = self.next_free;
        self.next_free += needed_pages;

        Some(Frame::new(ppn, aligned_size))
    }

    fn deallocate(&mut self, frame: Frame) {
        self.free_list.push(frame);
        // TODO: Coalesce adjacent free frames
    }

    fn available_memory(&self) -> usize {
        let end_ppn = (self.base_pa + self.total_size) >> PAGE_SHIFT;
        let remaining = (end_ppn - self.next_free) << PAGE_SHIFT;
        let free_from_list: usize = self.free_list.iter().map(|f| f.size).sum();
        remaining + free_from_list
    }

    fn total_memory(&self) -> usize {
        self.total_size
    }
}

/// Address space represents a virtual address space
pub struct AddressSpace {
    /// Root page table
    root: RootPageTable,
    /// Memory regions in this address space
    regions: Vec<MemRegion>,
    /// Frame allocator
    allocator: Option<&'static mut dyn FrameAllocator>,
}

impl AddressSpace {
    /// Create a new address space
    pub fn new(mode: u8, asid: Asid) -> Result<Self, &'static str> {
        let root = RootPageTable::new(mode, asid)?;

        Ok(Self {
            root,
            regions: Vec::new(),
            allocator: None,
        })
    }

    /// Create a new address space with a frame allocator
    pub fn with_allocator(
        mode: u8,
        asid: Asid,
        allocator: &'static mut dyn FrameAllocator,
    ) -> Result<Self, &'static str> {
        let root = RootPageTable::new(mode, asid)?;

        Ok(Self {
            root,
            regions: Vec::new(),
            allocator: Some(allocator),
        })
    }

    /// Get the ASID
    pub fn asid(&self) -> Asid {
        self.root.asid()
    }

    /// Get the translation mode
    pub fn mode(&self) -> u8 {
        self.root.mode()
    }

    /// Get the SATP value for this address space
    pub fn satp(&self) -> usize {
        self.root.satp()
    }

    /// Activate this address space
    pub fn activate(&self) {
        self.root.activate();
    }

    /// Map a memory region
    pub fn map_region(&mut self, region: MemRegion) -> Result<(), &'static str> {
        let pte_flags: PteFlags = region.flags.into();
        let levels = match self.mode() {
            8 => 3, // Sv39
            9 => 4, // Sv48
            _ => return Err("Unsupported translation mode"),
        };

        // Map each page in the region
        let mut va = region.va_start;
        let mut pa = region.pa_start;
        let remaining = region.size;

        while remaining > 0 {
            self.root.root_mut().map(va, pa, pte_flags, levels)?;

            let page_size = PAGE_SIZE;
            va += page_size;
            pa += page_size;

            let remaining = remaining.saturating_sub(page_size);
        }

        self.regions.push(region);
        Ok(())
    }

    /// Unmap a memory region
    pub fn unmap_region(&mut self, va_start: usize, size: usize) -> Result<(), &'static str> {
        let levels = match self.mode() {
            8 => 3, // Sv39
            9 => 4, // Sv48
            _ => return Err("Unsupported translation mode"),
        };

        let mut va = va_start;
        let remaining = size;

        while remaining > 0 {
            self.root.root_mut().unmap(va, levels)?;

            va += PAGE_SIZE;
            let remaining = remaining.saturating_sub(PAGE_SIZE);
        }

        // Remove the region from our list
        self.regions.retain(|r| {
            !(r.va_start >= va_start && r.va_start < va_start + size)
        });

        Ok(())
    }

    /// Translate a virtual address
    pub fn translate(&self, va: usize) -> Result<(usize, PteFlags), &'static str> {
        let root_ppn = self.root.root().ppn();
        let result = super::translation::translate_single_stage(root_ppn, va, self.mode());

        if result.success {
            Ok((result.pa, result.flags))
        } else {
            Err("Translation failed")
        }
    }

    /// Get all memory regions
    pub fn regions(&self) -> &[MemRegion] {
        &self.regions
    }

    /// Find a region containing the given virtual address
    pub fn find_region(&self, va: usize) -> Option<&MemRegion> {
        self.regions.iter().find(|r| r.contains_va(va))
    }
}

/// Memory management unit
pub struct Mmu {
    /// ASID manager
    asid_manager: AsidManager,
    /// Frame allocator
    allocator: &'static mut dyn FrameAllocator,
}

impl Mmu {
    /// Create a new MMU instance
    pub fn new(allocator: &'static mut dyn FrameAllocator) -> Self {
        Self {
            asid_manager: AsidManager::new(4095), // 12-bit ASID
            allocator,
        }
    }

    /// Create a new address space
    pub fn create_address_space(&mut self, mode: u8) -> Result<AddressSpace, &'static str> {
        let asid = self.asid_manager.allocate()?;
        AddressSpace::with_allocator(mode, asid, self.allocator)
    }

    /// Destroy an address space
    pub fn destroy_address_space(&mut self, addr_space: AddressSpace) {
        self.asid_manager.free(addr_space.asid());
        // TODO: Free all frames used by the address space
    }

    /// Get memory statistics
    pub fn memory_stats(&self) -> MemoryStats {
        MemoryStats {
            total_memory: self.allocator.total_memory(),
            available_memory: self.allocator.available_memory(),
            used_memory: self.allocator.total_memory() - self.allocator.available_memory(),
        }
    }
}

/// Memory statistics
#[derive(Debug, Clone, Copy)]
pub struct MemoryStats {
    /// Total memory
    pub total_memory: usize,
    /// Available memory
    pub available_memory: usize,
    /// Used memory
    pub used_memory: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mem_region() {
        let region = MemRegion::new(
            0x1000,
            0x2000,
            0x3000,
            MemFlags::READABLE | MemFlags::WRITABLE | MemFlags::USER,
            "test_region",
        );

        assert_eq!(region.va_start, 0x1000);
        assert_eq!(region.pa_start, 0x2000);
        assert_eq!(region.size, 0x3000);
        assert!(region.contains_va(0x1000));
        assert!(region.contains_va(0x3000));
        assert!(!region.contains_va(0x4000));

        let split = region.split_at_va(0x2000);
        assert!(split.is_some());

        let (left, right) = split.unwrap();
        assert_eq!(left.va_start, 0x1000);
        assert_eq!(left.size, 0x1000);
        assert_eq!(right.va_start, 0x2000);
        assert_eq!(right.size, 0x2000);
    }

    #[test]
    fn test_frame() {
        let frame = Frame::new(0x12345, 0x2000);
        assert_eq!(frame.pa(), 0x12345000);
        assert_eq!(frame.pa_end(), 0x12347000);
        assert_eq!(frame.num_pages(), 2);

        assert!(frame.contains_pa(0x12345000));
        assert!(!frame.contains_pa(0x12347000));
    }

    #[test]
    fn test_simple_frame_allocator() {
        let mut allocator = SimpleFrameAllocator::new(0x80000000, 0x1000000); // 16MB

        assert_eq!(allocator.total_memory(), 0x1000000);
        assert_eq!(allocator.available_memory(), 0x1000000);

        let frame1 = allocator.allocate(0x1000).unwrap();
        assert_eq!(frame1.size, 0x1000);
        assert_eq!(allocator.available_memory(), 0x1000000 - 0x1000);

        let frame2 = allocator.allocate(0x2000).unwrap();
        assert_eq!(frame2.size, 0x2000);

        allocator.deallocate(frame1);
        // After deallocation, we should be able to reuse the frame
        let frame3 = allocator.allocate(0x1000).unwrap();
        assert_eq!(frame3.size, 0x1000);
    }

    #[test]
    fn test_mem_flags_conversion() {
        let mem_flags = MemFlags::READABLE | MemFlags::WRITABLE | MemFlags::EXECUTABLE | MemFlags::USER;
        let pte_flags: PteFlags = mem_flags.into();

        assert!(pte_flags.contains(PteFlags::R));
        assert!(pte_flags.contains(PteFlags::W));
        assert!(pte_flags.contains(PteFlags::X));
        assert!(pte_flags.contains(PteFlags::U));
        assert!(pte_flags.contains(PteFlags::V));
    }
}