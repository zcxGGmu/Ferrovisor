//! RISC-V Virtual CPU (VCPU) Management
//!
//! This module provides VCPU management functionality including:
//! - VCPU state and context management
/// - VCPU scheduling and execution
/// - Virtual register handling
/// - VCPU lifecycle management

use crate::arch::riscv64::*;
use crate::arch::riscv64::cpu::regs::CpuState;
use crate::arch::riscv64::virtualization::hextension::*;
use bitflags::bitflags;

/// VCPU state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VcpuState {
    /// VCPU is not initialized
    Uninitialized,
    /// VCPU is ready to run
    Ready,
    /// VCPU is currently running
    Running,
    /// VCPU is blocked (waiting for I/O, etc.)
    Blocked,
    /// VCPU has exited
    Exited,
}

/// VCPU execution statistics
#[derive(Debug, Clone, Copy)]
pub struct VcpuStats {
    /// Number of instructions executed
    pub instructions_executed: u64,
    /// Number of cycles spent
    pub cycles_spent: u64,
    /// Number of hypervisor traps
    pub hypervisor_traps: u64,
    /// Number of virtual interrupts injected
    pub virtual_interrupts_injected: u64,
}

impl Default for VcpuStats {
    fn default() -> Self {
        Self {
            instructions_executed: 0,
            cycles_spent: 0,
            hypervisor_traps: 0,
            virtual_interrupts_injected: 0,
        }
    }
}

/// VCPU flags and configuration
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct VcpuFlags: u32 {
        /// Enable virtual interrupts
        const VIRTUAL_INTERRUPTS = 1 << 0;
        /// Enable virtual timer
        const VIRTUAL_TIMER = 1 << 1;
        /// Enable virtual performance counters
        const VIRTUAL_PMU = 1 << 2;
        /// Enable nested virtualization
        const NESTED_VIRTUALIZATION = 1 << 3;
        /// Enable debug support
        const DEBUG_SUPPORT = 1 << 4;
    }
}

/// Virtual CPU
pub struct Vcpu {
    /// VCPU ID (unique within a VM)
    pub id: u8,
    /// VMID this VCPU belongs to
    pub vmid: u16,
    /// CPU state
    pub cpu_state: CpuState,
    /// Guest CSR state
    pub guest_csr: GuestCsrState,
    /// Current VCPU state
    pub state: VcpuState,
    /// VCPU flags
    pub flags: VcpuFlags,
    /// Exit information
    pub exit_info: Option<VcpuExitInfo>,
    /// Statistics
    pub stats: VcpuStats,
    /// Pending virtual interrupts
    pub pending_interrupts: u64,
    /// Virtual interrupt enable mask
    pub interrupt_enable: u64,
}

impl Vcpu {
    /// Create a new VCPU
    pub fn new(id: u8, vmid: u16, flags: VcpuFlags) -> Self {
        Self {
            id,
            vmid,
            cpu_state: CpuState::new(),
            guest_csr: GuestCsrState::new(),
            state: VcpuState::Uninitialized,
            flags,
            exit_info: None,
            stats: VcpuStats::default(),
            pending_interrupts: 0,
            interrupt_enable: 0,
        }
    }

    /// Initialize the VCPU
    pub fn init(&mut self, entry_pc: usize, stack_pointer: usize) -> Result<(), &'static str> {
        // Set up initial CPU state
        self.cpu_state.set_pc(entry_pc);
        self.cpu_state.set_sp(stack_pointer);
        self.cpu_state.set_privilege(crate::arch::riscv64::PrivilegeLevel::Supervisor);

        // Initialize guest CSR state
        self.guest_csr.vsstatus = crate::arch::riscv64::cpu::csr::SSTATUS::default().bits();
        self.guest_csr.vstvec = entry_pc; // Initial trap vector
        self.guest_csr.vssatp = 0; // Initially no translation

        // Set VCPU state to ready
        self.state = VcpuState::Ready;

