//! RISC-V Page Table Management
//!
//! This module provides page table management for RISC-V including:
//! - Sv39/Sv48 page table formats
//! - Page table entry operations
//! - Page table allocation and initialization
//! - Address translation utilities

use crate::arch::riscv64::*;
use bitflags::bitflags;

/// Page table entry flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PteFlags: usize {
        const V = 1 << 0;       // Valid bit
        const R = 1 << 1;       // Read bit
        const W = 1 << 2;       // Write bit
        const X = 1 << 3;       // Execute bit
        const U = 1 << 4;       // User mode bit
        const G = 1 << 5;       // Global bit
        const A = 1 << 6;       // Accessed bit
        const D = 1 << 7;       // Dirty bit
        const RSW = 0x3 << 8;   // Reserved for software
    }
}

/// Page table entry structure
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct PageTableEntry(usize);

impl PageTableEntry {
    /// Create a new invalid page table entry
    #[inline]
    pub const fn new() -> Self {
        Self(0)
    }

    /// Create a page table entry from raw value
    #[inline]
    pub const fn from_raw(value: usize) -> Self {
        Self(value)
    }

    /// Get raw value
    #[inline]
    pub const fn raw(&self) -> usize {
        self.0
    }

    /// Check if the entry is valid
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.0 & PteFlags::V.bits() != 0
    }

    /// Set valid bit
    #[inline]
    pub fn set_valid(&mut self, valid: bool) {
        if valid {
            self.0 |= PteFlags::V.bits();
        } else {
            self.0 &= !PteFlags::V.bits();
        }
    }

    /// Check if the entry is readable
    #[inline]
    pub fn is_readable(&self) -> bool {
        self.0 & PteFlags::R.bits() != 0
    }

    /// Set readable bit
    #[inline]
    pub fn set_readable(&mut self, readable: bool) {
        if readable {
            self.0 |= PteFlags::R.bits();
        } else {
            self.0 &= !PteFlags::R.bits();
        }
    }

    /// Check if the entry is writable
    #[inline]
    pub fn is_writable(&self) -> bool {
        self.0 & PteFlags::W.bits() != 0
    }

    /// Set writable bit
    #[inline]
    pub fn set_writable(&mut self, writable: bool) {
        if writable {
            self.0 |= PteFlags::W.bits();
        } else {
            self.0 &= !PteFlags::W.bits();
        }
    }

    /// Check if the entry is executable
    #[inline]
    pub fn is_executable(&self) -> bool {
        self.0 & PteFlags::X.bits() != 0
    }

    /// Set executable bit
    #[inline]
    pub fn set_executable(&mut self, executable: bool) {
        if executable {
            self.0 |= PteFlags::X.bits();
        } else {
            self.0 &= !PteFlags::X.bits();
        }
    }

    /// Check if the entry is accessible in user mode
    #[inline]
    pub fn is_user(&self) -> bool {
        self.0 & PteFlags::U.bits() != 0
    }

    /// Set user mode bit
    #[inline]
    pub fn set_user(&mut self, user: bool) {
        if user {
            self.0 |= PteFlags::U.bits();
        } else {
            self.0 &= !PteFlags::U.bits();
        }
    }

    /// Check if the entry is global
    #[inline]
    pub fn is_global(&self) -> bool {
        self.0 & PteFlags::G.bits() != 0
    }

    /// Set global bit
    #[inline]
    pub fn set_global(&mut self, global: bool) {
        if global {
            self.0 |= PteFlags::G.bits();
        } else {
            self.0 &= !PteFlags::G.bits();
        }
    }

    /// Get the physical frame number (PPN)
    #[inline]
    pub fn ppn(&self) -> usize {
        self.0 >> 10
    }

    /// Set the physical frame number (PPN)
    #[inline]
    pub fn set_ppn(&mut self, ppn: usize) {
        self.0 = (self.0 & 0x3FF) | (ppn << 10);
    }

    /// Get the physical address
    #[inline]
    pub fn pa(&self) -> usize {
        self.ppn() << PAGE_SHIFT
    }

    /// Set the physical address
    #[inline]
    pub fn set_pa(&mut self, pa: usize) {
        self.set_ppn(pa >> PAGE_SHIFT);
    }

    /// Check if this is a leaf entry (has R/W/X bits set)
    #[inline]
    pub fn is_leaf(&self) -> bool {
        let rwx = PteFlags::R.bits() | PteFlags::W.bits() | PteFlags::X.bits();
        self.0 & rwx != 0
    }

    /// Check if this is a branch entry (points to next level page table)
    #[inline]
    pub fn is_branch(&self) -> bool {
        self.is_valid() && !self.is_leaf()
    }

    /// Get flags
    #[inline]
    pub fn flags(&self) -> PteFlags {
        PteFlags::from_bits_truncate(self.0)
    }

    /// Set flags
    #[inline]
    pub fn set_flags(&mut self, flags: PteFlags) {
        self.0 = (self.0 & !0xFF) | flags.bits();
    }

    /// Create a leaf entry for a page mapping
    pub fn leaf_entry(pa: usize, flags: PteFlags) -> Self {
        let mut entry = Self(0);
        entry.set_pa(pa);
        entry.set_flags(flags | PteFlags::V);
        entry
    }

    /// Create a branch entry for next level page table
    pub fn branch_entry(ppn: usize) -> Self {
        let mut entry = Self(0);
        entry.set_ppn(ppn);
        entry.set_valid(true);
        entry
    }

    /// Update accessed bit
    pub fn update_accessed(&mut self) {
        self.0 |= PteFlags::A.bits();
    }

    /// Update dirty bit
    pub fn update_dirty(&mut self) {
        self.0 |= PteFlags::D.bits();
    }
}

