//! Virtual memory management
//!
//! Provides page table management and virtual address space operations.

use crate::core::mm::{
    VirtAddr, PhysAddr, PageNr, FrameNr, PAGE_SIZE, PAGE_SHIFT,
    PageFlags, AddressSpaceType, align_up, align_down, flush_tlb_addr,
};
use crate::core::mm::frame::{alloc_frame, dealloc_frame};
use crate::core::sync::SpinLock;
use core::ptr::NonNull;

// Simple logging macros for no_std environment
macro_rules! cow_info {
    ($($arg:tt)*) => ({
        // In a real implementation, this would output to UART or other debug console
        #[cfg(debug_assertions)]
        {
            // For now, just use a no-op
        }
    });
}

/// Copy-on-Write page tracking
#[derive(Debug, Clone, Copy)]
pub struct CowPage {
    /// Original physical frame that is shared
    pub original_frame: PhysAddr,
    /// Reference count of how many mappings share this page
    pub ref_count: u32,
    /// Whether this page has been copied (break COW)
    pub copied: bool,
}

impl CowPage {
    pub fn new(frame: PhysAddr) -> Self {
        Self {
            original_frame: frame,
            ref_count: 1,
            copied: false,
        }
    }

    pub fn increment_ref(&mut self) {
        self.ref_count += 1;
    }

    pub fn decrement_ref(&mut self) -> bool {
        if self.ref_count > 0 {
            self.ref_count -= 1;
        }
        self.ref_count == 0
    }
}

/// COW memory manager
pub struct CowManager {
    /// Track COW pages by original frame address
    cow_pages: SpinLock<heapless::FnvIndexMap<PhysAddr, CowPage, 1024>>,
    /// COW statistics
    stats: SpinLock<CowStats>,
}

/// COW operation statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct CowStats {
    /// Total COW pages allocated
    pub cow_pages: u64,
    /// Pages copied on write (COW breaks)
    pub cow_breaks: u64,
    /// Memory saved by COW (in pages)
    pub memory_saved: u64,
    /// Total write faults handled
    pub write_faults: u64,
}

impl CowManager {
    pub const fn new() -> Self {
        Self {
            cow_pages: SpinLock::new(heapless::FnvIndexMap::new()),
            stats: SpinLock::new(CowStats::default()),
        }
    }

    /// Register a page for COW tracking
    pub fn register_cow_page(&self, original_frame: PhysAddr) -> Result<(), crate::Error> {
        let mut cow_pages = self.cow_pages.lock();

        if let Some(cow_page) = cow_pages.get_mut(&original_frame) {
            cow_page.increment_ref();
        } else {
            let new_cow_page = CowPage::new(original_frame);
            cow_pages.insert(original_frame, new_cow_page).map_err(|_| crate::Error::OutOfMemory)?;
        }

        // Update statistics
        let mut stats = self.stats.lock();
        stats.cow_pages += 1;
        stats.memory_saved += 1;

        Ok(())
    }

    /// Handle a write fault on a COW page
    pub fn handle_write_fault(&self, frame: PhysAddr) -> Result<PhysAddr, crate::Error> {
        let mut cow_pages = self.cow_pages.lock();
        let mut stats = self.stats.lock();

        stats.write_faults += 1;

        if let Some(cow_page) = cow_pages.get_mut(&frame) {
            if !cow_page.copied {
                // Need to copy the page
                let new_frame = alloc_frame().ok_or(crate::Error::OutOfMemory)?;

                // Copy page content
                unsafe {
                    let src = frame as *const u8;
                    let dst = new_frame as *mut u8;
                    core::ptr::copy_nonoverlapping(src, dst, PAGE_SIZE as usize);
                }

                // Update COW tracking
                cow_page.copied = true;
                cow_page.decrement_ref();
                stats.cow_breaks += 1;

                // If reference count is zero, remove from tracking
                if cow_page.ref_count == 0 {
                    cow_pages.remove(&frame);
                    stats.memory_saved = stats.memory_saved.saturating_sub(1);
                }

                Ok(new_frame)
            } else {
                // Page was already copied
                Ok(frame)
            }
        } else {
            // Not a COW page, return original
            Ok(frame)
        }
    }