        log::debug!("VCPU {} initialized (VMID: {}, PC: {:#x}, SP: {:#x})",
                   self.id, self.vmid, entry_pc, stack_pointer);
        Ok(())
    }

    /// Check if VCPU is ready to run
    pub fn is_ready(&self) -> bool {
        self.state == VcpuState::Ready
    }

    /// Check if VCPU is running
    pub fn is_running(&self) -> bool {
        self.state == VcpuState::Running
    }

    /// Check if VCPU has exited
    pub fn has_exited(&self) -> bool {
        self.state == VcpuState::Exited
    }

    /// Set VCPU state
    pub fn set_state(&mut self, new_state: VcpuState) {
        log::debug!("VCPU {} state change: {:?} -> {:?}", self.id, self.state, new_state);
        self.state = new_state;
    }

    /// Inject a virtual interrupt
    pub fn inject_interrupt(&mut self, interrupt_id: u32) -> Result<(), &'static str> {
        if !self.flags.contains(VcpuFlags::VIRTUAL_INTERRUPTS) {
            return Err("Virtual interrupts not enabled");
        }

        let interrupt_bit = 1u64 << interrupt_id;
        self.pending_interrupts |= interrupt_bit;

        log::debug!("Injected virtual interrupt {} into VCPU {}", interrupt_id, self.id);
        self.stats.virtual_interrupts_injected += 1;
        Ok(())
    }

    /// Clear a virtual interrupt
    pub fn clear_interrupt(&mut self, interrupt_id: u32) {
        let interrupt_bit = 1u64 << interrupt_id;
        self.pending_interrupts &= !interrupt_bit;
    }

    /// Check if a virtual interrupt is pending
    pub fn has_pending_interrupt(&self, interrupt_id: u32) -> bool {
        let interrupt_bit = 1u64 << interrupt_id;
        (self.pending_interrupts & interrupt_bit) != 0
    }

    /// Get pending virtual interrupts that are enabled
    pub fn get_enabled_pending_interrupts(&self) -> u64 {
        self.pending_interrupts & self.interrupt_enable
    }

    /// Enable virtual interrupt
    pub fn enable_interrupt(&mut self, interrupt_id: u32) {
        let interrupt_bit = 1u64 << interrupt_id;
        self.interrupt_enable |= interrupt_bit;
    }

    /// Disable virtual interrupt
    pub fn disable_interrupt(&mut self, interrupt_id: u32) {
        let interrupt_bit = 1u64 << interrupt_id;
        self.interrupt_enable &= !interrupt_bit;
    }

    /// Save VCPU state
    pub fn save_state(&mut self) -> Result<(), &'static str> {
        // Save guest CSR state
        self.guest_csr = GuestCsrState::save();

        // Update statistics
        self.stats.instructions_executed += read_csr!(crate::arch::riscv64::cpu::csr::MINSTRET);
        self.stats.cycles_spent += read_csr!(crate::arch::riscv64::cpu::csr::MCYCLE);

        log::debug!("Saved state for VCPU {}", self.id);
        Ok(())
    }

    /// Restore VCPU state
    pub fn restore_state(&self) -> Result<(), &'static str> {
        // Restore guest CSR state
        self.guest_csr.load();

        // In a real implementation, this would also restore:
        // - General purpose registers
        // - Floating point registers
        // - Vector registers (if V extension is enabled)

        log::debug!("Restored state for VCPU {}", self.id);
        Ok(())
    }

    /// Get VCPU statistics
    pub fn get_stats(&self) -> VcpuStats {
        self.stats
    }

    /// Reset VCPU statistics
    pub fn reset_stats(&mut self) {
        self.stats = VcpuStats::default();
    }

    /// Handle hypervisor trap
    pub fn handle_hypervisor_trap(&mut self, trap_info: &HypervisorTrapInfo) -> Result<(), &'static str> {
        self.stats.hypervisor_traps += 1;

        // Create exit information
        self.exit_info = Some(VcpuExitInfo {
            reason: self.determine_exit_reason(trap_info),
            trap_cause: trap_info.cause,
            trap_val: trap_info.tval,
            instruction: trap_info.htinst,
        });

        // Set state to exited
        self.state = VcpuState::Exited;

        log::debug!("VCPU {} exited due to hypervisor trap", self.id);
        Ok(())
    }

    /// Determine exit reason from trap information
    fn determine_exit_reason(&self, trap_info: &HypervisorTrapInfo) -> VcpuExitReason {
        let is_interrupt = (trap_info.cause & 0x80000000) != 0;

        if is_interrupt {
            VcpuExitReason::Interrupt
        } else {
            match trap_info.cause {
                2 => VcpuExitReason::IllegalInstruction,
                3 => VcpuExitReason::Breakpoint,
                8 | 9 => VcpuExitReason::SystemCall,
                12 | 13 | 15 => VcpuExitReason::MemoryFault,
                _ => VcpuExitReason::Unknown,
            }
        }
    }
}

