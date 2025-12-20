//! RISC-V CPU Management Module
//!
//! This module provides CPU management functionality including:
//! - Register definitions and management
//! - CSR (Control and Status Register) access
//! - CPU state save/restore
//! - Context switching
//! - CPU features detection

pub mod regs;
pub mod csr;
pub mod state;
pub mod switch;
pub mod features;
pub mod asm;

pub use regs::*;
pub use csr::*;
pub use state::*;
pub use switch::*;
pub use features::*;

use crate::arch::riscv64::*;

/// Initialize CPU management subsystem
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing RISC-V CPU management");

    // Initialize CPU state management
    state::init()?;

    // Detect and initialize CPU features
    features::detect()?;

    // Initialize assembly helpers
    asm::init()?;

    log::info!("RISC-V CPU management initialized");
    Ok(())
}

/// Get current CPU ID
#[inline]
pub fn current_cpu_id() -> usize {
    let mut hartid: usize;
    unsafe {
        core::arch::asm!(
            "csrr {}, mhartid",
            out(reg) hartid,
        );
    }
    hartid
}

/// Get current privilege level
#[inline]
pub fn current_privilege_level() -> PrivilegeLevel {
    let mut mstatus: usize;
    unsafe {
        core::arch::asm!(
            "csrr {}, mstatus",
            out(reg) mstatus,
        );
    }

    // Extract current privilege level from MPP field
    match (mstatus >> 11) & 0x3 {
        0 => PrivilegeLevel::User,
        1 => PrivilegeLevel::Supervisor,
        2 => PrivilegeLevel::Reserved,
        3 => PrivilegeLevel::Machine,
        _ => unreachable!(),
    }
}

/// Wait for interrupt (WFI instruction)
#[inline]
pub fn wait_for_interrupt() {
    unsafe {
        core::arch::asm!("wfi");
    }
}

/// Memory fence instruction
#[inline]
pub fn memory_fence() {
    unsafe {
        core::arch::asm!("fence");
    }
}

/// Memory fence for I/O operations
#[inline]
pub fn memory_fence_io() {
    unsafe {
        core::arch::asm!("fence iorw, iorw");
    }
}

/// SFENCE.VMA instruction - flush TLB entries
#[inline]
pub fn sfence_vma() {
    unsafe {
        core::arch::asm!("sfence.vma");
    }
}

/// SFENCE.VMA with specific address and ASID
#[inline]
pub fn sfence_vma_addr(addr: usize, asid: usize) {
    unsafe {
        core::arch::asm!(
            "sfence.vma {}, {}",
            in(reg) addr,
            in(reg) asid,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_id() {
        let id = current_cpu_id();
        // The CPU ID should be a valid hart ID
        assert!(id < 4096); // Reasonable upper bound
    }

    #[test]
    fn test_privilege_level() {
        let level = current_privilege_level();
        // In most test environments, we'll be in machine mode
        assert_eq!(level, PrivilegeLevel::Machine);
    }
}