//! Stage-2 address translation for ARM64
//!
//! Provides IPA (Intermediate Physical Address) to PA (Physical Address) translation
//! by walking Stage-2 page tables.
//! Reference: ARM DDI 0487I.a, D13.2 Translation Table Walk

use super::stage2::*;
use crate::Result;

/// Translation result
#[derive(Debug, Clone, Copy)]
pub struct TranslationResult {
    /// Physical address (output address from the translation)
    pub pa: u64,
    /// Block size of the mapping (4KB, 2MB, 1GB, etc.)
    pub block_size: u64,
    /// Level at which the translation was found
    pub level: PageTableLevel,
    /// Whether the mapping is executable
    pub xn: bool,
    /// Hypervisor access permissions
    pub hap: u64,
    /// Memory attributes
    pub memattr: u64,
    /// Access flag
    pub af: bool,
    /// Shareability
    pub sh: u64,
    /// Whether this is a contiguous block hint
    pub contiguous: bool,
}

/// Stage-2 translation fault types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslationFault {
    /// Translation fault - page table entry not valid
    Translation,
    /// Access fault - permission denied
    Access,
    /// Permission fault - insufficient permissions
    Permission,
}

/// Stage-2 translation error
#[derive(Debug, Clone, Copy)]
pub enum TranslationError {
    /// Translation fault at specific level
    Fault {
        fault: TranslationFault,
        level: PageTableLevel,
        ipa: u64,
    },
    /// Invalid page table structure
    InvalidTable {
        level: PageTableLevel,
        addr: u64,
    },
    /// Address exceeds supported range
    AddressOverflow {
        ipa: u64,
    },
}

/// Walk the Stage-2 page tables to translate IPA to PA
///
/// # Arguments
/// * `root_pt_pa` - Physical address of the root page table (VTTBR_EL2.BADDR)
/// * `ipa` - Intermediate Physical Address to translate
///
/// # Returns
/// * `Ok(TranslationResult)` - Translation succeeded
/// * `Err(TranslationError)` - Translation failed
///
/// # Safety
/// Must be called with valid physical addresses
pub unsafe fn translate_ipa(root_pt_pa: u64, ipa: u64) -> Result<TranslationResult, TranslationError> {
    // Start at level determined by VTCR_EL2.SL0 (typically L1 for 48-bit)
    let mut current_level = PageTableLevel::L1;
    let mut current_pt_pa = root_pt_pa;

    loop {
        // Get the page table at current level
        let pt_va = current_pt_pa as *const PageTable;

        // Calculate index at this level
        let idx = level_index(ipa, current_level);

        // Get the PTE
        let pte = (*pt_va).get(idx)
            .ok_or(TranslationError::InvalidTable {
                level: current_level,
                addr: current_pt_pa,
            })?;

        // Check if PTE is valid
        if !pte.is_valid() {
            return Err(TranslationError::Fault {
                fault: TranslationFault::Translation,
                level: current_level,
                ipa,
            });
        }

        // Check if this is a table descriptor (points to next level)
        if pte.is_table() && !current_level.is_last_level() {
            // Follow to next level
            current_pt_pa = pte.output_address();
            current_level = match current_level {
                PageTableLevel::L1 => PageTableLevel::L2,
                PageTableLevel::L2 => PageTableLevel::L3,
                _ => {
                    return Err(TranslationError::InvalidTable {
                        level: current_level,
                        addr: current_pt_pa,
                    });
                }
            };
            continue;
        }

        // This is a block or page descriptor - translation complete
        if !pte.is_block() {
            return Err(TranslationError::Fault {
                fault: TranslationFault::Translation,
                level: current_level,
                ipa,
            });
        }

        // Calculate the output address
        let block_size = current_level.block_size();
        let block_offset = ipa & (block_size - 1);
        let output_addr = pte.output_address() | block_offset;

        return Ok(TranslationResult {
            pa: output_addr,
            block_size,
            level: current_level,
            xn: pte.is_xn(),
            hap: pte.hap(),
            memattr: pte.memattr(),
            af: pte.access_flag(),
            sh: pte.shareability(),
            contiguous: pte.is_contiguous(),
        });
    }
}

/// Check if an IPA range is mapped in the page tables
///
/// # Arguments
/// * `root_pt_pa` - Physical address of the root page table
/// * `ipa_start` - Start of IPA range
/// * `size` - Size of the range
///
/// # Returns
/// * `Ok(true)` - Entire range is mapped
/// * `Ok(false)` - Range is not fully mapped
/// * `Err(TranslationError)` - Translation error
pub unsafe fn is_range_mapped(root_pt_pa: u64, ipa_start: u64, size: u64) -> Result<bool, TranslationError> {
    let ipa_end = ipa_start.checked_add(size)
        .ok_or(TranslationError::AddressOverflow { ipa: ipa_start })?;

    // Check each page block in the range
    // For efficiency, we check at 4KB granularity
    const PAGE_SIZE: u64 = 0x1000;
    let mut current_ipa = ipa_start & !(PAGE_SIZE - 1); // Align to page boundary

    while current_ipa < ipa_end {
        match translate_ipa(root_pt_pa, current_ipa) {
            Ok(_) => {
                current_ipa += PAGE_SIZE;
            }
            Err(TranslationError::Fault { .. }) => {
                return Ok(false);
            }
            Err(e) => {
                return Err(e);
            }
        }
    }

    Ok(true)
}

