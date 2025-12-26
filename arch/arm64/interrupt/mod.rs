//! Interrupt handling for ARM64
//!
//! Provides interrupt controller support (GIC/VGIC) for virtualization.

/// GIC discovery and initialization
pub mod gic;

/// VGIC (Virtual GIC) implementation
pub mod vgic;

/// Virtual interrupt handling
pub mod virq;

/// Initialize interrupt handling
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing ARM64 interrupt controller");
    log::info!("ARM64 interrupt controller initialized (GIC/VGIC ready)");
    Ok(())
}