/// VCPU exit reason
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VcpuExitReason {
    /// Normal interrupt
    Interrupt,
    /// Illegal instruction
    IllegalInstruction,
    /// Breakpoint
    Breakpoint,
    /// System call
    SystemCall,
    /// Memory fault
    MemoryFault,
    /// I/O operation
    Io,
    /// Hypercall
    Hypercall,
    /// Unknown reason
    Unknown,
}

/// VCPU exit information
#[derive(Debug, Clone)]
pub struct VcpuExitInfo {
    /// Exit reason
    pub reason: VcpuExitReason,
    /// Trap cause
    pub trap_cause: usize,
    /// Trap value
    pub trap_val: usize,
    /// Instruction that caused the exit
    pub instruction: usize,
}

/// VCPU manager for managing multiple VCPUs
pub struct VcpuManager {
    /// List of VCPUs
    vcpus: Vec<Vcpu>,
    /// Currently running VCPU ID
    current_vcpu: Option<u8>,
    /// Next VCPU ID to allocate
    next_vcpu_id: u8,
}

impl VcpuManager {
    /// Create a new VCPU manager
    pub fn new() -> Self {
        Self {
            vcpus: Vec::new(),
            current_vcpu: None,
            next_vcpu_id: 0,
        }
    }

    /// Allocate a new VCPU
    pub fn allocate_vcpu(
        &mut self,
        vmid: u16,
        flags: VcpuFlags,
    ) -> Result<&mut Vcpu, &'static str> {
        if self.next_vcpu_id >= 16 {
            return Err("Maximum VCPUs reached");
        }

        let vcpu_id = self.next_vcpu_id;
        self.next_vcpu_id += 1;

        let vcpu = Vcpu::new(vcpu_id, vmid, flags);
        self.vcpus.push(vcpu);

        Ok(&mut self.vcpus[self.vcpus.len() - 1])
    }

    /// Free a VCPU
    pub fn free_vcpu(&mut self, vcpu_id: u8) -> Result<(), &'static str> {
        let index = self.vcpus.iter().position(|v| v.id == vcpu_id)
            .ok_or("VCPU not found")?;

        // If this is the current VCPU, clear it
        if self.current_vcpu == Some(vcpu_id) {
            self.current_vcpu = None;
        }

        self.vcpus.remove(index);
        log::debug!("Freed VCPU {}", vcpu_id);
        Ok(())
    }

    /// Get a VCPU by ID
    pub fn get_vcpu(&mut self, vcpu_id: u8) -> Option<&mut Vcpu> {
        self.vcpus.iter_mut().find(|v| v.id == vcpu_id)
    }

    /// Get the currently running VCPU
    pub fn get_current_vcpu(&mut self) -> Option<&mut Vcpu> {
        if let Some(vcpu_id) = self.current_vcpu {
            self.get_vcpu(vcpu_id)
        } else {
            None
        }
    }

    /// Schedule a VCPU to run
    pub fn schedule_vcpu(&mut self, vcpu_id: u8) -> Result<(), &'static str> {
        let vcpu = self.get_vcpu(vcpu_id).ok_or("VCPU not found")?;

        if !vcpu.is_ready() {
            return Err("VCPU is not ready");
        }

        // If another VCPU is running, save its state
        if let Some(current_id) = self.current_vcpu {
            if let Some(current_vcpu) = self.get_vcpu(current_id) {
                current_vcpu.save_state()?;
                current_vcpu.set_state(VcpuState::Ready);
            }
        }

        // Set new VCPU as current
        vcpu.set_state(VcpuState::Running);
        self.current_vcpu = Some(vcpu_id);

        // Restore VCPU state
        vcpu.restore_state()?;

        log::debug!("Scheduled VCPU {} (VMID: {}) to run", vcpu_id, vcpu.vmid);
        Ok(())
    }

    /// Get list of all VCPUs
    pub fn get_vcpus(&self) -> &[Vcpu] {
        &self.vcpus
    }

    /// Get mutable list of all VCPUs
    pub fn get_vcpus_mut(&mut self) -> &mut [Vcpu] {
        &mut self.vcpus
    }

    /// Get total number of VCPUs
    pub fn vcpu_count(&self) -> usize {
        self.vcpus.len()
    }

    /// Check if any VCPU is ready to run
    pub fn has_ready_vcpu(&self) -> bool {
        self.vcpus.iter().any(|v| v.is_ready())
    }

    /// Get the next ready VCPU (simple round-robin)
    pub fn get_next_ready_vcpu(&mut self) -> Option<&mut Vcpu> {
        // Simple round-robin scheduler
        let start_index = self.current_vcpu.map(|id| {
            self.vcpus.iter().position(|v| v.id == id).unwrap_or(0)
        }).unwrap_or(0);

        for i in 0..self.vcpus.len() {
            let index = (start_index + i) % self.vcpus.len();
            if self.vcpus[index].is_ready() {
                return Some(&mut self.vcpus[index]);
            }
        }

        None
    }

    /// Inject virtual interrupt into all VCPUs of a VM
    pub fn inject_interrupt_to_vm(&mut self, vmid: u16, interrupt_id: u32) -> Result<(), &'static str> {
        for vcpu in &mut self.vcpus {
            if vcpu.vmid == vmid {
                vcpu.inject_interrupt(interrupt_id)?;
            }
        }
        Ok(())
    }

    /// Get statistics for all VCPUs
    pub fn get_all_stats(&self) -> Vec<(u8, VcpuStats)> {
        self.vcpus.iter().map(|v| (v.id, v.stats)).collect()
    }
}