/// Page table structure
#[repr(C, align(4096))]
pub struct PageTable {
    entries: [PageTableEntry; 512],
}

impl PageTable {
    /// Create a new zero-initialized page table
    pub const fn new() -> Self {
        Self {
            entries: [PageTableEntry::new(); 512],
        }
    }

    /// Get entry at index
    #[inline]
    pub fn entry(&self, index: usize) -> PageTableEntry {
        self.entries[index]
    }

    /// Get mutable entry at index
    #[inline]
    pub fn entry_mut(&mut self, index: usize) -> &mut PageTableEntry {
        &mut self.entries[index]
    }

    /// Get raw entries slice
    #[inline]
    pub fn entries(&self) -> &[PageTableEntry] {
        &self.entries
    }

    /// Get raw entries mutable slice
    #[inline]
    pub fn entries_mut(&mut self) -> &mut [PageTableEntry] {
        &mut self.entries
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        for entry in &mut self.entries {
            *entry = PageTableEntry::new();
        }
    }

    /// Get physical address of this page table
    #[inline]
    pub fn pa(&self) -> usize {
        self as *const _ as usize
    }

    /// Get physical frame number of this page table
    #[inline]
    pub fn ppn(&self) -> usize {
        self.pa() >> PAGE_SHIFT
    }

    /// Map a virtual address to physical address
    pub fn map(
        &mut self,
        va: usize,
        pa: usize,
        flags: PteFlags,
        levels: usize,
    ) -> Result<(), &'static str> {
        let vpn_levels = [
            (va >> 12) & 0x1FF,  // Level 0 (4KB pages)
            (va >> 21) & 0x1FF,  // Level 1 (2MB pages)
            (va >> 30) & 0x1FF,  // Level 2 (1GB pages)
            (va >> 39) & 0x1FF,  // Level 3 (512GB pages) - for Sv48
        ];

        let mut current_table = self;

