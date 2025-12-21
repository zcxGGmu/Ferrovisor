//! Core hypervisor modules
//!
//! This module contains the core functionality of the hypervisor,
//! including virtual machine management, scheduling, memory management,
//! and interrupt handling.

pub mod vmm;
pub mod sched;
pub mod mm;
pub mod irq;
pub mod sync;

use crate::Result;

/// Core error type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Virtual machine error
    VmError,
    /// VCPU error
    VcpuError,
    /// Memory management error
    MemoryError,
    /// Scheduler error
    SchedulerError,
    /// Interrupt error
    IrqError,
    /// Synchronization error
    SyncError,
    /// Invalid state
    InvalidState,
    /// Resource unavailable
    ResourceUnavailable,
    /// Not implemented
    NotImplemented,
    /// Emulator error
    EmulatorError(crate::emulator::EmulatorError),
    /// Library error
    LibError(crate::libs::LibError),
}

/// Initialize all core components
pub fn init() -> Result<()> {
    // Initialize memory management first
    mm::init()?;

    // Initialize interrupt handling
    irq::init()?;

    // Initialize synchronization primitives
    sync::init()?;

    // Initialize virtual machine manager
    vmm::init()?;

    // Initialize scheduler
    sched::init()?;

    Ok(())
}

/// Get the current CPU ID
pub fn cpu_id() -> usize {
    #[cfg(target_arch = "aarch64")]
    {
        cortex_a::registers::MPIDR_EL1.get() as usize & 0xFF
    }

    #[cfg(target_arch = "riscv64")]
    {
        riscv::register::mhartid::read() as usize
    }

    #[cfg(target_arch = "x86_64")]
    {
        0 // TODO: Implement CPU ID detection for x86_64
    }
}