/// Get the memory attributes for an IPA
///
/// # Arguments
/// * `root_pt_pa` - Physical address of the root page table
/// * `ipa` - Intermediate Physical Address
///
/// # Returns
/// * Memory attributes if translation succeeds
pub unsafe fn get_ipa_attributes(
    root_pt_pa: u64,
    ipa: u64,
) -> Result<(u64, u64, bool), TranslationError> {
    let result = translate_ipa(root_pt_pa, ipa)?;
    Ok((result.memattr, result.sh, result.xn))
}

/// Check if an IPA is writable
///
/// # Arguments
/// * `root_pt_pa` - Physical address of the root page table
/// * `ipa` - Intermediate Physical Address
///
/// # Returns
/// * `Ok(true)` if writable, `Ok(false)` if not writable
pub unsafe fn is_ipa_writable(root_pt_pa: u64, ipa: u64) -> Result<bool, TranslationError> {
    let result = translate_ipa(root_pt_pa, ipa)?;
    // HAP values: 0=No access, 1=Read-only, 2=Write-only, 3=Read/Write
    Ok(result.hap == pte::HAP_READ_WRITE || result.hap == pte::HAP_WRITE_ONLY)
}

/// Check if an IPA is readable
///
/// # Arguments
/// * `root_pt_pa` - Physical address of the root page table
/// * `ipa` - Intermediate Physical Address
pub unsafe fn is_ipa_readable(root_pt_pa: u64, ipa: u64) -> Result<bool, TranslationError> {
    let result = translate_ipa(root_pt_pa, ipa)?;
    // HAP values: 0=No access, 1=Read-only, 2=Write-only, 3=Read/Write
    Ok(result.hap == pte::HAP_READ_WRITE || result.hap == pte::HAP_READ_ONLY)
}

/// Check if an IPA is executable
///
/// # Arguments
/// * `root_pt_pa` - Physical address of the root page table
/// * `ipa` - Intermediate Physical Address
pub unsafe fn is_ipa_executable(root_pt_pa: u64, ipa: u64) -> Result<bool, TranslationError> {
    let result = translate_ipa(root_pt_pa, ipa)?;
    Ok(!result.xn)
}

/// Walk page tables for debugging/analysis
///
/// Returns information about the page table structure for a given IPA
///
/// # Arguments
/// * `root_pt_pa` - Physical address of the root page table
/// * `ipa` - Intermediate Physical Address
pub unsafe fn walk_debug(root_pt_pa: u64, ipa: u64) -> Result<PageTableWalkInfo, TranslationError> {
    let mut info = PageTableWalkInfo {
        ipa,
        l1_index: level_index(ipa, PageTableLevel::L1),
        l1_valid: false,
        l1_table: false,
        l1_pa: 0,
        l2_index: level_index(ipa, PageTableLevel::L2),
        l2_valid: false,
        l2_table: false,
        l2_pa: 0,
        l3_index: level_index(ipa, PageTableLevel::L3),
        l3_valid: false,
        l3_pa: 0,
        final_pa: None,
        final_level: None,
    };

    // Level 1
    let l1_pt = root_pt_pa as *const PageTable;
    if let Some(l1_pte) = (*l1_pt).get(info.l1_index) {
        info.l1_valid = l1_pte.is_valid();
        info.l1_table = l1_pte.is_table();
        info.l1_pa = l1_pte.output_address();

        if !info.l1_valid {
            return Ok(info);
        }

        if info.l1_table {
            // Level 2
            let l2_pt = info.l1_pa as *const PageTable;
            if let Some(l2_pte) = (*l2_pt).get(info.l2_index) {
                info.l2_valid = l2_pte.is_valid();
                info.l2_table = l2_pte.is_table();
                info.l2_pa = l2_pte.output_address();

                if !info.l2_valid {
                    return Ok(info);
                }

                if info.l2_table {
                    // Level 3
                    let l3_pt = info.l2_pa as *const PageTable;
                    if let Some(l3_pte) = (*l3_pt).get(info.l3_index) {
                        info.l3_valid = l3_pte.is_valid();
                        info.l3_pa = l3_pte.output_address();

                        if info.l3_valid && l3_pte.is_block() {
                            let block_offset = ipa & (block_sizes::SIZE_4K - 1);
                            info.final_pa = Some(info.l3_pa | block_offset);
                            info.final_level = Some(PageTableLevel::L3);
                        }
                    }
                } else if l2_pte.is_block() {
                    let block_offset = ipa & (block_sizes::SIZE_2M - 1);
                    info.final_pa = Some(info.l2_pa | block_offset);
                    info.final_level = Some(PageTableLevel::L2);
                }
            }
        } else if l1_pte.is_block() {
            let block_offset = ipa & (block_sizes::SIZE_1G - 1);
            info.final_pa = Some(info.l1_pa | block_offset);
            info.final_level = Some(PageTableLevel::L1);
        }
    }

    Ok(info)
}

