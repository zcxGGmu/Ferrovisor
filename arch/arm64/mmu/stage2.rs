//! Stage-2 page table management for ARM64
//!
//! Provides Stage-2 translation table structures and operations.
//! Reference: xvisor/arch/arm/cpu/common/mmu_lpae.c

/// Stage-2 page table levels (48-bit IPA uses 4 levels, but we start at L1)
pub const STAGE2_LEVELS: usize = 3;

/// Page table entry bits and masks
pub mod pte {
    /// Output address mask (bits [47:12])
    pub const OUTADDR_MASK: u64 = 0x0000_FFFF_FFFF_F000;

    // Stage-2 specific bits

    /// Valid bit (bit 0)
    pub const VALID_MASK: u64 = 0x0000_0000_0000_0001;

    /// Table bit (bit 1)
    pub const TABLE_MASK: u64 = 0x0000_0000_0000_0002;

    // Block/Page descriptor bits (Stage-2 lower attributes)

    /// Access Flag (bit 10)
    pub const AF_MASK: u64 = 0x0000_0000_0000_0400;
    pub const AF_SHIFT: u64 = 10;

    /// Shareability (bits [9:8])
    pub const SH_MASK: u64 = 0x0000_0000_0000_0300;
    pub const SH_SHIFT: u64 = 8;

    /// Shareability values
    pub const SH_NON_SHAREABLE: u64 = 0x0 << 8;
    pub const SH_OUTER_SHAREABLE: u64 = 0x2 << 8;
    pub const SH_INNER_SHAREABLE: u64 = 0x3 << 8;

    /// Hypervisor Access Permission (bits [7:6])
    pub const HAP_MASK: u64 = 0x0000_0000_0000_00C0;
    pub const HAP_SHIFT: u64 = 6;

    /// HAP values
    pub const HAP_NO_ACCESS: u64 = 0x0 << 6;
    pub const HAP_READ_ONLY: u64 = 0x1 << 6;
    pub const HAP_WRITE_ONLY: u64 = 0x2 << 6;
    pub const HAP_READ_WRITE: u64 = 0x3 << 6;

    /// Memory Attributes (bits [5:2])
    pub const MEMATTR_MASK: u64 = 0x0000_0000_0000_003C;
    pub const MEMATTR_SHIFT: u64 = 2;

    /// Memory attribute values
    /// 0x0 - Strongly Ordered / Device
    /// 0x4 - Normal Memory, Non-Cacheable
    /// 0x5 - Normal Memory, Inner/Outer Write-Through
    /// 0x7 - Normal Memory, Inner/Outer Write-Back
    pub const MEMATTR_DEVICE: u64 = 0x0;
    pub const MEMATTR_NORMAL_NC: u64 = 0x4;
    pub const MEMATTR_NORMAL_WT: u64 = 0x5;
    pub const MEMATTR_NORMAL_WB: u64 = 0x7;

    /// Contiguous hint (bit 52)
    pub const CONTIGUOUS_MASK: u64 = 0x0010_0000_0000_0000;

    /// Execute never (bit 54)
    pub const XN_MASK: u64 = 0x0040_0000_0000_0000;
    pub const XN_SHIFT: u64 = 54;
}

/// Block sizes at each level for 48-bit IPA
pub mod block_sizes {
    /// 4KB page (level 3)
    pub const SIZE_4K: u64 = 0x1000;
    pub const SHIFT_4K: u32 = 12;

    /// 2MB block (level 2)
    pub const SIZE_2M: u64 = 0x200000;
    pub const SHIFT_2M: u32 = 21;

    /// 1GB block (level 1)
    pub const SIZE_1G: u64 = 0x40000000;
    pub const SHIFT_1G: u32 = 30;

    /// 512GB block (level 0)
    pub const SIZE_512G: u64 = 0x8000000000;
    pub const SHIFT_512G: u32 = 39;
}

/// Index masks and shifts at each level
pub mod index {
    /// Level 3 index (Bit[20:12])
    pub const L3_MASK: u64 = 0x0000_0000_01FF_000;
    pub const L3_SHIFT: u32 = super::block_sizes::SHIFT_4K;
    pub const L3_COUNT: usize = 512;

    /// Level 2 index (Bit[29:21])
    pub const L2_MASK: u64 = 0x0000_03FE_00_0000;
    pub const L2_SHIFT: u32 = super::block_sizes::SHIFT_2M;
    pub const L2_COUNT: usize = 512;

    /// Level 1 index (Bit[38:30])
    pub const L1_MASK: u64 = 0x0007_FC00_0000_0000;
    pub const L1_SHIFT: u32 = super::block_sizes::SHIFT_1G;
    pub const L1_COUNT: usize = 512;

    /// Level 0 index (Bit[47:39])
    pub const L0_MASK: u64 = 0xFF80_0000_0000_0000;
    pub const L0_SHIFT: u32 = super::block_sizes::SHIFT_512G;
    pub const L0_COUNT: usize = 256;
}

