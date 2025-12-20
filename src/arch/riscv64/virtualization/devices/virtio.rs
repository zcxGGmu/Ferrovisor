//! VirtIO Device Framework

use crate::arch::riscv64::virtualization::vm::*;

/// Initialize VirtIO subsystem
pub fn init() -> Result<(), &'static str> {
    log::debug!("Initializing VirtIO subsystem");
    Ok(())
}