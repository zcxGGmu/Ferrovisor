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

/// Address translation (IPA -> PA)
pub mod translate;

/// G-stage (Stage-2) context management
pub mod gstage;

/// Stage-2 fault handling
pub mod fault;

// Re-export commonly used types
pub use stage2::{PageTableEntry, PageTable, PageTableLevel, pte, block_sizes, index, level_index};
pub use operations::{MapFlags, map_range, unmap_range, tlb_flush_ipa, tlb_flush_all, pte_sync};
pub use vtcr::{VtcrConfig, read_vtcr_el2, write_vtcr_el2, init_default_48bit};
pub use attrs::{MemoryType, Shareability, MemoryAttr, MairConfig, read_mair_el2, write_mair_el2};
pub use translate::{translate_ipa, TranslationResult as TranslateResult, TranslationFault as TranslateFault, TranslationError, walk_debug};
pub use gstage::{GStageMode, GStageCapabilities, GStageContext, GStageManager, Ipa, Hpa, Vmid};
pub use fault::{Stage2Fault, FaultInfo, handle_stage2_fault};

/// Initialize MMU
pub fn init() -> Result<(), &'static str> {
    stage2::init()?;
    // log::info!("ARM64 MMU initialized (Stage-2 translation ready)");
    Ok(())
}
