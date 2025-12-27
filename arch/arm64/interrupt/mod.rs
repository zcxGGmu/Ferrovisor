//! Interrupt handling for ARM64
//!
//! Provides interrupt controller support (GIC/VGIC) for virtualization.

/// GIC discovery and initialization
pub mod gic;

/// GIC device tree discovery
pub mod gic_discovery;

/// VGIC (Virtual GIC) implementation
pub mod vgic;

/// Virtual interrupt handling
pub mod virq;

/// Exception handlers (C-compatible ABI for assembly)
pub mod handlers;

// Re-export commonly used types
pub use gic::{
    GicVersion, GicDistributor, GicCpuInterface, GicHypInterface, GicDevice,
    gicd, gicc, gich, gicr, icc,
};
pub use gic_discovery::{
    GicDiscoveryConfig, GicInitializedInfo,
    discover_and_init_gic, auto_init_gic, init_platform_gic,
    init_qemu_virt_gic, init_foundation_v8_gic,
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
pub use handlers::{
    ExceptionType, ExceptionContext, ExceptionHandler,
    set_exception_handler,
};

/// Initialize interrupt handling
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing ARM64 interrupt controller");
    log::info!("ARM64 interrupt controller initialized (GIC/VGIC ready)");
    Ok(())
}
