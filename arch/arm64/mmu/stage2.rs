//! Stage-2 page table management for ARM64
//!
//! Provides Stage-2 translation table structures and operations.

use crate::mmu::stage2;

/// Stage-2 page table levels
pub const STAGE2_LEVELS: usize = 3;

/// Page table entry at each level
#[derive(Debug, Clone, Copy)]
#[repr(C, align(8))]
pub struct PageTableEntry {
    entry: u64,
}

impl PageTableEntry {
    /// Create a new page table entry
    pub fn new(entry: u64) -> Self {
        Self { entry }
    }

    /// Check if entry is valid
    pub fn is_valid(&self) -> bool {
        (self.entry & 0x1) != 0
    }

    /// Check if entry is a table (points to next level)
    pub fn is_table(&self) -> bool {
        (self.entry & 0x3) == 0x3
    }

    /// Check if entry is a block (mapping)
    pub fn is_block(&self) -> bool {
        (self.entry & 0x3) == 0x1
    }

    /// Get physical address from entry
    pub fn addr(&self) -> u64 {
        self.entry & 0x0000FFFFFFFFF000
    }
}

/// Stage-2 page table
#[derive(Debug)]
pub struct PageTable {
    entries: [PageTableEntry; 512],
}

impl PageTable {
    /// Create a new zeroed page table
    pub fn new() -> Self {
        Self {
            entries: [PageTableEntry { entry: 0 }; 512],
        }
    }

    /// Get entry at index
    pub fn get(&self, index: usize) -> Option<&PageTableEntry> {
        self.entries.get(index)
    }

    /// Set entry at index
    pub fn set(&mut self, index: usize, entry: PageTableEntry) {
        if index < 512 {
            self.entries[index] = entry;
        }
    }
}

impl Default for PageTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Page table levels for Stage-2 translation
pub enum PageTableLevel {
    L0 = 0,
    L1 = 1,
    L2 = 2,
    L3 = 3,
}

/// Block sizes at each level
pub mod block_sizes {
    use crate::*;

    /// 4KB page (level 3)
    pub const SIZE_4K: u64 = 0x1000;
    /// 2MB block (level 2)
    pub const SIZE_2M: u64 = 0x200000;
    /// 1GB block (level 1)
    pub const SIZE_1G: u64 = 0x40000000;
    /// 512GB block (level 0)
    pub const SIZE_512G: u64 = 0x8000000000;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_table_entry() {
        let entry = PageTableEntry::new(0);
        assert!(!entry.is_valid());
        assert!(!entry.is_table());
        assert!(!entry.is_block());
    }

    #[test]
    fn test_page_table() {
        let pt = PageTable::new();
        assert!(pt.get(0).is_some());
        assert!(pt.get(512).is_none());
    }

    #[test]
    fn test_block_sizes() {
        assert_eq!(block_sizes::SIZE_4K, 0x1000);
        assert_eq!(block_sizes::SIZE_2M, 0x200000);
        assert_eq!(block_sizes::SIZE_1G, 0x40000000);
        assert_eq!(block_sizes::SIZE_512G, 0x8000000000);
    }
}