    /// Check if a frame is COW-enabled
    pub fn is_cow_page(&self, frame: PhysAddr) -> bool {
        let cow_pages = self.cow_pages.lock();
        cow_pages.contains_key(&frame)
    }

    /// Get COW statistics
    pub fn get_stats(&self) -> CowStats {
        *self.stats.lock()
    }

    /// Unregister a COW page
    pub fn unregister_cow_page(&self, frame: PhysAddr) -> Result<(), crate::Error> {
        let mut cow_pages = self.cow_pages.lock();
        let mut stats = self.stats.lock();

        if let Some(cow_page) = cow_pages.get_mut(&frame) {
            if cow_page.decrement_ref() {
                cow_pages.remove(&frame);
                stats.memory_saved = stats.memory_saved.saturating_sub(1);
            }
        }

        Ok(())
    }
}

/// Global COW manager
static COW_MANAGER: CowManager = CowManager::new();

/// Get the global COW manager
pub fn get_cow_manager() -> &'static CowManager {
    &COW_MANAGER
}

/// Page table entry size (in bytes)
pub const PT_ENTRY_SIZE: usize = 8;

/// Number of entries per page table level
pub const PT_ENTRIES: usize = 512;

/// Bits for each level index
pub const PT_SHIFT: u32 = 9;

/// Virtual address space bits
#[cfg(target_arch = "aarch64")]
pub const VA_BITS: u32 = 48;

#[cfg(target_arch = "riscv64")]
pub const VA_BITS: u32 = 48;

#[cfg(target_arch = "x86_64")]
pub const VA_BITS: u32 = 48;

/// Maximum virtual address
pub const MAX_VIRT_ADDR: u64 = (1u64 << VA_BITS) - 1;

/// Page table levels
pub const PT_LEVELS: usize = 4;

/// Page table
pub struct PageTable {
    /// Physical address of the page table
    phys_addr: PhysAddr,
    /// Virtual address of the page table (if mapped)
    virt_addr: Option<VirtAddr>,
    /// Array of entries
    entries: [u64; PT_ENTRIES],
}

impl PageTable {
    /// Create a new page table
    pub fn new() -> Option<NonNull<Self>> {
        let frame = alloc_frame()?;
        let pt_ptr = frame as *mut Self;

        // Zero initialize the page table
        unsafe {
            (*pt_ptr).phys_addr = frame;
            (*pt_ptr).virt_addr = None;
            (*pt_ptr).entries = [0; PT_ENTRIES];
        }

        Some(unsafe { NonNull::new_unchecked(pt_ptr) })
    }

    /// Get the physical address of this page table
    pub fn phys_addr(&self) -> PhysAddr {
        self.phys_addr
    }

    /// Set the virtual address mapping for this page table
    pub fn set_virt_addr(&mut self, virt_addr: VirtAddr) {
        self.virt_addr = Some(virt_addr);
    }

    /// Get an entry from the page table
    pub fn entry(&self, index: usize) -> u64 {
        self.entries[index]
    }

    /// Set an entry in the page table
    pub fn set_entry(&mut self, index: usize, entry: u64) {
        self.entries[index] = entry;
    }

    /// Clear an entry in the page table
    pub fn clear_entry(&mut self, index: usize) {
        self.entries[index] = 0;
    }

    /// Check if an entry is present
    pub fn is_present(&self, index: usize) -> bool {
        (self.entries[index] & 0x1) != 0
    }

    /// Extract the frame address from an entry
    pub fn entry_frame_addr(&self, index: usize) -> PhysAddr {
        (self.entries[index] & !0xFFF) // Clear lower 12 bits
    }
}