impl Default for VcpuManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vcpu_creation() {
        let vcpu = Vcpu::new(0, 100, VcpuFlags::VIRTUAL_INTERRUPTS);

        assert_eq!(vcpu.id, 0);
        assert_eq!(vcpu.vmid, 100);
        assert_eq!(vcpu.state, VcpuState::Uninitialized);
        assert!(vcpu.flags.contains(VcpuFlags::VIRTUAL_INTERRUPTS));
    }

    #[test]
    fn test_vcpu_initialization() {
        let mut vcpu = Vcpu::new(0, 100, VcpuFlags::empty());

        vcpu.init(0x80000000, 0x90000000).unwrap();

        assert_eq!(vcpu.state, VcpuState::Ready);
        assert_eq!(vcpu.cpu_state.get_pc(), 0x80000000);
        assert_eq!(vcpu.cpu_state.get_sp(), 0x90000000);
        assert_eq!(vcpu.cpu_state.get_privilege(), crate::arch::riscv64::PrivilegeLevel::Supervisor);
    }

    #[test]
    fn test_virtual_interrupts() {
        let mut vcpu = Vcpu::new(0, 100, VcpuFlags::VIRTUAL_INTERRUPTS);

        // Inject interrupt
        vcpu.inject_interrupt(1).unwrap();
        assert!(vcpu.has_pending_interrupt(1));
        assert_eq!(vcpu.stats.virtual_interrupts_injected, 1);

        // Enable interrupt
        vcpu.enable_interrupt(1);
        assert!(vcpu.get_enabled_pending_interrupts() != 0);

        // Clear interrupt
        vcpu.clear_interrupt(1);
        assert!(!vcpu.has_pending_interrupt(1));
    }

    #[test]
    fn test_vcpu_manager() {
        let mut manager = VcpuManager::new();

        // Allocate VCPUs
        let vcpu1 = manager.allocate_vcpu(100, VcpuFlags::empty()).unwrap();
        let vcpu2 = manager.allocate_vcpu(100, VcpuFlags::empty()).unwrap();

        assert_eq!(manager.vcpu_count(), 2);
        assert_ne!(vcpu1.id, vcpu2.id);

        // Initialize VCPUs
        vcpu1.init(0x80000000, 0x90000000).unwrap();
        vcpu2.init(0x80100000, 0x90100000).unwrap();

        // Test scheduling
        manager.schedule_vcpu(vcpu1.id).unwrap();
        assert_eq!(manager.current_vcpu, Some(vcpu1.id));

        // Test ready VCPU detection
        assert!(manager.has_ready_vcpu());

        // Test VM-wide interrupt injection
        manager.inject_interrupt_to_vm(100, 5).unwrap();
        assert!(vcpu1.has_pending_interrupt(5));
        assert!(vcpu2.has_pending_interrupt(5));
    }
}