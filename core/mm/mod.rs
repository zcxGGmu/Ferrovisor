//! Memory management module
//!
//! This module provides memory management capabilities for the hypervisor,
//! including physical memory allocation, virtual memory management, and heap management.

use crate::Result;

pub mod frame;
pub mod page;
pub mod heap;
pub mod slab;
pub mod buddy;
pub mod allocator;
pub mod hugepage;
pub mod gstage;

// Re-export commonly used types
pub use page::{AddressSpace, AddressSpaceType};
pub use page::{CowPage, CowStats, CowManager, get_cow_manager, init_cow, handle_write_fault, optimize_memory_sharing};
pub use hugepage::{HugePage, HugePageManager, HugePageStats, PageSize, default_huge_page_size, default_huge_page_shift};
pub use hugepage::{get_huge_page_manager, init_huge_page_manager, alloc_huge_page, free_huge_page, can_use_huge_pages, optimize_with_huge_pages};
pub use gstage::{GStageContext, GStageManager, GStagePageTable, GStagePte, GStageMode, GStageLevel};
pub use gstage::{Gva, Gpa, Hpa, Vmid, init as init_gstage, get as get_gstage_manager, get_expect as get_gstage_manager_expect};
pub use gstage::gstage_pte;
pub use gstage::flags as gstage_flags;

/// Physical address type
pub type PhysAddr = u64;

/// Virtual address type
pub type VirtAddr = u64;

/// Page number type
pub type PageNr = u64;

/// Frame number type
pub type FrameNr = u64;

/// Standard page size (4KB)
pub const PAGE_SIZE: u64 = 4096;

/// Page shift (number of bits for page offset)
pub const PAGE_SHIFT: u32 = 12;

/// Page mask
pub const PAGE_MASK: u64 = !(PAGE_SIZE - 1);

/// Huge page sizes supported by the system
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PageSize {
    /// 4KB standard page
    Size4K = 12,
    /// 2MB huge page
    Size2M = 21,
    /// 1GB huge page
    Size1G = 30,
}

impl PageSize {
    /// Get the size in bytes
    pub const fn size(self) -> u64 {
        match self {
            PageSize::Size4K => 4 * 1024,
            PageSize::Size2M => 2 * 1024 * 1024,
            PageSize::Size1G => 1024 * 1024 * 1024,
        }
    }

    /// Get the shift amount
    pub const fn shift(self) -> u32 {
        match self {
            PageSize::Size4K => 12,
            PageSize::Size2M => 21,
            PageSize::Size1G => 30,
        }
    }

    /// Get the mask for alignment
    pub const fn mask(self) -> u64 {
        !(self.size() - 1)
    }

    /// Check if a size is a valid page size
    pub fn is_valid(size: u64) -> bool {
        match size {
            4096 | 2_097_152 | 1_073_741_824 => true,
            _ => false,
        }
    }

    /// Get the smallest page size that can accommodate the given size
    pub fn from_size(size: u64) -> Option<Self> {
        if size <= PageSize::Size4K.size() {
            Some(PageSize::Size4K)
        } else if size <= PageSize::Size2M.size() {
            Some(PageSize::Size2M)
        } else if size <= PageSize::Size1G.size() {
            Some(PageSize::Size1G)
        } else {
            None
        }
    }

    /// Check if an address is aligned to this page size
    pub fn is_aligned(self, addr: u64) -> bool {
        (addr & (self.size() - 1)) == 0
    }

    /// Align an address down to this page size boundary
    pub const fn align_down(self, addr: u64) -> u64 {
        addr & self.mask()
    }

    /// Align an address up to this page size boundary
    pub const fn align_up(self, addr: u64) -> u64 {
        (addr + self.size() - 1) & self.mask()
    }

    /// Get the number of standard pages in this huge page
    pub const fn page_count(self) -> u64 {
        self.size() / PAGE_SIZE
    }
}

/// Default huge page size (2MB)
pub const DEFAULT_HUGE_PAGE: PageSize = PageSize::Size2M;

/// Get system default huge page size
pub const fn default_huge_page_size() -> u64 {
    DEFAULT_HUGE_PAGE.size()
}

/// Get system default huge page shift
pub const fn default_huge_page_shift() -> u32 {
    DEFAULT_HUGE_PAGE.shift()
}

/// Check if a size should use huge pages
pub fn should_use_huge_pages(size: u64) -> bool {
    size >= DEFAULT_HUGE_PAGE.size()
}