/// Address space
pub struct AddressSpace {
    /// Root page table
    root_pt: NonNull<PageTable>,
    /// Type of address space
    kind: AddressSpaceType,
    /// Address space ID
    asid: u16,
    /// Current virtual memory usage
    used_virt: u64,
    /// Lock for thread safety
    lock: SpinLock<()>,
}

impl AddressSpace {
    /// Create a new address space
    pub fn new(kind: AddressSpaceType) -> Option<Self> {
        let root_pt = PageTable::new()?;

        Some(Self {
            root_pt,
            kind,
            asid: 0, // TODO: Allocate ASID
            used_virt: 0,
            lock: SpinLock::new(()),
        })
    }

    /// Get the root page table
    pub fn root_page_table(&self) -> NonNull<PageTable> {
        self.root_pt
    }

    /// Get the address space type
    pub fn kind(&self) -> AddressSpaceType {
        self.kind
    }

    /// Get the address space ID
    pub fn asid(&self) -> u16 {
        self.asid
    }

    /// Map a page in this address space
    pub fn map_page(
        &self,
        virt_addr: VirtAddr,
        phys_addr: PhysAddr,
        flags: PageFlags,
    ) -> Result<(), crate::Error> {
        let _guard = self.lock.lock();

        // Ensure addresses are page-aligned
        let virt_page = align_down(virt_addr);
        let phys_frame = align_down(phys_addr);

        // Extract indices for each level
        let indices = [
            ((virt_addr >> (PAGE_SHIFT + PT_SHIFT * 3)) & (PT_ENTRIES as u64 - 1)) as usize,
            ((virt_addr >> (PAGE_SHIFT + PT_SHIFT * 2)) & (PT_ENTRIES as u64 - 1)) as usize,
            ((virt_addr >> (PAGE_SHIFT + PT_SHIFT * 1)) & (PT_ENTRIES as u64 - 1)) as usize,
            ((virt_addr >> PAGE_SHIFT) & (PT_ENTRIES as u64 - 1)) as usize,
        ];

        // Walk or create the page table hierarchy
        let mut current_pt = self.root_pt;

        for level in 0..PT_LEVELS - 1 {
            let pt_ref = unsafe { current_pt.as_ref() };

            if !pt_ref.is_present(indices[level]) {
                // Need to allocate a new page table
                let new_pt = PageTable::new().ok_or(crate::Error::OutOfMemory)?;
                let new_pt_ref = unsafe { new_pt.as_ref() };

                // Create entry with appropriate flags
                let mut entry = new_pt_ref.phys_addr();
                if flags.writable {
                    entry |= 0x2; // Writable bit
                }
                if flags.user {
                    entry |= 0x4; // User bit
                }

                // Set the entry in current page table
                unsafe {
                    let current_pt_mut = current_pt.as_mut();
                    current_pt_mut.set_entry(indices[level], entry | 0x1); // Present bit
                }
            }

            // Move to next level
            let next_pt_addr = unsafe { current_pt.as_ref().entry_frame_addr(indices[level]) };
            current_pt = unsafe { NonNull::new_unchecked(next_pt_addr as *mut PageTable) };
        }

        // Final level - create the page mapping
        let final_pt_ref = unsafe { current_pt.as_ref() };
        if final_pt_ref.is_present(indices[PT_LEVELS - 1]) {
            return Err(crate::Error::InvalidArgument); // Already mapped
        }

        // Create final page entry
        let mut entry = phys_frame;
        if flags.writable {
            entry |= 0x2;
        }
        if flags.user {
            entry |= 0x4;
        }
        if !flags.executable {
            entry |= 0x8000000000000000u64; // NX bit (x86_64) / XN bit (ARM64)
        }

        // Handle COW-specific flags
        if flags.cow {
            entry |= 0x200; // Architecture-specific COW/dirty bit
        }

        unsafe {
            let final_pt_mut = current_pt.as_mut();
            final_pt_mut.set_entry(indices[PT_LEVELS - 1], entry | 0x1);
        }

        Ok(())
    }

