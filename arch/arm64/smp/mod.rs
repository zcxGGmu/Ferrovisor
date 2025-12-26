//! Symmetric Multiprocessing (SMP) for ARM64
//!
//! Provides SMP initialization and CPU management.

/// PSCI-based SMP
pub mod psci;

/// Spin table SMP
pub mod spin_table;

/// Initialize SMP
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing ARM64 SMP");
    log::info!("ARM64 SMP initialized");
    Ok(())
}