/// Get the optimal page size for a given allocation size
pub fn optimal_page_size(size: u64) -> PageSize {
    if size >= PageSize::Size1G.size() && PageSize::Size1G.is_aligned(size) {
        PageSize::Size1G
    } else if size >= PageSize::Size2M.size() && PageSize::Size2M.is_aligned(size) {
        PageSize::Size2M
    } else {
        PageSize::Size4K
    }
}

/// Memory region descriptor
#[derive(Debug, Clone, Copy)]
pub struct MemoryRegion {
    /// Start physical address
    pub start: PhysAddr,
    /// Size in bytes
    pub size: u64,
    /// Region type
    pub kind: MemoryRegionKind,
    /// Region flags
    pub flags: MemoryRegionFlags,
}

/// Types of memory regions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryRegionKind {
    /// Available memory
    Available,
    /// Reserved memory
    Reserved,
    /// Device memory
    Device,
    /// ACPI reclaimable memory
    AcpiReclaimable,
    /// ACPI non-volatile storage
    AcpiNvs,
    /// Memory used by the kernel/hypervisor
    Kernel,
    /// Memory mapped I/O
    Mmio,
}

/// Memory region flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryRegionFlags {
    pub readable: bool,
    pub writable: bool,
    pub executable: bool,
    pub cached: bool,
    pub device: bool,
}

impl Default for MemoryRegionFlags {
    fn default() -> Self {
        Self {
            readable: true,
            writable: true,
            executable: false,
            cached: true,
            device: false,
        }
    }
}

/// Address space type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressSpaceType {
    /// Kernel address space
    Kernel,
    /// User (guest) address space
    User,
    /// Direct-mapped physical address space
    Physical,
}

/// Page table entry flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageFlags {
    /// Page is present
    pub present: bool,
    /// Page is writable
    pub writable: bool,
    /// Page is executable (on architectures that support NX, this means not executable)
    pub executable: bool,
    /// Page is accessible from user mode
    pub user: bool,
    /// Page has write-through caching
    pub write_through: bool,
    /// Page disables cache
    pub cache_disable: bool,
    /// Page was accessed
    pub accessed: bool,
    /// Page was written to (dirty)
    pub dirty: bool,
    /// Global page (not flushed on TLB shootdown)
    pub global: bool,
    /// Copy-on-write page
    pub cow: bool,
    /// Write-protected (for COW)
    pub write_protected: bool,
}

impl Default for PageFlags {
    fn default() -> Self {
        Self {
            present: true,
            writable: false,
            executable: true,
            user: false,
            write_through: false,
            cache_disable: false,
            accessed: false,
            dirty: false,
            global: false,
            cow: false,
            write_protected: false,
        }
    }
}

impl PageFlags {
    /// Create COW page flags (read-only, will trigger write fault)
    pub fn cow() -> Self {
        Self {
            present: true,
            writable: false,        // Write-protected to trigger fault
            executable: true,
            user: false,
            write_through: false,
            cache_disable: false,
            accessed: false,
            dirty: false,
            global: false,
            cow: true,
            write_protected: true,
        }
    }

    /// Check if page is COW-enabled
    pub fn is_cow(&self) -> bool {
        self.cow && self.write_protected
    }

    /// Create writable COW page (after copy)
    pub fn cow_writable() -> Self {
        Self {
            present: true,
            writable: true,         // Now writable after copy
            executable: true,
            user: false,
            write_through: false,
            cache_disable: false,
            accessed: false,
            dirty: false,
            global: false,
            cow: false,            // No longer COW
            write_protected: false,
        }
    }
}

/// Initialize the memory management subsystem
pub fn init() -> Result<()> {
    // Initialize physical memory manager
    frame::init()?;

    // Initialize virtual memory manager
    page::init()?;

    // Initialize heap allocator
    heap::init()?;

    // Initialize buddy allocator
    buddy::init(0x80000000, 64 * 1024 * 1024) // 64MB starting at 2GB
        .map_err(|_| crate::Error::MemoryError)?;

    // Initialize slab allocator
    slab::init().map_err(|_| crate::Error::MemoryError)?;

    // Initialize unified allocator
    allocator::init().map_err(|_| crate::Error::MemoryError)?;

    // Initialize COW memory management
    page::init_cow().map_err(|_| crate::Error::MemoryError)?;

    // Initialize huge page management
    hugepage::init_huge_page_manager().map_err(|_| crate::Error::MemoryError)?;

    // Initialize G-stage address translation (support up to 256 VMs)
    gstage::init(255)?;

    Ok(())
}