    /// Unmap a page from this address space
    pub fn unmap_page(&self, virt_addr: VirtAddr) -> Result<PhysAddr, crate::Error> {
        let _guard = self.lock.lock();

        let virt_page = align_down(virt_addr);
        let indices = [
            ((virt_addr >> (PAGE_SHIFT + PT_SHIFT * 3)) & (PT_ENTRIES as u64 - 1)) as usize,
            ((virt_addr >> (PAGE_SHIFT + PT_SHIFT * 2)) & (PT_ENTRIES as u64 - 1)) as usize,
            ((virt_addr >> (PAGE_SHIFT + PT_SHIFT * 1)) & (PT_ENTRIES as u64 - 1)) as usize,
            ((virt_addr >> PAGE_SHIFT) & (PT_ENTRIES as u64 - 1)) as usize,
        ];

        // Walk the page table hierarchy
        let mut current_pt = self.root_pt;

        for level in 0..PT_LEVELS {
            let pt_ref = unsafe { current_pt.as_ref() };

            if !pt_ref.is_present(indices[level]) {
                return Err(crate::Error::NotFound);
            }

            if level == PT_LEVELS - 1 {
                // Final level - get the physical address and clear the entry
                let phys_addr = pt_ref.entry_frame_addr(indices[level]);
                unsafe {
                    let current_pt_mut = current_pt.as_mut();
                    current_pt_mut.clear_entry(indices[level]);
                }

                // TODO: Recursively free empty page tables
                return Ok(phys_addr);
            }

            // Move to next level
            let next_pt_addr = pt_ref.entry_frame_addr(indices[level]);
            current_pt = unsafe { NonNull::new_unchecked(next_pt_addr as *mut PageTable) };
        }

        Err(crate::Error::InvalidState)
    }

    /// Map a range of pages
    pub fn map_range(
        &self,
        virt_addr: VirtAddr,
        phys_addr: PhysAddr,
        size: u64,
        flags: PageFlags,
    ) -> Result<(), crate::Error> {
        let aligned_virt = align_down(virt_addr);
        let aligned_phys = align_down(phys_addr);
        let aligned_size = align_up(size + (virt_addr - aligned_virt));

        let num_pages = aligned_size / PAGE_SIZE;

        for i in 0..num_pages {
            let vaddr = aligned_virt + i * PAGE_SIZE;
            let paddr = aligned_phys + i * PAGE_SIZE;
            self.map_page(vaddr, paddr, flags)?;
        }

        Ok(())
    }

    /// Change protection flags for a page
    pub fn protect_page(
        &self,
        virt_addr: VirtAddr,
        flags: PageFlags,
    ) -> Result<(), crate::Error> {
        let _guard = self.lock.lock();

        let virt_page = align_down(virt_addr);
        let indices = [
            ((virt_page >> (PAGE_SHIFT + PT_SHIFT * 3)) & (PT_ENTRIES as u64 - 1)) as usize,
            ((virt_page >> (PAGE_SHIFT + PT_SHIFT * 2)) & (PT_ENTRIES as u64 - 1)) as usize,
            ((virt_page >> (PAGE_SHIFT + PT_SHIFT * 1)) & (PT_ENTRIES as u64 - 1)) as usize,
            ((virt_page >> PAGE_SHIFT) & (PT_ENTRIES as u64 - 1)) as usize,
        ];

        // Walk to the final level
        let mut current_pt = self.root_pt;

        for level in 0..PT_LEVELS {
            let pt_ref = unsafe { current_pt.as_ref() };

            if !pt_ref.is_present(indices[level]) {
                return Err(crate::Error::NotFound);
            }

            if level == PT_LEVELS - 1 {
                // Update the entry flags
                let current_entry = pt_ref.entry(indices[level]);
                let frame_addr = current_entry & !0xFFF;

                let mut new_entry = frame_addr;
                if flags.writable {
                    new_entry |= 0x2;
                }
                if flags.user {
                    new_entry |= 0x4;
                }
                if !flags.executable {
                    new_entry |= 0x8000000000000000u64;
                }
                new_entry |= 0x1; // Present bit

                unsafe {
                    let current_pt_mut = current_pt.as_mut();
                    current_pt_mut.set_entry(indices[level], new_entry);
                }

                return Ok(());
            }

            // Move to next level
            let next_pt_addr = pt_ref.entry_frame_addr(indices[level]);
            current_pt = unsafe { NonNull::new_unchecked(next_pt_addr as *mut PageTable) };
        }

        Err(crate::Error::InvalidState)
    }

