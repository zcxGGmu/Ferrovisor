//! Stage-2 page table operations
//!
//! Provides page table mapping/unmapping operations and TLB maintenance.
//! Reference: xvisor/arch/arm/cpu/common/mmu_lpae.c

use super::stage2::*;
use crate::Result;

/// Memory mapping flags
#[derive(Debug, Clone, Copy)]
pub struct MapFlags {
    /// Whether the mapping is cacheable
    pub cacheable: bool,
    /// Whether the mapping is bufferable (write-back)
    pub bufferable: bool,
    /// Whether the mapping is writable
    pub writable: bool,
    /// Whether the mapping is executable
    pub executable: bool,
    /// Whether this is device memory
    pub device: bool,
}

impl Default for MapFlags {
    fn default() -> Self {
        Self {
            cacheable: true,
            bufferable: true,
            writable: true,
            executable: false,
            device: false,
        }
    }
}

impl MapFlags {
    /// Create flags for normal memory
    pub fn normal_memory() -> Self {
        Self {
            cacheable: true,
            bufferable: true,
            writable: true,
            executable: true,
            device: false,
        }
    }

    /// Create flags for device memory
    pub fn device_memory() -> Self {
        Self {
            cacheable: false,
            bufferable: false,
            writable: true,
            executable: false,
            device: true,
        }
    }

    /// Create flags for read-only memory
    pub fn read_only() -> Self {
        Self {
            cacheable: true,
            bufferable: true,
            writable: false,
            executable: true,
            device: false,
        }
    }

    /// Get memory attribute value for Stage-2 PTE
    pub fn memattr(&self) -> u64 {
        if self.device {
            pte::MEMATTR_DEVICE
        } else if self.cacheable && self.bufferable {
            pte::MEMATTR_NORMAL_WB
        } else if self.cacheable && !self.bufferable {
            pte::MEMATTR_NORMAL_WT
        } else {
            pte::MEMATTR_NORMAL_NC
        }
    }

    /// Get hypervisor access permission value
    pub fn hap(&self) -> u64 {
        if self.writable {
            pte::HAP_READ_WRITE
        } else {
            pte::HAP_READ_ONLY
        }
    }

    /// Get shareability value
    pub fn sh(&self) -> u64 {
        if self.device {
            pte::SH_NON_SHAREABLE
        } else {
            pte::SH_INNER_SHAREABLE
        }
    }

    /// Get XN (execute-never) value
    pub fn xn(&self) -> bool {
        !self.executable
    }
}

/// Map an IPA range to PA range in page table
///
/// # Arguments
/// * `root_pt` - Root page table physical address
/// * `ipa_start` - Start of IPA range
/// * `pa_start` - Start of PA range
/// * `size` - Size of the range (must be block-aligned)
/// * `flags` - Mapping flags
///
/// # Safety
/// Must be called with valid physical addresses
pub unsafe fn map_range(
    root_pt: u64,
    ipa_start: u64,
    pa_start: u64,
    size: u64,
    flags: MapFlags,
) -> Result<()> {
    let ipa_end = ipa_start.checked_add(size)
        .ok_or("IPA overflow")?;
    let pa_end = pa_start.checked_add(size)
        .ok_or("PA overflow")?;

    // Ensure size is non-zero
    if size == 0 {
        return Err("Size cannot be zero");
    }

    // Determine the largest block size we can use
    let mut current_ipa = ipa_start;
    let mut current_pa = pa_start;

    while current_ipa < ipa_end {
        // Try to use the largest possible block size
        let (block_size, level) = find_largest_block_size(current_ipa, ipa_end, current_pa, pa_end);

        // Get the page table for this level
        let pt_va = pt_at_level(root_pt, current_ipa, level)?;

        // Calculate index at this level
        let idx = level_index(current_ipa, level);

        // Create the block/page descriptor
        let entry = if level == PageTableLevel::L3 {
            PageTableEntry::page_descriptor(
                current_pa,
                flags.memattr(),
                flags.hap(),
                flags.sh(),
                flags.xn(),
            )
        } else {
            PageTableEntry::block_descriptor(
                current_pa,
                flags.memattr(),
                flags.hap(),
                flags.sh(),
                true, // AF always set for blocks
            )
        };

        // Set the entry
        (*pt_va).set(idx, entry);

        // Move to next block
        current_ipa += block_size;
        current_pa += block_size;
    }

    // Flush TLB for the mapped range
    tlb_flush_ipa(ipa_start, size);

    Ok(())
}

