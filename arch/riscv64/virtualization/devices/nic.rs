//! Virtual NIC Device

use crate::arch::riscv64::virtualization::vm::*;

/// Initialize virtual NIC subsystem
pub fn init() -> Result<(), &'static str> {
    log::debug!("Initializing virtual NIC subsystem");
    Ok(())
}