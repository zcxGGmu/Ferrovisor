//! Virtual UART Device

use crate::arch::riscv64::virtualization::vm::*;

/// Initialize virtual UART subsystem
pub fn init() -> Result<(), &'static str> {
    log::debug!("Initializing virtual UART subsystem");
    Ok(())
}