    /// Get the physical address mapped to a virtual address
    pub fn translate(&self, virt_addr: VirtAddr) -> Option<PhysAddr> {
        let _guard = self.lock.lock();

        let virt_page = align_down(virt_addr);
        let page_offset = virt_addr - virt_page;
        let indices = [
            ((virt_page >> (PAGE_SHIFT + PT_SHIFT * 3)) & (PT_ENTRIES as u64 - 1)) as usize,
            ((virt_page >> (PAGE_SHIFT + PT_SHIFT * 2)) & (PT_ENTRIES as u64 - 1)) as usize,
            ((virt_page >> (PAGE_SHIFT + PT_SHIFT * 1)) & (PT_ENTRIES as u64 - 1)) as usize,
            ((virt_page >> PAGE_SHIFT) & (PT_ENTRIES as u64 - 1)) as usize,
        ];

        // Walk the page table hierarchy
        let mut current_pt = self.root_pt;

        for level in 0..PT_LEVELS {
            let pt_ref = unsafe { current_pt.as_ref() };

            if !pt_ref.is_present(indices[level]) {
                return None;
            }

            if level == PT_LEVELS - 1 {
                // Final level - return the physical address
                let phys_page = pt_ref.entry_frame_addr(indices[level]);
                return Some(phys_page + page_offset);
            }

            // Move to next level
            let next_pt_addr = pt_ref.entry_frame_addr(indices[level]);
            current_pt = unsafe { NonNull::new_unchecked(next_pt_addr as *mut PageTable) };
        }

        None
    }

    /// Map a page with copy-on-write protection
    pub fn map_cow_page(
        &self,
        virt_addr: VirtAddr,
        phys_addr: PhysAddr,
    ) -> Result<(), crate::Error> {
        let _guard = self.lock.lock();

        // Register the page for COW tracking
        get_cow_manager().register_cow_page(phys_addr)?;

        // Map with COW flags (read-only, write-protected)
        self.map_page_internal(virt_addr, phys_addr, PageFlags::cow())
    }

    /// Handle a write fault on a COW page
    pub fn handle_cow_fault(&self, virt_addr: VirtAddr) -> Result<(), crate::Error> {
        let _guard = self.lock.lock();

        // Get the current physical frame
        let current_frame = self.translate(virt_addr)
            .ok_or(crate::Error::NotFound)?;

        // Handle COW write fault
        let new_frame = get_cow_manager().handle_write_fault(current_frame)?;

        // Update page table entry with new frame and writable flags
        self.map_page_internal(virt_addr, new_frame, PageFlags::cow_writable())?;

        // Invalidate TLB for this address
        flush_tlb_addr(virt_addr);

        Ok(())
    }

    /// Share a memory region with COW between two address spaces
    pub fn share_region_cow(
        &self,
        other: &AddressSpace,
        virt_addr: VirtAddr,
        size: u64,
    ) -> Result<(), crate::Error> {
        let aligned_virt = align_down(virt_addr);
        let aligned_size = align_up(size + (virt_addr - aligned_virt));
        let num_pages = aligned_size / PAGE_SIZE;

        for i in 0..num_pages {
            let vaddr = aligned_virt + i * PAGE_SIZE;

            // Get the physical frame from this address space
            let phys_frame = self.translate(vaddr)
                .ok_or(crate::Error::NotFound)?;

            // Map in the other address space with COW protection
            other.map_cow_page(vaddr, phys_frame)?;
        }

        Ok(())
    }