/// Page table entry at each level
#[derive(Debug, Clone, Copy)]
#[repr(C, align(8))]
pub struct PageTableEntry {
    entry: u64,
}

impl PageTableEntry {
    /// Create a new page table entry
    pub const fn new(entry: u64) -> Self {
        Self { entry }
    }

    /// Create a zero page table entry
    pub const fn zero() -> Self {
        Self { entry: 0 }
    }

    /// Create a table descriptor (points to next level)
    pub fn table_descriptor(next_level_pa: u64) -> Self {
        let entry = (next_level_pa & pte::OUTADDR_MASK) | pte::TABLE_MASK | pte::VALID_MASK;
        Self { entry }
    }

    /// Create a block descriptor
    pub fn block_descriptor(output_pa: u64, memattr: u64, hap: u64, sh: u64, af: bool) -> Self {
        let mut entry = (output_pa & pte::OUTADDR_MASK) | pte::VALID_MASK;
        entry |= (memattr << pte::MEMATTR_SHIFT) & pte::MEMATTR_MASK;
        entry |= (hap << pte::HAP_SHIFT) & pte::HAP_MASK;
        entry |= sh & pte::SH_MASK;
        if af {
            entry |= pte::AF_MASK;
        }
        Self { entry }
    }

    /// Create a page descriptor (level 3 block)
    pub fn page_descriptor(output_pa: u64, memattr: u64, hap: u64, sh: u64, xn: bool) -> Self {
        let mut entry = (output_pa & pte::OUTADDR_MASK) | pte::VALID_MASK;
        entry |= (memattr << pte::MEMATTR_SHIFT) & pte::MEMATTR_MASK;
        entry |= (hap << pte::HAP_SHIFT) & pte::HAP_MASK;
        entry |= sh & pte::SH_MASK;
        entry |= pte::AF_MASK; // Pages always have AF set
        if xn {
            entry |= pte::XN_MASK;
        }
        Self { entry }
    }

    /// Check if entry is valid (has valid bit set)
    pub fn is_valid(&self) -> bool {
        (self.entry & pte::VALID_MASK) != 0
    }

    /// Check if entry is a table (points to next level)
    pub fn is_table(&self) -> bool {
        (self.entry & (pte::TABLE_MASK | pte::VALID_MASK)) == (pte::TABLE_MASK | pte::VALID_MASK)
    }

    /// Check if entry is a block/page (mapping)
    pub fn is_block(&self) -> bool {
        self.is_valid() && !self.is_table()
    }

    /// Get physical address from entry
    pub fn output_address(&self) -> u64 {
        self.entry & pte::OUTADDR_MASK
    }

    /// Get access flag
    pub fn access_flag(&self) -> bool {
        (self.entry & pte::AF_MASK) != 0
    }

    /// Get shareability
    pub fn shareability(&self) -> u64 {
        (self.entry & pte::SH_MASK) >> pte::SH_SHIFT
    }

    /// Get hypervisor access permissions
    pub fn hap(&self) -> u64 {
        (self.entry & pte::HAP_MASK) >> pte::HAP_SHIFT
    }

    /// Get memory attributes
    pub fn memattr(&self) -> u64 {
        (self.entry & pte::MEMATTR_MASK) >> pte::MEMATTR_SHIFT
    }

    /// Check if execute-never is set
    pub fn is_xn(&self) -> bool {
        (self.entry & pte::XN_MASK) != 0
    }

    /// Check if contiguous hint is set
    pub fn is_contiguous(&self) -> bool {
        (self.entry & pte::CONTIGUOUS_MASK) != 0
    }

    /// Get raw entry value
    pub fn raw(&self) -> u64 {
        self.entry
    }
}

/// Stage-2 page table (512 entries, 4KB aligned)
#[derive(Debug)]
#[repr(C, align(4096))]
pub struct PageTable {
    entries: [PageTableEntry; 512],
}

impl PageTable {
    /// Create a new zeroed page table
    pub const fn new() -> Self {
        Self {
            entries: [PageTableEntry::zero(); 512],
        }
    }

    /// Get entry at index
    pub fn get(&self, index: usize) -> Option<&PageTableEntry> {
        self.entries.get(index)
    }

    /// Get mutable entry at index
    pub fn get_mut(&mut self, index: usize) -> Option<&mut PageTableEntry> {
        self.entries.get_mut(index)
    }

    /// Set entry at index
    pub fn set(&mut self, index: usize, entry: PageTableEntry) {
        if index < 512 {
            self.entries[index] = entry;
        }
    }

    /// Clear entry at index
    pub fn clear(&mut self, index: usize) {
        if index < 512 {
            self.entries[index] = PageTableEntry::zero();
        }
    }

    /// Get entries as a slice
    pub fn entries(&self) -> &[PageTableEntry] {
        &self.entries
    }

    /// Get entries as a mutable slice
    pub fn entries_mut(&mut self) -> &mut [PageTableEntry] {
        &mut self.entries
    }
}