/// Align an address down to page boundary
pub const fn align_down(addr: u64) -> u64 {
    addr & PAGE_MASK
}

/// Align an address up to page boundary
pub const fn align_up(addr: u64) -> u64 {
    (addr + PAGE_SIZE - 1) & PAGE_MASK
}

/// Check if an address is page-aligned
pub const fn is_aligned(addr: u64) -> bool {
    (addr & (PAGE_SIZE - 1)) == 0
}

/// Convert a virtual address to page number
pub const fn virt_to_page(addr: VirtAddr) -> PageNr {
    addr >> PAGE_SHIFT
}

/// Convert a page number to virtual address
pub const fn page_to_virt(page: PageNr) -> VirtAddr {
    page << PAGE_SHIFT
}

/// Convert a physical address to frame number
pub const fn phys_to_frame(addr: PhysAddr) -> FrameNr {
    addr >> PAGE_SHIFT
}

/// Convert a frame number to physical address
pub const fn frame_to_phys(frame: FrameNr) -> PhysAddr {
    frame << PAGE_SHIFT
}

/// Flush the TLB for the entire address space
pub fn flush_tlb_all() {
    #[cfg(target_arch = "aarch64")]
    {
        unsafe {
            core::arch::asm!("tlbi vmalle1is");
            core::arch::asm!("dsb ish");
            core::arch::asm!("isb");
        }
    }

    #[cfg(target_arch = "riscv64")]
    {
        unsafe {
            core::arch::asm!("sfence.vma");
        }
    }

    #[cfg(target_arch = "x86_64")]
    {
        unsafe { core::arch::asm!("invlpg [rax]", in("rax") 0_usize) };
    }
}

/// Flush the TLB for a specific virtual address
pub fn flush_tlb_addr(addr: VirtAddr) {
    #[cfg(target_arch = "aarch64")]
    {
        unsafe {
            let addr: u64;
            core::arch::asm!("mrs {}, tcr_el1", out(reg) addr);
            let tcr_el1 = addr;
            let tg = (tcr_el1 >> 14) & 0x3;
            let page_size = match tg {
                0 => 16 * 1024,     // 16KB pages
                1 => 4 * 1024,      // 4KB pages
                2 => 64 * 1024,     // 64KB pages
                _ => 4 * 1024,      // Default to 4KB
            };

            let asid = (tcr_el1 >> 36) & 0xFFFF;
            core::arch::asm!(
                "tlbi vae1is, {}",
                in(reg) ((asid << 48) | (addr / page_size))
            );
            core::arch::asm!("dsb ish");
            core::arch::asm!("isb");
        }
    }

    #[cfg(target_arch = "riscv64")]
    {
        unsafe {
            core::arch::asm!("sfence.vma, {}", in(reg) addr);
        }
    }

    #[cfg(target_arch = "x86_64")]
    {
        unsafe { core::arch::asm!("invlpg [rax]", in("rax") addr) };
    }
}

/// Memory barrier operations
pub mod barrier {
    /// Ensure all memory reads/writes are complete
    pub fn memory() {
        #[cfg(target_arch = "aarch64")]
        unsafe { core::arch::asm!("dmb sy") };

        #[cfg(target_arch = "riscv64")]
        unsafe { core::arch::asm!("fence rw, rw") };

        #[cfg(target_arch = "x86_64")]
        unsafe { core::arch::asm!("mfence") };
    }

    /// Ensure all memory writes are complete
    pub fn write() {
        #[cfg(target_arch = "aarch64")]
        unsafe { core::arch::asm!("dsb st") };

        #[cfg(target_arch = "riscv64")]
        unsafe { core::arch::asm!("fence w, w") };

        #[cfg(target_arch = "x86_64")]
        unsafe { core::arch::asm!("sfence") };
    }

    /// Ensure all memory reads are complete
    pub fn read() {
        #[cfg(target_arch = "aarch64")]
        unsafe { core::arch::asm!("dsb ld") };

        #[cfg(target_arch = "riscv64")]
        unsafe { core::arch::asm!("fence r, r") };

        #[cfg(target_arch = "x86_64")]
        unsafe { core::arch::asm!("lfence") };
    }
}