    /// Internal map_page method to avoid double locking
    fn map_page_internal(
        &self,
        virt_addr: VirtAddr,
        phys_addr: PhysAddr,
        flags: PageFlags,
    ) -> Result<(), crate::Error> {
        // Ensure addresses are page-aligned
        let virt_page = align_down(virt_addr);
        let phys_frame = align_down(phys_addr);

        // Extract indices for each level
        let indices = [
            ((virt_addr >> (PAGE_SHIFT + PT_SHIFT * 3)) & (PT_ENTRIES as u64 - 1)) as usize,
            ((virt_addr >> (PAGE_SHIFT + PT_SHIFT * 2)) & (PT_ENTRIES as u64 - 1)) as usize,
            ((virt_addr >> (PAGE_SHIFT + PT_SHIFT * 1)) & (PT_ENTRIES as u64 - 1)) as usize,
            ((virt_addr >> PAGE_SHIFT) & (PT_ENTRIES as u64 - 1)) as usize,
        ];

        // Walk or create the page table hierarchy
        let mut current_pt = self.root_pt;

        for level in 0..PT_LEVELS - 1 {
            let pt_ref = unsafe { current_pt.as_ref() };

            if !pt_ref.is_present(indices[level]) {
                // Need to allocate a new page table
                let new_pt = PageTable::new().ok_or(crate::Error::OutOfMemory)?;
                let new_pt_ref = unsafe { new_pt.as_ref() };

                // Create entry with appropriate flags
                let mut entry = new_pt_ref.phys_addr();
                if flags.writable {
                    entry |= 0x2; // Writable bit
                }
                if flags.user {
                    entry |= 0x4; // User bit
                }

                // Set the entry in current page table
                unsafe {
                    let current_pt_mut = current_pt.as_mut();
                    current_pt_mut.set_entry(indices[level], entry | 0x1); // Present bit
                }
            }

            // Move to next level
            let next_pt_addr = unsafe { current_pt.as_ref().entry_frame_addr(indices[level]) };
            current_pt = unsafe { NonNull::new_unchecked(next_pt_addr as *mut PageTable) };
        }

        // Final level - create the page mapping
        let final_pt_ref = unsafe { current_pt.as_ref() };
        if final_pt_ref.is_present(indices[PT_LEVELS - 1]) {
            // Update existing mapping
            unsafe {
                let current_pt_mut = current_pt.as_mut();
                current_pt_mut.clear_entry(indices[PT_LEVELS - 1]);
            }
        }

        // Create final page entry
        let mut entry = phys_frame;
        if flags.writable {
            entry |= 0x2;
        }
        if flags.user {
            entry |= 0x4;
        }
        if !flags.executable {
            entry |= 0x8000000000000000u64; // NX bit (x86_64) / XN bit (ARM64)
        }

        // Handle COW-specific flags
        if flags.cow {
            entry |= 0x200; // Architecture-specific COW/dirty bit
        }

        unsafe {
            let final_pt_mut = current_pt.as_mut();
            final_pt_mut.set_entry(indices[PT_LEVELS - 1], entry | 0x1);
        }

        Ok(())
    }

    /// Get COW statistics for this address space
    pub fn get_cow_stats(&self) -> crate::core::mm::page::CowStats {
        get_cow_manager().get_stats()
    }
}

/// Initialize virtual memory management
pub fn init() -> Result<(), crate::Error> {
    // TODO: Initialize kernel address space
    Err(crate::Error::NotImplemented)
}

/// Create the kernel address space
pub fn create_kernel_space() -> Option<AddressSpace> {
    AddressSpace::new(AddressSpaceType::Kernel)
}

/// Create a user/guest address space
pub fn create_user_space() -> Option<AddressSpace> {
    AddressSpace::new(AddressSpaceType::User)
}