impl Default for PageTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Page table level enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageTableLevel {
    L0 = 0,
    L1 = 1,
    L2 = 2,
    L3 = 3,
}

impl PageTableLevel {
    /// Get block size for this level
    pub fn block_size(&self) -> u64 {
        match self {
            Self::L0 => block_sizes::SIZE_512G,
            Self::L1 => block_sizes::SIZE_1G,
            Self::L2 => block_sizes::SIZE_2M,
            Self::L3 => block_sizes::SIZE_4K,
        }
    }

    /// Get block shift for this level
    pub fn block_shift(&self) -> u32 {
        match self {
            Self::L0 => block_sizes::SHIFT_512G,
            Self::L1 => block_sizes::SHIFT_1G,
            Self::L2 => block_sizes::SHIFT_2M,
            Self::L3 => block_sizes::SHIFT_4K,
        }
    }

    /// Get index mask for this level
    pub fn index_mask(&self) -> u64 {
        match self {
            Self::L0 => index::L0_MASK,
            Self::L1 => index::L1_MASK,
            Self::L2 => index::L2_MASK,
            Self::L3 => index::L3_MASK,
        }
    }

    /// Get index shift for this level
    pub fn index_shift(&self) -> u32 {
        match self {
            Self::L0 => index::L0_SHIFT,
            Self::L1 => index::L1_SHIFT,
            Self::L2 => index::L2_SHIFT,
            Self::L3 => index::L3_SHIFT,
        }
    }

    /// Get index count for this level
    pub fn index_count(&self) -> usize {
        match self {
            Self::L0 => index::L0_COUNT,
            Self::L1 => index::L1_COUNT,
            Self::L2 => index::L2_COUNT,
            Self::L3 => index::L3_COUNT,
        }
    }

    /// Check if this is the last level
    pub fn is_last_level(&self) -> bool {
        *self == Self::L3
    }
}

/// Extract index for given level from IPA
pub fn level_index(ipa: u64, level: PageTableLevel) -> usize {
    ((ipa & level.index_mask()) >> level.index_shift()) as usize
}

/// Initialize MMU module
pub fn init() -> Result<(), &'static str> {
    // log::debug!("ARM64 Stage-2 MMU initialized");
    Ok(())
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_page_table_entry() {
        let entry = PageTableEntry::new(0);
        assert!(!entry.is_valid());
        assert!(!entry.is_table());
        assert!(!entry.is_block());
    }

    #[test]
    fn test_table_descriptor() {
        let entry = PageTableEntry::table_descriptor(0x4000_0000);
        assert!(entry.is_valid());
        assert!(entry.is_table());
        assert!(!entry.is_block());
        assert_eq!(entry.output_address(), 0x4000_0000);
    }

    #[test]
    fn test_block_descriptor() {
        let entry = PageTableEntry::block_descriptor(
            0x5000_0000,
            pte::MEMATTR_NORMAL_WB,
            pte::HAP_READ_WRITE,
            pte::SH_INNER_SHAREABLE,
            true,
        );
        assert!(entry.is_valid());
        assert!(!entry.is_table());
        assert!(entry.is_block());
        assert_eq!(entry.output_address(), 0x5000_0000);
        assert!(entry.access_flag());
    }

    #[test]
    fn test_page_table() {
        let pt = PageTable::new();
        assert!(pt.get(0).is_some());
        assert!(pt.get(512).is_none());
        assert!(pt.get(511).is_some());
    }

    #[test]
    fn test_block_sizes() {
        assert_eq!(block_sizes::SIZE_4K, 0x1000);
        assert_eq!(block_sizes::SIZE_2M, 0x200000);
        assert_eq!(block_sizes::SIZE_1G, 0x40000000);
        assert_eq!(block_sizes::SIZE_512G, 0x8000000000);
    }

    #[test]
    fn test_page_table_level() {
        assert_eq!(PageTableLevel::L0.block_size(), block_sizes::SIZE_512G);
        assert_eq!(PageTableLevel::L1.block_size(), block_sizes::SIZE_1G);
        assert_eq!(PageTableLevel::L2.block_size(), block_sizes::SIZE_2M);
        assert_eq!(PageTableLevel::L3.block_size(), block_sizes::SIZE_4K);

        assert!(PageTableLevel::L3.is_last_level());
        assert!(!PageTableLevel::L2.is_last_level());
    }

    #[test]
    fn test_level_index() {
        let ipa: u64 = 0x0000_1234_5678_9000;

        let l0_idx = level_index(ipa, PageTableLevel::L0);
        let l1_idx = level_index(ipa, PageTableLevel::L1);
        let l2_idx = level_index(ipa, PageTableLevel::L2);
        let l3_idx = level_index(ipa, PageTableLevel::L3);

        // Verify indices are within valid ranges
        assert!(l0_idx < PageTableLevel::L0.index_count());
        assert!(l1_idx < PageTableLevel::L1.index_count());
        assert!(l2_idx < PageTableLevel::L2.index_count());
        assert!(l3_idx < PageTableLevel::L3.index_count());
    }
}

