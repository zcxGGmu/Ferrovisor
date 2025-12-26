//! Memory Management Unit for ARM64
//!
//! Provides Stage-2 translation and memory management for virtualization.

/// Stage-2 page table structures
pub mod stage2;

/// Stage-2 page table operations
pub mod operations;

/// VTTBR_EL2 management
pub mod vttbr;

/// VTCR_EL2 configuration
pub mod vtcr;

/// Memory attributes
pub mod attrs;

// Re-export commonly used types
pub use stage2::{PageTableEntry, PageTable, PageTableLevel, pte, block_sizes, index, level_index};
pub use operations::{MapFlags, map_range, unmap_range, tlb_flush_ipa, tlb_flush_all, pte_sync};

/// Initialize MMU
pub fn init() -> Result<(), &'static str> {
    stage2::init()?;
    // log::info!("ARM64 MMU initialized (Stage-2 translation ready)");
    Ok(())
}
