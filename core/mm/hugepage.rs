//! Huge Page Management
//!
//! This module provides support for huge pages (2MB, 1GB) to reduce TLB pressure
//! and improve memory access performance for large contiguous allocations.

use crate::core::mm::{
    PhysAddr, VirtAddr, PageNr, FrameNr, PAGE_SIZE, PageSize,
    align_up, align_down, default_huge_page_size, default_huge_page_shift,
};
use crate::core::mm::frame::{alloc_contiguous_frames, dealloc_contiguous_frames};
use crate::core::sync::SpinLock;
use core::ptr::NonNull;

/// Huge page descriptor
#[derive(Debug, Clone)]
pub struct HugePage {
    /// Physical address of the huge page
    pub phys_addr: PhysAddr,
    /// Virtual address (if mapped)
    pub virt_addr: Option<VirtAddr>,
    /// Size of the huge page
    pub size: PageSize,
    /// Number of standard pages this huge page represents
    pub page_count: u64,
    /// Whether this huge page is currently mapped
    pub mapped: bool,
    /// Reference count for shared huge pages
    pub ref_count: u32,
}

impl HugePage {
    /// Create a new huge page descriptor
    pub fn new(phys_addr: PhysAddr, size: PageSize) -> Self {
        Self {
            phys_addr,
            virt_addr: None,
            size,
            page_count: size.page_count(),
            mapped: false,
            ref_count: 1,
        }
    }

    /// Get the physical address
    pub fn phys_addr(&self) -> PhysAddr {
        self.phys_addr
    }

    /// Get the size
    pub fn size(&self) -> PageSize {
        self.size
    }

    /// Get the virtual address (if mapped)
    pub fn virt_addr(&self) -> Option<VirtAddr> {
        self.virt_addr
    }

    /// Set the virtual address
    pub fn set_virt_addr(&mut self, virt_addr: VirtAddr) {
        self.virt_addr = Some(virt_addr);
        self.mapped = true;
    }

    /// Clear the virtual address (unmap)
    pub fn clear_virt_addr(&mut self) {
        self.virt_addr = None;
        self.mapped = false;
    }

    /// Check if this is a valid huge page
    pub fn is_valid(&self) -> bool {
        self.size.is_aligned(self.phys_addr) && self.size != PageSize::Size4K
    }

    /// Increment reference count
    pub fn inc_ref(&mut self) {
        self.ref_count += 1;
    }

    /// Decrement reference count
    pub fn dec_ref(&mut self) -> bool {
        if self.ref_count > 0 {
            self.ref_count -= 1;
        }
        self.ref_count == 0
    }

    /// Get reference count
    pub fn ref_count(&self) -> u32 {
        self.ref_count
    }
}

/// Huge page manager
pub struct HugePageManager {
    /// List of allocated huge pages
    huge_pages: SpinLock<heapless::Vec<HugePage, 256>>,
    /// Huge page statistics
    stats: SpinLock<HugePageStats>,
    /// Default huge page size
    default_size: PageSize,
}

/// Huge page statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct HugePageStats {
    /// Total huge pages allocated
    pub total_huge_pages: u64,
    /// 2MB huge pages
    pub huge_2mb_pages: u64,
    /// 1GB huge pages
    pub huge_1gb_pages: u64,
    /// Memory saved by using huge pages (in bytes)
    pub memory_saved: u64,
    /// Huge page allocation failures
    pub allocation_failures: u64,
    /// TLB entries saved
    pub tlb_entries_saved: u64,
}

impl HugePageManager {
    /// Create a new huge page manager
    pub const fn new() -> Self {
        Self {
            huge_pages: SpinLock::new(heapless::Vec::new()),
            stats: SpinLock::new(HugePageStats::default()),
            default_size: PageSize::Size2M,
        }
    }

