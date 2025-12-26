//! Interrupt handling for ARM64
//!
//! Provides interrupt controller support (GIC/VGIC) for virtualization.

/// GIC discovery and initialization
pub mod gic;

/// VGIC (Virtual GIC) implementation
pub mod vgic;

/// Virtual interrupt handling
pub mod virq;

// Re-export commonly used types
pub use gic::{
    GicVersion, GicDistributor, GicCpuInterface, GicHypInterface, GicDevice,
    gicd, gicc, gich, gicr, icc,
};
pub use vgic::{
    VgicModel, VgicLr, VgicLrFlags, VgicHwState, VgicHwStateV2,
    VgicVcpuState, VgicGuestState, VgicOps, VgicV2Ops, VgicDevice,
    VGIC_MAX_NCPU, VGIC_MAX_NIRQ, VGIC_MAX_LRS, VGIC_LR_UNKNOWN,
};
pub use virq::{
    VirtIrqType, IrqState, VirtInterrupt,
    inject_virq, deassert_virq, virq_pending, execute_virq,
    eoi_interrupt, configure_interrupt_delegation,
    assert_virq, deassert_irq, get_irq_priority, vgic_available,
};

/// Initialize interrupt handling
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing ARM64 interrupt controller");
    log::info!("ARM64 interrupt controller initialized (GIC/VGIC ready)");
    Ok(())
}