        for level in (0..levels).rev() {
            let vpn = vpn_levels[level];

            if level == 0 {
                // Leaf level - create page mapping
                let entry = current_table.entry_mut(vpn);
                if entry.is_valid() {
                    return Err("Virtual address already mapped");
                }
                *entry = PageTableEntry::leaf_entry(pa, flags);
                return Ok(());
            } else {
                // Branch level - ensure next level table exists
                let entry = current_table.entry_mut(vpn);

                if !entry.is_valid() {
                    // Allocate new page table
                    let next_table = PageTable::allocate()?;
                    *entry = PageTableEntry::branch_entry(next_table.ppn());
                    current_table = next_table;
                } else if entry.is_leaf() {
                    return Err("Invalid page table structure: expected branch entry");
                } else {
                    // Navigate to next level
                    let next_pa = entry.pa();
                    current_table = unsafe { &mut *(next_pa as *mut PageTable) };
                }
            }
        }

        Ok(())
    }

    /// Unmap a virtual address
    pub fn unmap(&mut self, va: usize, levels: usize) -> Result<(), &'static str> {
        let vpn_levels = [
            (va >> 12) & 0x1FF,  // Level 0 (4KB pages)
            (va >> 21) & 0x1FF,  // Level 1 (2MB pages)
            (va >> 30) & 0x1FF,  // Level 2 (1GB pages)
            (va >> 39) & 0x1FF,  // Level 3 (512GB pages) - for Sv48
        ];

        let mut current_table = self;

        for level in (0..levels).rev() {
            let vpn = vpn_levels[level];

            if level == 0 {
                // Leaf level - remove mapping
                let entry = current_table.entry_mut(vpn);
                if !entry.is_valid() {
                    return Err("Virtual address not mapped");
                }
                *entry = PageTableEntry::new();
                return Ok(());
            } else {
                // Branch level - navigate to next level
                let entry = current_table.entry(vpn);

                if !entry.is_valid() {
                    return Err("Virtual address not mapped");
                }

                if entry.is_leaf() {
                    return Err("Invalid page table structure: expected branch entry");
                }

                let next_pa = entry.pa();
                current_table = unsafe { &mut *(next_pa as *mut PageTable) };
            }
        }

        Ok(())
    }

    /// Look up a virtual address translation
    pub fn lookup(&self, va: usize, levels: usize) -> Result<(usize, PteFlags), &'static str> {
        let vpn_levels = [
            (va >> 12) & 0x1FF,  // Level 0 (4KB pages)
            (va >> 21) & 0x1FF,  // Level 1 (2MB pages)
            (va >> 30) & 0x1FF,  // Level 2 (1GB pages)
            (va >> 39) & 0x1FF,  // Level 3 (512GB pages) - for Sv48
        ];

        let mut current_table = self;

        for level in (0..levels).rev() {
            let vpn = vpn_levels[level];
            let entry = current_table.entry(vpn);

            if !entry.is_valid() {
                return Err("Page not present");
            }

            if level == 0 {
                // Leaf level found
                let flags = entry.flags();
                let pa = entry.pa() | (va & (PAGE_SIZE - 1));
                return Ok((pa, flags));
            } else if entry.is_leaf() {
                // Superpage mapping
                let page_size = PAGE_SIZE << (10 * level);
                let page_mask = page_size - 1;
                let flags = entry.flags();
                let pa = entry.pa() | (va & page_mask);
                return Ok((pa, flags));
            } else {
                // Navigate to next level
                let next_pa = entry.pa();
                current_table = unsafe { &*(next_pa as *const PageTable) };
            }
        }

        Err("Page not found")
    }

    /// Allocate a new page table
    fn allocate() -> Result<&'static mut Self, &'static str> {
        // TODO: Implement proper page allocation
        // For now, use static allocation
        static mut PAGE_TABLE_POOL: [PageTable; 16] = [PageTable::new(); 16];
        static mut NEXT_ALLOC: usize = 0;

        unsafe {
            if NEXT_ALLOC >= PAGE_TABLE_POOL.len() {
                return Err("Out of page table memory");
            }

            let table = &mut PAGE_TABLE_POOL[NEXT_ALLOC];
            NEXT_ALLOC += 1;
            table.clear();
            Ok(table)
        }
    }
}