/// Unmap an IPA range from page table
///
/// # Arguments
/// * `root_pt` - Root page table physical address
/// * `ipa_start` - Start of IPA range
/// * `size` - Size of the range
pub unsafe fn unmap_range(root_pt: u64, ipa_start: u64, size: u64) -> Result<()> {
    let ipa_end = ipa_start.checked_add(size)
        .ok_or("IPA overflow")?;

    let mut current_ipa = ipa_start;

    while current_ipa < ipa_end {
        // Walk the page table to find the entry
        let (pt_va, level, idx) = walk_page_table(root_pt, current_ipa)?;

        if let Some(entry) = (*pt_va).get(idx) {
            if entry.is_table() {
                // Need to unmap entire sub-table
                // TODO: Implement recursive unmap
                return Err("Recursive unmap not implemented");
            }

            // Clear the entry
            (*pt_va).clear(idx);

            // Advance by block size at this level
            let block_size = level.block_size();
            current_ipa += block_size;
        } else {
            // Entry is invalid, skip to next block
            let block_size = level.block_size();
            current_ipa += block_size;
        }
    }

    // Flush TLB for the unmapped range
    tlb_flush_ipa(ipa_start, size);

    Ok(())
}

/// Walk the page table to find the entry for a given IPA
///
/// # Returns
/// * (page_table_va, level, index) - Pointer to page table, level, and index
unsafe fn walk_page_table(
    root_pt: u64,
    ipa: u64,
) -> Result<(*mut PageTable, PageTableLevel, usize)> {
    let mut current_pt = root_pt as *mut PageTable;
    let mut current_level = PageTableLevel::L1; // Start at L1 for 48-bit IPA

    loop {
        let idx = level_index(ipa, current_level);

        if let Some(entry) = (*current_pt).get(idx) {
            if entry.is_table() && !current_level.is_last_level() {
                // Follow to next level
                let next_pt_pa = entry.output_address();
                current_pt = next_pt_pa as *mut PageTable;
                current_level = match current_level {
                    PageTableLevel::L1 => PageTableLevel::L2,
                    PageTableLevel::L2 => PageTableLevel::L3,
                    _ => return Err("Invalid page table walk"),
                };
            } else {
                // Found block or page entry
                return Ok((current_pt, current_level, idx));
            }
        } else {
            // Invalid entry
            return Ok((current_pt, current_level, idx));
        }
    }
}

/// Get page table at specific level for IPA
unsafe fn pt_at_level(
    root_pt: u64,
    ipa: u64,
    target_level: PageTableLevel,
) -> Result<*mut PageTable> {
    let mut current_pt = root_pt as *mut PageTable;
    let mut current_level = PageTableLevel::L1;

    while current_level < target_level {
        let idx = level_index(ipa, current_level);

        if let Some(entry) = (*current_pt).get(idx) {
            if entry.is_table() {
                // Follow to next level
                let next_pt_pa = entry.output_address();
                current_pt = next_pt_pa as *mut PageTable;
                current_level = match current_level {
                    PageTableLevel::L1 => PageTableLevel::L2,
                    PageTableLevel::L2 => PageTableLevel::L3,
                    _ => return Err("Invalid page table level"),
                };
            } else {
                // Need to allocate new page table
                return Err("Page table allocation not implemented");
            }
        } else {
            // Need to allocate new page table
            return Err("Page table allocation not implemented");
        }
    }

    Ok(current_pt)
}

