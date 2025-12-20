//! RISC-V Interrupt Module
//!
//! This module provides interrupt handling functionality including:
//! - Exception handling
//! - Interrupt controller support
//! - Interrupt routing and delegation
//! - External interrupt handling

use crate::arch::riscv64::*;

/// Initialize interrupt subsystem
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing RISC-V interrupt handling");

    // TODO: Implement interrupt initialization
    log::info!("RISC-V interrupt handling initialized");
    Ok(())
}

/// Enable external interrupts
pub fn enable_external_interrupts() {
    unsafe {
        core::arch::asm!("csrs mstatus, {0}", in(reg) 1 << 3); // MIE bit
    }
}

/// Disable external interrupts
pub fn disable_external_interrupts() {
    unsafe {
        core::arch::asm!("csrc mstatus, {0}", in(reg) 1 << 3); // MIE bit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interrupt_control() {
        // Test that we can enable/disable interrupts
        // Note: This test would need to run in a proper RISC-V environment
        enable_external_interrupts();
        disable_external_interrupts();
    }
}