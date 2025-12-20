//! RISC-V CPU State Management
//!
//! This module provides CPU state management functionality including:
//! - Per-CPU data structures
//! - CPU initialization
//! - State save/restore operations
//! - CPU context tracking

use crate::arch::riscv64::cpu::regs::CpuState;
use core::cell::UnsafeCell;

/// Per-CPU data structure
#[repr(C)]
pub struct PerCpuData {
    /// Current CPU state
    pub current_state: CpuState,
    /// Saved state for context switching
    pub saved_state: CpuState,
    /// CPU ID
    pub cpu_id: usize,
    /// Is CPU online?
    pub online: bool,
    /// Is CPU in hypervisor mode?
    pub in_hypervisor: bool,
    /// Current virtual CPU ID (if any)
    pub vcpu_id: Option<usize>,
    /// Private data for the CPU
    pub private_data: usize,
    /// Padding to align to cache line
    _padding: [u8; 64 - (8 * 8) % 64],
}

impl PerCpuData {
    /// Create a new per-CPU data structure
    pub fn new(cpu_id: usize) -> Self {
        Self {
            current_state: CpuState::new(),
            saved_state: CpuState::new(),
            cpu_id,
            online: false,
            in_hypervisor: false,
            vcpu_id: None,
            private_data: 0,
            _padding: [0; 64 - (8 * 8) % 64],
        }
    }

    /// Mark CPU as online
    pub fn mark_online(&mut self) {
        self.online = true;
    }

    /// Mark CPU as offline
    pub fn mark_offline(&mut self) {
        self.online = false;
    }

    /// Check if CPU is online
    pub fn is_online(&self) -> bool {
        self.online
    }

    /// Enter hypervisor mode
    pub fn enter_hypervisor(&mut self) {
        self.in_hypervisor = true;
    }

    /// Exit hypervisor mode
    pub fn exit_hypervisor(&mut self) {
        self.in_hypervisor = false;
    }

    /// Check if in hypervisor mode
    pub fn in_hypervisor_mode(&self) -> bool {
        self.in_hypervisor
    }

    /// Set current VCPU
    pub fn set_vcpu(&mut self, vcpu_id: usize) {
        self.vcpu_id = Some(vcpu_id);
    }

    /// Clear current VCPU
    pub fn clear_vcpu(&mut self) {
        self.vcpu_id = None;
    }

    /// Get current VCPU ID
    pub fn get_vcpu_id(&self) -> Option<usize> {
        self.vcpu_id
    }
}

/// Global per-CPU data array
static mut PER_CPU_DATA: [UnsafeCell<PerCpuData>; crate::arch::riscv64::MAX_CPUS] = [
    UnsafeCell::new(PerCpuData::new(0)),
    UnsafeCell::new(PerCpuData::new(1)),
    UnsafeCell::new(PerCpuData::new(2)),
    UnsafeCell::new(PerCpuData::new(3)),
    UnsafeCell::new(PerCpuData::new(4)),
    UnsafeCell::new(PerCpuData::new(5)),
    UnsafeCell::new(PerCpuData::new(6)),
    UnsafeCell::new(PerCpuData::new(7)),
    UnsafeCell::new(PerCpuData::new(8)),
    UnsafeCell::new(PerCpuData::new(9)),
    UnsafeCell::new(PerCpuData::new(10)),
    UnsafeCell::new(PerCpuData::new(11)),
    UnsafeCell::new(PerCpuData::new(12)),
    UnsafeCell::new(PerCpuData::new(13)),
    UnsafeCell::new(PerCpuData::new(14)),
    UnsafeCell::new(PerCpuData::new(15)),
};

/// Get per-CPU data for the current CPU
pub fn this_cpu() -> &'static mut PerCpuData {
    let cpu_id = super::current_cpu_id();
    unsafe {
        &mut *PER_CPU_DATA[cpu_id].get()
    }
}

/// Get per-CPU data for a specific CPU
pub fn cpu_data(cpu_id: usize) -> Option<&'static mut PerCpuData> {
    if cpu_id < crate::arch::riscv64::MAX_CPUS {
        unsafe {
            Some(&mut *PER_CPU_DATA[cpu_id].get())
        }
    } else {
        None
    }
}

/// Initialize CPU state management
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing CPU state management");

    let cpu_id = super::current_cpu_id();
    let per_cpu = this_cpu();

    // Initialize basic CPU state
    per_cpu.mark_online();
    per_cpu.enter_hypervisor();

    log::info!("CPU {} state management initialized", cpu_id);
    Ok(())
}

/// Save current CPU state
pub fn save_state() -> CpuState {
    // This would typically be implemented in assembly
    // For now, return the current state from per-CPU data
    let per_cpu = this_cpu();
    per_cpu.current_state
}

/// Restore CPU state
pub fn restore_state(state: CpuState) {
    // This would typically be implemented in assembly
    // For now, update the per-CPU data
    let per_cpu = this_cpu();
    per_cpu.current_state = state;
}

/// Save CPU state to per-CPU data
pub fn save_to_per_cpu(state: CpuState) {
    let per_cpu = this_cpu();
    per_cpu.saved_state = state;
}

/// Restore CPU state from per-CPU data
pub fn restore_from_per_cpu() -> CpuState {
    let per_cpu = this_cpu();
    per_cpu.saved_state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_per_cpu_data() {
        let mut data = PerCpuData::new(0);

        assert_eq!(data.cpu_id, 0);
        assert!(!data.online);
        assert!(!data.in_hypervisor);
        assert!(data.vcpu_id.is_none());

        data.mark_online();
        assert!(data.online);

        data.enter_hypervisor();
        assert!(data.in_hypervisor);

        data.set_vcpu(5);
        assert_eq!(data.get_vcpu_id(), Some(5));

        data.clear_vcpu();
        assert!(data.vcpu_id.is_none());
    }

    #[test]
    fn test_cpu_data_access() {
        // These tests would need to run in a proper RISC-V environment
        // For now, we just test the logic
        assert!(cpu_data(0).is_some());
        assert!(cpu_data(100).is_none());
    }
}