/// Find the largest block size that can be used for the given range
fn find_largest_block_size(
    ipa_start: u64,
    ipa_end: u64,
    pa_start: u64,
    pa_end: u64,
) -> (u64, PageTableLevel) {
    // Try from largest to smallest
    for level in &[PageTableLevel::L1, PageTableLevel::L2, PageTableLevel::L3] {
        let block_size = level.block_size();

        // Check if IPA range is aligned to block size
        if ipa_start % block_size != 0 {
            continue;
        }

        // Check if PA range is aligned to block size
        if pa_start % block_size != 0 {
            continue;
        }

        // Check if remaining range fits in one block
        let remaining_ipa = ipa_end - ipa_start;
        let remaining_pa = pa_end - pa_start;

        if remaining_ipa >= block_size && remaining_pa >= block_size {
            return (block_size, *level);
        }
    }

    // Should always at least use 4K pages
    (block_sizes::SIZE_4K, PageTableLevel::L3)
}

/// Flush TLB for IPA range
pub fn tlb_flush_ipa(ipa: u64, size: u64) {
    unsafe {
        // TLBI IPAS2E1IS, <IPA> - Invalidate Stage-2 TLB for IPA
        // For now, we'll use a simplified flush
        if size >= block_sizes::SIZE_1G {
            // Flush all TLB for large ranges
            core::arch::asm!(
                "tlbi vmalls12e1is",
                options(nostack, nomem)
            );
        } else {
            // Flush specific IPA
            let ipa_val: u64;
            if size == block_sizes::SIZE_4K {
                // Page granularity
                ipa_val = ipa >> 12;
                core::arch::asm!(
                    "tlbi ipas2e1is, {0}",
                    in(reg) ipa_val,
                    options(nostack, nomem)
                );
            } else {
                // Block granularity
                ipa_val = (ipa >> 12) | 1; // Set bit 0 to indicate block
                core::arch::asm!(
                    "tlbi ipas2e1is, {0}",
                    in(reg) ipa_val,
                    options(nostack, nomem)
                );
            }
        }

        // Data synchronization barrier
        core::arch::asm!("dsb ish", options(nostack, nomem));
    }
}

/// Flush all Stage-2 TLBs
pub fn tlb_flush_all() {
    unsafe {
        core::arch::asm!(
            "tlbi vmalls12e1is",
            options(nostack, nomem)
        );
        core::arch::asm!("dsb ish", options(nostack, nomem));
    }
}

/// Sync page table entry to memory
pub fn pte_sync(pte: &PageTableEntry) {
    unsafe {
        // Data memory barrier before PTE update
        core::arch::asm!("dmb ish", options(nostack, nomem));

        // Ensure the PTE is written (compiler barrier)
        core::ptr::read_volatile(pte as *const _ as *const u64);

        // Data synchronization barrier after PTE update
        core::arch::asm!("dsb ish", options(nostack, nomem));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_flags() {
        let flags = MapFlags::normal_memory();
        assert!(flags.cacheable);
        assert!(flags.bufferable);
        assert!(flags.writable);
        assert!(flags.executable);
        assert!(!flags.device);

        let flags = MapFlags::device_memory();
        assert!(!flags.cacheable);
        assert!(flags.device);

        let flags = MapFlags::read_only();
        assert!(!flags.writable);
        assert!(flags.executable);
    }

    #[test]
    fn test_memattr_conversion() {
        let flags = MapFlags::normal_memory();
        assert_eq!(flags.memattr(), pte::MEMATTR_NORMAL_WB);

        let flags = MapFlags::device_memory();
        assert_eq!(flags.memattr(), pte::MEMATTR_DEVICE);
    }

    #[test]
    fn test_find_largest_block_size() {
        // Aligned 1GB range
        let (size, level) = find_largest_block_size(
            0x0,
            0x4000_0000,
            0x0,
            0x4000_0000,
        );
        assert_eq!(size, block_sizes::SIZE_1G);
        assert_eq!(level, PageTableLevel::L1);

        // Aligned 2MB range
        let (size, level) = find_largest_block_size(
            0x0,
            0x20_0000,
            0x0,
            0x20_0000,
        );
        assert_eq!(size, block_sizes::SIZE_2M);
        assert_eq!(level, PageTableLevel::L2);

        // Non-aligned range
        let (size, level) = find_largest_block_size(
            0x1000,
            0x3000,
            0x0,
            0x2000,
        );
        assert_eq!(size, block_sizes::SIZE_4K);
        assert_eq!(level, PageTableLevel::L3);
    }
}
