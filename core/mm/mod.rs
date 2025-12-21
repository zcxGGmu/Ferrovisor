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

// Re-export commonly used types
pub use page::{AddressSpace, AddressSpaceType};

/// Physical address type
pub type PhysAddr = u64;

/// Virtual address type
pub type VirtAddr = u64;

/// Page number type
pub type PageNr = u64;

/// Frame number type
pub type FrameNr = u64;

/// Page size (typically 4KB)
pub const PAGE_SIZE: u64 = 4096;

/// Page shift (number of bits for page offset)
pub const PAGE_SHIFT: u32 = 12;

/// Page mask
pub const PAGE_MASK: u64 = !(PAGE_SIZE - 1);

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
        x86_64::instructions::tlb::flush_all();
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
        x86_64::instructions::tlb::flush(addr);
    }
}

/// Memory barrier operations
pub mod barrier {
    /// Ensure all memory reads/writes are complete
    pub fn memory() {
        #[cfg(target_arch = "aarch64")]
        cortex_a::asm::dmb(cortex_a::asm::SY);

        #[cfg(target_arch = "riscv64")]
        riscv::asm::fence(riscv::asm::Ordering::SeqCst, riscv::asm::Ordering::SeqCst);

        #[cfg(target_arch = "x86_64")]
        x86_64::instructions::mfence();
    }

    /// Ensure all memory writes are complete
    pub fn write() {
        #[cfg(target_arch = "aarch64")]
        cortex_a::asm::dsb(cortex_a::asm::ST);

        #[cfg(target_arch = "riscv64")]
        riscv::asm::fence(riscv::asm::Ordering::Release, riscv::asm::Ordering::Relaxed);

        #[cfg(target_arch = "x86_64")]
        x86_64::instructions::sfence();
    }

    /// Ensure all memory reads are complete
    pub fn read() {
        #[cfg(target_arch = "aarch64")]
        cortex_a::asm::dsb(cortex_a::asm::LD);

        #[cfg(target_arch = "riscv64")]
        riscv::asm::fence(riscv::asm::Ordering::Relaxed, riscv::asm::Ordering::Acquire);

        #[cfg(target_arch = "x86_64")]
        x86_64::instructions::lfence();
    }
}