/// Debug information from a page table walk
#[derive(Debug, Clone, Copy)]
pub struct PageTableWalkInfo {
    /// Original IPA
    pub ipa: u64,
    /// Level 1 index
    pub l1_index: usize,
    /// Level 1 PTE valid
    pub l1_valid: bool,
    /// Level 1 is a table descriptor
    pub l1_table: bool,
    /// Level 1 output address
    pub l1_pa: u64,
    /// Level 2 index
    pub l2_index: usize,
    /// Level 2 PTE valid
    pub l2_valid: bool,
    /// Level 2 is a table descriptor
    pub l2_table: bool,
    /// Level 2 output address
    pub l2_pa: u64,
    /// Level 3 index
    pub l3_index: usize,
    /// Level 3 PTE valid
    pub l3_valid: bool,
    /// Level 3 output address
    pub l3_pa: u64,
    /// Final translated PA (if successful)
    pub final_pa: Option<u64>,
    /// Level at which translation completed
    pub final_level: Option<PageTableLevel>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translation_fault_enum() {
        let fault = TranslationFault::Translation;
        assert_eq!(fault, TranslationFault::Translation);

        let access = TranslationFault::Access;
        assert_eq!(access, TranslationFault::Access);

        let perm = TranslationFault::Permission;
        assert_eq!(perm, TranslationFault::Permission);
    }

    #[test]
    fn test_translation_error() {
        let err = TranslationError::Fault {
            fault: TranslationFault::Translation,
            level: PageTableLevel::L3,
            ipa: 0x1000,
        };
        match err {
            TranslationError::Fault { fault, level, ipa } => {
                assert_eq!(fault, TranslationFault::Translation);
                assert_eq!(level, PageTableLevel::L3);
                assert_eq!(ipa, 0x1000);
            }
            _ => panic!("Unexpected error type"),
        }
    }

    #[test]
    fn test_translation_result() {
        let result = TranslationResult {
            pa: 0x5000_0000,
            block_size: 0x1000,
            level: PageTableLevel::L3,
            xn: false,
            hap: pte::HAP_READ_WRITE,
            memattr: pte::MEMATTR_NORMAL_WB,
            af: true,
            sh: pte::SH_INNER_SHAREABLE,
            contiguous: false,
        };

        assert_eq!(result.pa, 0x5000_0000);
        assert_eq!(result.block_size, 0x1000);
        assert_eq!(result.level, PageTableLevel::L3);
        assert!(!result.xn);
        assert!(result.af);
    }

    #[test]
    fn test_level_index_calculation() {
        // Test IPA 0x0000_1234_5678_9000
        let ipa: u64 = 0x0000_1234_5678_9000;

        let l1_idx = level_index(ipa, PageTableLevel::L1);
        let l2_idx = level_index(ipa, PageTableLevel::L2);
        let l3_idx = level_index(ipa, PageTableLevel::L3);

        // Verify indices are within valid ranges
        assert!(l1_idx < PageTableLevel::L1.index_count());
        assert!(l2_idx < PageTableLevel::L2.index_count());
        assert!(l3_idx < PageTableLevel::L3.index_count());

        // L1 should be index 0x91 (bits [38:30] of IPA)
        assert_eq!(l1_idx, 0x91);

        // L2 should be index 0x1B3 (bits [29:21] of IPA)
        assert_eq!(l2_idx, 0x1B3);

        // L3 should be index 0x078 (bits [20:12] of IPA)
        assert_eq!(l3_idx, 0x078);
    }

    #[test]
    fn test_page_table_walk_info() {
        let info = PageTableWalkInfo {
            ipa: 0x1000,
            l1_index: 0,
            l1_valid: true,
            l1_table: true,
            l1_pa: 0x4000_0000,
            l2_index: 1,
            l2_valid: true,
            l2_table: true,
            l2_pa: 0x4001_0000,
            l3_index: 2,
            l3_valid: true,
            l3_pa: 0x5000_0000,
            final_pa: Some(0x5000_1000),
            final_level: Some(PageTableLevel::L3),
        };

        assert_eq!(info.ipa, 0x1000);
        assert!(info.l1_valid);
        assert!(info.l1_table);
        assert_eq!(info.final_pa, Some(0x5000_1000));
        assert_eq!(info.final_level, Some(PageTableLevel::L3));
    }
}