/// Convert virtual address to page indices
pub fn virt_to_indices(virt_addr: VirtAddr) -> [usize; 4] {
    [
        ((virt_addr >> (PAGE_SHIFT + PT_SHIFT * 3)) & (PT_ENTRIES as u64 - 1)) as usize,
        ((virt_addr >> (PAGE_SHIFT + PT_SHIFT * 2)) & (PT_ENTRIES as u64 - 1)) as usize,
        ((virt_addr >> (PAGE_SHIFT + PT_SHIFT * 1)) & (PT_ENTRIES as u64 - 1)) as usize,
        ((virt_addr >> PAGE_SHIFT) & (PT_ENTRIES as u64 - 1)) as usize,
    ]
}

/// Calculate the page table entry for a physical address with flags
pub fn make_pt_entry(phys_addr: PhysAddr, flags: PageFlags) -> u64 {
    let mut entry = align_down(phys_addr);

    if flags.writable {
        entry |= 0x2;
    }
    if flags.user {
        entry |= 0x4;
    }
    if !flags.executable {
        entry |= 0x8000000000000000u64;
    }

    // Handle COW-specific flags
    if flags.cow {
        entry |= 0x200; // Architecture-specific COW/dirty bit
    }

    entry | 0x1 // Present bit
}

/// Initialize COW memory management
pub fn init_cow() -> Result<(), crate::Error> {
    cow_info!("Initializing copy-on-write memory management");

    // Initialize COW manager
    let _stats = get_cow_manager().get_stats();

    cow_info!("Copy-on-write memory management initialized");
    Ok(())
}

/// Handle a memory write fault (potentially COW)
pub fn handle_write_fault(
    addr_space: &AddressSpace,
    virt_addr: VirtAddr,
) -> Result<(), crate::Error> {
    // Check if this is a COW page
    if let Some(phys_frame) = addr_space.translate(virt_addr) {
        if get_cow_manager().is_cow_page(phys_frame) {
            return addr_space.handle_cow_fault(virt_addr);
        }
    }

    // Not a COW fault, return error
    Err(crate::Error::InvalidState)
}

/// Optimize memory sharing using COW
pub fn optimize_memory_sharing(
    source_space: &AddressSpace,
    dest_space: &AddressSpace,
    start_addr: VirtAddr,
    size: u64,
) -> Result<u64, crate::Error> {
    let aligned_start = align_down(start_addr);
    let aligned_size = align_up(size + (start_addr - aligned_start));
    let num_pages = aligned_size / PAGE_SIZE;
    let mut shared_pages = 0;

    for i in 0..num_pages {
        let vaddr = aligned_start + i * PAGE_SIZE;

        // Check if both spaces have the same physical page
        if let (Some(src_frame), Some(dst_frame)) = (
            source_space.translate(vaddr),
            dest_space.translate(vaddr),
        ) {
            if src_frame == dst_frame {
                // Pages are already shared
                shared_pages += 1;
                continue;
            }
        }

        // Try to share with COW
        if let Some(src_frame) = source_space.translate(vaddr) {
            if dest_space.map_cow_page(vaddr, src_frame).is_ok() {
                shared_pages += 1;
            }
        }
    }

    Ok(shared_pages)
}

/// Invalidate TLB entries for a specific address space
pub fn invalidate_tlb_asid(asid: u16) {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        core::arch::asm!("tlbi aside1is, {}", in(reg) (asid as u64));
        core::arch::asm!("dsb ish");
        core::arch::asm!("isb");
    }

    #[cfg(target_arch = "riscv64")]
    {
        // RISC-V doesn't have ASID-based TLB invalidation
        // Use full flush
        unsafe {
            core::arch::asm!("sfence.vma");
        }
    }

    #[cfg(target_arch = "x86_64")]
    {
        // x86_64 doesn't have ASID in hardware
        x86_64::instructions::tlb::flush_all();
    }
}