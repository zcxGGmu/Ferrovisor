//! Virtual memory management
//!
//! Provides page table management and virtual address space operations.

use crate::core::mm::{
    VirtAddr, PhysAddr, PageNr, FrameNr, PAGE_SIZE, PAGE_SHIFT,
    PageFlags, AddressSpaceType, align_up, align_down,
};
use crate::core::mm::frame::{alloc_frame, dealloc_frame};
use crate::utils::spinlock::SpinLock;
use core::ptr::NonNull;

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

    entry | 0x1 // Present bit
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