    /// Allocate a huge page
    pub fn allocate_huge_page(&self, size: PageSize) -> Result<NonNull<HugePage>, crate::Error> {
        if size == PageSize::Size4K {
            return Err(crate::Error::InvalidArgument);
        }

        // Allocate contiguous frames
        let page_count = size.page_count();
        let phys_addr = alloc_contiguous_frames(page_count)
            .ok_or(crate::Error::OutOfMemory)?;

        // Check alignment
        if !size.is_aligned(phys_addr) {
            // Free the frames and return error
            dealloc_contiguous_frames(phys_addr, page_count);
            let mut stats = self.stats.lock();
            stats.allocation_failures += 1;
            return Err(crate::Error::InvalidArgument);
        }

        // Create huge page descriptor
        let huge_page = HugePage::new(phys_addr, size);

        // Add to list
        let mut pages = self.huge_pages.lock();
        if pages.push(huge_page.clone()).is_err() {
            // List is full, clean up
            dealloc_contiguous_frames(phys_addr, page_count);
            let mut stats = self.stats.lock();
            stats.allocation_failures += 1;
            return Err(crate::Error::OutOfMemory);
        }

        // Update statistics
        let mut stats = self.stats.lock();
        stats.total_huge_pages += 1;
        match size {
            PageSize::Size2M => stats.huge_2mb_pages += 1,
            PageSize::Size1G => stats.huge_1gb_pages += 1,
            _ => {}
        }

        // Calculate memory saved (reduced TLB pressure)
        let tlb_entries_saved = page_count - 1;
        stats.tlb_entries_saved += tlb_entries_saved;

        // Return pointer to the huge page
        let ptr = pages.last_mut().unwrap() as *mut HugePage;
        Ok(unsafe { NonNull::new_unchecked(ptr) })
    }

    /// Free a huge page
    pub fn free_huge_page(&self, huge_page: &mut HugePage) -> Result<(), crate::Error> {
        if !huge_page.is_valid() {
            return Err(crate::Error::InvalidArgument);
        }

        // Check reference count
        if huge_page.dec_ref() {
            // Last reference, free the memory
            dealloc_contiguous_frames(huge_page.phys_addr, huge_page.page_count);

            // Remove from list
            let mut pages = self.huge_pages.lock();
            if let Some(pos) = pages.iter().position(|p| p.phys_addr == huge_page.phys_addr) {
                pages.remove(pos);
            }

            // Update statistics
            let mut stats = self.stats.lock();
            stats.total_huge_pages -= 1;
            match huge_page.size {
                PageSize::Size2M => stats.huge_2mb_pages -= 1,
                PageSize::Size1G => stats.huge_1gb_pages -= 1,
                _ => {}
            }
            stats.tlb_entries_saved = stats.tlb_entries_saved.saturating_sub(huge_page.page_count - 1);
        }

        Ok(())
    }

    /// Find a huge page by physical address
    pub fn find_huge_page(&self, phys_addr: PhysAddr) -> Option<*mut HugePage> {
        let pages = self.huge_pages.lock();
        for page in pages.iter() {
            if phys_addr >= page.phys_addr && phys_addr < page.phys_addr + page.size.size() {
                // Return a mutable pointer (this is safe as we have exclusive access)
                return Some(page as *const HugePage as *mut HugePage);
            }
        }
        None
    }

    /// Split a huge page into standard pages
    pub fn split_huge_page(&self, huge_page: &mut HugePage) -> Result<(), crate::Error> {
        if huge_page.size == PageSize::Size4K {
            return Err(crate::Error::InvalidArgument);
        }

        if huge_page.mapped {
            return Err(crate::Error::InvalidState);
        }

        // This is a simplified implementation - in practice, we would need to
        // split the contiguous allocation into individual page allocations
        // For now, we'll just deallocate the huge page and let the caller
        // allocate standard pages

        // Update statistics
        let mut stats = self.stats.lock();
        stats.total_huge_pages -= 1;
        match huge_page.size {
            PageSize::Size2M => stats.huge_2mb_pages -= 1,
            PageSize::Size1G => stats.huge_1gb_pages -= 1,
            _ => {}
        }
        stats.tlb_entries_saved = stats.tlb_entries_saved.saturating_sub(huge_page.page_count - 1);

        // Free the contiguous frames
        dealloc_contiguous_frames(huge_page.phys_addr, huge_page.page_count);

        // Remove from list
        let mut pages = self.huge_pages.lock();
        if let Some(pos) = pages.iter().position(|p| p.phys_addr == huge_page.phys_addr) {
            pages.remove(pos);
        }

        Ok(())
    }

    /// Check if we should use huge pages for a given allocation
    pub fn should_use_huge_pages(&self, size: u64) -> bool {
        size >= self.default_size.size()
    }

