//! Platform Virtual Devices

use crate::arch::riscv64::virtualization::vm::*;

/// Initialize platform device subsystem
pub fn init() -> Result<(), &'static str> {
    log::debug!("Initializing platform device subsystem");
    Ok(())
}