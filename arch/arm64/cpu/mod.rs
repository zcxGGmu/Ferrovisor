//! CPU module for ARM64
//!
//! This module provides CPU-specific functionality including:
//! - Register access and management
//! - CPU initialization and feature detection
//! - VCPU context management
//! - System register access
//! - Assembly helpers

pub mod regs;
pub mod features;
pub mod state;
pub mod init;

pub use regs::*;
pub use features::*;
pub use state::*;

/// Current CPU ID
#[inline]
pub fn current_cpu_id() -> usize {
    // Read MPIDR_EL1 (Multiprocessor Affinity Register)
    let mpidr: u64;
    unsafe {
        core::arch::asm!("mrs {}, mpidr_el1", out(reg) mpidr);
    }
    // Extract affinity 0 (CPU ID within cluster)
    (mpidr & 0xFF) as usize
}

/// Get current exception level
#[inline]
pub fn current_exception_level() -> ExceptionLevel {
    let el: u64;
    unsafe {
        core::arch::asm!(
            "mrs {x}, CurrentEL",
            x = out(reg) el,
        );
    }
    match (el >> 2) & 0x3 {
        0 => ExceptionLevel::EL0,
        1 => ExceptionLevel::EL1,
        2 => ExceptionLevel::EL2,
        3 => ExceptionLevel::EL3,
        _ => ExceptionLevel::EL0,
    }
}

/// Wait for interrupt (WFI)
#[inline]
pub fn wait_for_interrupt() {
    unsafe {
        core::arch::asm!("wfi");
    }
}

/// Wait for event (WFE)
#[inline]
pub fn wait_for_event() {
    unsafe {
        core::arch::asm!("wfe");
    }
}

/// Send event (SEV)
#[inline]
pub fn send_event() {
    unsafe {
        core::arch::asm!("sev");
    }
}

/// Data memory barrier
#[inline]
pub fn dmb() {
    unsafe {
        core::arch::asm!("dmb sy");
    }
}

/// Data synchronization barrier
#[inline]
pub fn dsb() {
    unsafe {
        core::arch::asm!("dsb sy");
    }
}

/// Instruction synchronization barrier
#[inline]
pub fn isb() {
    unsafe {
        core::arch::asm!("isb");
    }
}

/// Data memory barrier (inner shareable)
#[inline]
pub fn dmb_is() {
    unsafe {
        core::arch::asm!("dmb ish");
    }
}

/// Data synchronization barrier (inner shareable)
#[inline]
pub fn dsb_is() {
    unsafe {
        core::arch::asm!("dsb ish");
    }
}

/// Memory barrier for before DMA
#[inline]
pub fn dma_wmb() {
    unsafe {
        core::arch::asm!("dmb ost");
    }
}

/// Memory barrier for after DMA
#[inline]
pub fn dma_rmb() {
    unsafe {
        core::arch::asm!("dmb ld");
    }
}

/// Initialize CPU management
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing ARM64 CPU");

    // Detect CPU features
    features::detect();

    // Initialize CPU state
    state::init();

    log::info!("ARM64 CPU initialized: {}", features::cpu_id_string());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_barrier_functions() {
        // These should compile without errors
        dmb();
        dsb();
        isb();
    }
}