    /// Get the optimal page size for a given allocation
    pub fn optimal_page_size(&self, size: u64) -> PageSize {
        if size >= PageSize::Size1G.size() && PageSize::Size1G.is_aligned(size) {
            PageSize::Size1G
        } else if size >= self.default_size.size() && self.default_size.is_aligned(size) {
            self.default_size
        } else {
            PageSize::Size4K
        }
    }

    /// Get huge page statistics
    pub fn get_stats(&self) -> HugePageStats {
        *self.stats.lock()
    }

    /// Set default huge page size
    pub fn set_default_size(&mut self, size: PageSize) {
        if size != PageSize::Size4K {
            self.default_size = size;
        }
    }

    /// Get default huge page size
    pub fn default_size(&self) -> PageSize {
        self.default_size
    }

    /// Estimate memory savings from huge pages
    pub fn estimate_memory_savings(&self) -> u64 {
        let stats = self.stats.lock();

        // Rough estimation: Each huge page saves (page_count - 1) TLB entries
        // Assume each TLB entry saves about 64 bytes of metadata overhead
        stats.tlb_entries_saved * 64
    }
}

/// Global huge page manager
static mut HUGE_PAGE_MANAGER: HugePageManager = HugePageManager::new();

/// Get the global huge page manager
pub fn get_huge_page_manager() -> &'static HugePageManager {
    unsafe { &HUGE_PAGE_MANAGER }
}

/// Initialize huge page management
pub fn init_huge_page_manager() -> Result<(), crate::Error> {
    crate::info!("Initializing huge page manager");

    let manager = get_huge_page_manager();
    let stats = manager.get_stats();

    crate::info!("Huge page manager initialized - default size: {} bytes",
                 manager.default_size().size());

    Ok(())
}

/// Allocate a huge page with the specified size
pub fn alloc_huge_page(size: PageSize) -> Result<NonNull<HugePage>, crate::Error> {
    get_huge_page_manager().allocate_huge_page(size)
}

/// Free a huge page
pub fn free_huge_page(huge_page: &mut HugePage) -> Result<(), crate::Error> {
    get_huge_page_manager().free_huge_page(huge_page)
}

/// Check if an address range can be covered by huge pages
pub fn can_use_huge_pages(start_addr: u64, size: u64) -> Option<PageSize> {
    let end_addr = start_addr + size;

    // Check 1GB alignment first
    if PageSize::Size1G.is_aligned(start_addr) &&
       PageSize::Size1G.is_aligned(end_addr) &&
       size >= PageSize::Size1G.size() {
        return Some(PageSize::Size1G);
    }

    // Check 2MB alignment
    if PageSize::Size2M.is_aligned(start_addr) &&
       PageSize::Size2M.is_aligned(end_addr) &&
       size >= PageSize::Size2M.size() {
        return Some(PageSize::Size2M);
    }

    None
}

/// Optimize a memory region using huge pages where possible
pub fn optimize_with_huge_pages(
    start_addr: VirtAddr,
    size: u64,
) -> Result<(Vec<(VirtAddr, PageSize)>, u64), crate::Error> {
    let mut regions = Vec::new();
    let mut current_addr = start_addr;
    let mut remaining = size;
    let mut huge_pages_used = 0;

    while remaining > 0 {
        // Find the largest huge page we can use at this address
        if let Some(page_size) = can_use_huge_pages(current_addr, remaining) {
            let page_size_bytes = page_size.size();

            // Allocate huge page
            let huge_page = alloc_huge_page(page_size)?;

            // Add to regions
            regions.push((current_addr, page_size));
            huge_pages_used += 1;

            current_addr += page_size_bytes;
            remaining -= page_size_bytes;
        } else {
            // Fall back to standard pages
            let standard_size = remaining.min(PAGE_SIZE);
            regions.push((current_addr, PageSize::Size4K));

            current_addr += standard_size;
            remaining -= standard_size;
        }
    }

    Ok((regions, huge_pages_used))
}

/// Convert between different page sizes
pub fn convert_page_size(
    from_size: PageSize,
    to_size: PageSize,
    addr: u64,
) -> Result<u64, crate::Error> {
    if from_size == to_size {
        return Ok(addr);
    }

    // Align address to source page size
    let aligned_addr = from_size.align_down(addr);

    // Check if address is properly aligned for target size
    if !to_size.is_aligned(aligned_addr) {
        return Err(crate::Error::InvalidArgument);
    }

    Ok(aligned_addr)
}