impl Default for PageTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Address space identifier for ASID-based TLB management
pub type Asid = u16;

/// Root page table structure with ASID support
#[derive(Debug)]
pub struct RootPageTable {
    /// Root page table
    table: &'static mut PageTable,
    /// Address space identifier
    asid: Asid,
    /// Translation mode (Sv39/Sv48)
    mode: u8,
}

impl RootPageTable {
    /// Create a new root page table
    pub fn new(mode: u8, asid: Asid) -> Result<Self, &'static str> {
        let table = PageTable::allocate()?;

        Ok(Self {
            table,
            asid,
            mode,
        })
    }

    /// Get the SATP value for this page table
    pub fn satp(&self) -> usize {
        let ppn = self.table.ppn();
        crate::arch::riscv64::cpu::csr::SATP::make(ppn, self.asid as usize, self.mode as usize)
    }

    /// Activate this page table
    pub fn activate(&self) {
        crate::arch::riscv64::cpu::csr::SATP::write(self.satp());
        crate::arch::riscv64::cpu::asm::sfence_vma();
    }

    /// Get reference to root page table
    pub fn root(&self) -> &PageTable {
        self.table
    }

    /// Get mutable reference to root page table
    pub fn root_mut(&mut self) -> &mut PageTable {
        self.table
    }

    /// Get ASID
    pub fn asid(&self) -> Asid {
        self.asid
    }

    /// Get translation mode
    pub fn mode(&self) -> u8 {
        self.mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_table_entry() {
        let mut entry = PageTableEntry::new();
        assert!(!entry.is_valid());

        entry.set_valid(true);
        assert!(entry.is_valid());

        entry.set_readable(true);
        entry.set_writable(true);
        entry.set_user(true);

        assert!(entry.is_readable());
        assert!(entry.is_writable());
        assert!(entry.is_user());

        let pa = 0x123456000;
        entry.set_pa(pa);
        assert_eq!(entry.pa(), pa);

        let flags = PteFlags::R | PteFlags::W | PteFlags::U | PteFlags::V;
        let entry2 = PageTableEntry::leaf_entry(pa, flags);
        assert!(entry2.is_leaf());
        assert!(entry2.is_valid());
        assert_eq!(entry2.pa(), pa);
    }

    #[test]
    fn test_page_table() {
        let mut pt = PageTable::new();

        // Test setting and getting entries
        let entry = PageTableEntry::leaf_entry(0x1000, PteFlags::R | PteFlags::V);
        *pt.entry_mut(0) = entry;

        assert_eq!(pt.entry(0).raw(), entry.raw());
    }

    #[test]
    fn test_va_to_vpn_conversion() {
        let va = 0x123456789ABC;

        let vpn0 = (va >> 12) & 0x1FF;
        let vpn1 = (va >> 21) & 0x1FF;
        let vpn2 = (va >> 30) & 0x1FF;
        let vpn3 = (va >> 39) & 0x1FF;

        assert_eq!(vpn0, (va >> 12) & 0x1FF);
        assert_eq!(vpn1, (va >> 21) & 0x1FF);
        assert_eq!(vpn2, (va >> 30) & 0x1FF);
        assert_eq!(vpn3, (va >> 39) & 0x1FF);
    }

    #[test]
    fn test_root_page_table_satp() {
        // Test Sv39 mode (8)
        let rpt = RootPageTable::new(8, 123).unwrap();
        let satp = rpt.satp();

        let mode = crate::arch::riscv64::cpu::csr::SATP::extract_mode(satp);
        let asid = crate::arch::riscv64::cpu::csr::SATP::extract_asid(satp);
        let ppn = crate::arch::riscv64::cpu::csr::SATP::extract_ppn(satp);

        assert_eq!(mode, 8);
        assert_eq!(asid, 123);
        assert!(ppn > 0);
    }
}