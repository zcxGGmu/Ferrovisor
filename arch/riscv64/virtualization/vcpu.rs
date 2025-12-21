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

/// VCPU scheduling priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VcpuPriority {
    /// Idle priority (lowest)
    Idle = 0,
    /// Low priority
    Low = 1,
    /// Normal priority
    Normal = 2,
    /// High priority
    High = 3,
    /// Real-time priority (highest)
    RealTime = 4,
}

impl Default for VcpuPriority {
    fn default() -> Self {
        VcpuPriority::Normal
    }
}

/// VCPU affinity configuration
#[derive(Debug, Clone)]
pub struct VcpuAffinity {
    /// CPU mask for this VCPU
    pub cpu_mask: u64,
    /// Preferred host CPU
    pub preferred_cpu: Option<usize>,
    /// Allow migration
    pub allow_migration: bool,
}

impl Default for VcpuAffinity {
    fn default() -> Self {
        Self {
            cpu_mask: u64::MAX, // All CPUs by default
            preferred_cpu: None,
            allow_migration: true,
        }
    }
}

/// VCPU time management
#[derive(Debug, Clone, Copy)]
pub struct VcpuTimeManagement {
    /// Time slice in nanoseconds
    pub time_slice_ns: u64,
    /// Current time quota remaining
    pub time_quota_ns: u64,
    /// Deadline for current time slice
    pub deadline_ns: u64,
    /// Periodicity for periodic scheduling (0 = aperiodic)
    pub periodicity_ns: u64,
    /// Last scheduling timestamp
    pub last_schedule_ns: u64,
    /// Total execution time
    pub total_exec_ns: u64,
    /// Time spent in each state
    pub state_times_ns: [u64; 6], // One for each VcpuState
}

impl Default for VcpuTimeManagement {
    fn default() -> Self {
        Self {
            time_slice_ns: 10_000_000, // 10ms default
            time_quota_ns: 10_000_000,
            deadline_ns: 0,
            periodicity_ns: 0,
            last_schedule_ns: 0,
            total_exec_ns: 0,
            state_times_ns: [0; 6],
        }
    }
}

/// VCPU resource management
#[derive(Debug)]
pub struct VcpuResourceManager {
    /// Memory resources
    pub memory_regions: Vec<VcpuMemoryRegion>,
    /// I/O resources
    pub io_regions: Vec<VcpuIoRegion>,
    /// Device resources
    pub devices: Vec<VcpuDeviceResource>,
}

#[derive(Debug)]
pub struct VcpuMemoryRegion {
    pub start_addr: usize,
    pub size: usize,
    pub permissions: u32,
    pub name: String,
}

#[derive(Debug)]
pub struct VcpuIoRegion {
    pub start_port: u16,
    pub end_port: u16,
    pub permissions: u32,
}

#[derive(Debug)]
pub struct VcpuDeviceResource {
    pub device_id: String,
    pub config: VcpuDeviceConfig,
}

#[derive(Debug)]
pub enum VcpuDeviceConfig {
    Virtio(u32), // Device type
    Passthrough { bdf: u32 }, // Bus:Device:Function
    Emulated { dev_type: String },
}

/// VCPU wait queue management
#[derive(Debug)]
pub struct VcpuWaitQueue {
    /// Wait queue ID
    pub id: u32,
    /// Reason for waiting
    pub reason: VcpuWaitReason,
    /// Timeout in nanoseconds (0 = infinite)
    pub timeout_ns: u64,
    /// Wakeup condition
    pub wakeup_condition: Option<Box<dyn Fn(&Vcpu) -> bool>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VcpuWaitReason {
    /// Waiting for interrupt
    Interrupt,
    /// Waiting for I/O
    Io,
    /// Waiting for timer
    Timer,
    /// Waiting for memory allocation
    Memory,
    /// Waiting for lock
    Lock,
    /// Custom reason
    Custom,
}

/// Virtual CPU
pub struct Vcpu {
    /// VCPU ID (unique within a VM)
    pub id: u8,
    /// VMID this VCPU belongs to
    pub vmid: u16,
    /// VCPU name
    pub name: String,

    /// CPU state
    pub cpu_state: CpuState,
    /// Guest CSR state (legacy)
    pub guest_csr: GuestCsrState,
    /// Enhanced virtual CSR state
    pub virtual_csr: VirtualCsrState,

    /// Current VCPU state
    pub state: VcpuState,
    /// VCPU flags
    pub flags: VcpuFlags,

    /// Scheduling configuration
    pub priority: VcpuPriority,
    pub affinity: VcpuAffinity,
    pub time_mgmt: VcpuTimeManagement,

    /// Host CPU this VCPU is running on
    pub host_cpu: Option<usize>,
    /// Last state change timestamp
    pub state_tstamp: u64,

    /// Exit information
    pub exit_info: Option<VcpuExitInfo>,

    /// Statistics
    pub stats: VcpuStats,

    /// Interrupt management
    pub pending_interrupts: u64,
    pub interrupt_enable: u64,
    pub last_interrupt_time: u64,

    /// Resource management
    pub resources: VcpuResourceManager,

    /// Wait queue
    pub wait_queue: Option<VcpuWaitQueue>,

    /// Nested virtualization support
    pub nested_virt: Option<VcpuNestedVirt>,
}

/// Nested virtualization state
#[derive(Debug)]
pub struct VcpuNestedVirt {
    /// L2 VCPU configuration
    pub l2_vmid: u16,
    /// Nested CSR state
    pub nested_csr: VcpuNestedCsr,
    /// Nested page tables
    pub nested_pt: VcpuNestedPageTable,
}

#[derive(Debug)]
pub struct VcpuNestedCsr {
    pub vsstatus: usize,
    pub vstvec: usize,
    pub vsscratch: usize,
    pub vsepc: usize,
    pub vscause: usize,
    pub vstval: usize,
    pub vsatp: usize,
}

#[derive(Debug)]
pub struct VcpuNestedPageTable {
    pub root_ppn: usize,
    pub mode: crate::arch::riscv64::mmu::TranslationMode,
}

impl Vcpu {
    /// Create a new VCPU
    pub fn new(id: u8, vmid: u16, name: String, flags: VcpuFlags) -> Self {
        let current_time = Self::get_timestamp();

        Self {
            id,
            vmid,
            name,
            cpu_state: CpuState::new(),
            guest_csr: GuestCsrState::new(),
            virtual_csr: VirtualCsrState::new(vmid),
            state: VcpuState::Uninitialized,
            flags,
            priority: VcpuPriority::default(),
            affinity: VcpuAffinity::default(),
            time_mgmt: VcpuTimeManagement::default(),
            host_cpu: None,
            state_tstamp: current_time,
            exit_info: None,
            stats: VcpuStats::default(),
            pending_interrupts: 0,
            interrupt_enable: 0,
            last_interrupt_time: current_time,
            resources: VcpuResourceManager {
                memory_regions: Vec::new(),
                io_regions: Vec::new(),
                devices: Vec::new(),
            },
            wait_queue: None,
            nested_virt: None,
        }
    }

    /// Create a new VCPU with full configuration
    pub fn new_with_config(
        id: u8,
        vmid: u16,
        name: String,
        flags: VcpuFlags,
        priority: VcpuPriority,
        affinity: VcpuAffinity,
        time_slice_ns: u64,
        periodicity_ns: u64,
    ) -> Self {
        let current_time = Self::get_timestamp();
        let mut time_mgmt = VcpuTimeManagement::default();
        time_mgmt.time_slice_ns = time_slice_ns;
        time_mgmt.time_quota_ns = time_slice_ns;
        time_mgmt.periodicity_ns = periodicity_ns;

        Self {
            id,
            vmid,
            name,
            cpu_state: CpuState::new(),
            guest_csr: GuestCsrState::new(),
            virtual_csr: VirtualCsrState::new(vmid),
            state: VcpuState::Uninitialized,
            flags,
            priority,
            affinity,
            time_mgmt,
            host_cpu: None,
            state_tstamp: current_time,
            exit_info: None,
            stats: VcpuStats::default(),
            pending_interrupts: 0,
            interrupt_enable: 0,
            last_interrupt_time: current_time,
            resources: VcpuResourceManager {
                memory_regions: Vec::new(),
                io_regions: Vec::new(),
                devices: Vec::new(),
            },
            wait_queue: None,
            nested_virt: None,
        }
    }

    /// Create a new VCPU for nested virtualization
    pub fn new_nested(
        id: u8,
        vmid: u16,
        name: String,
        flags: VcpuFlags,
        l2_vmid: u16,
    ) -> Self {
        let mut vcpu = Self::new(id, vmid, name, flags);

        // Enable nested virtualization flag
        vcpu.flags.insert(VcpuFlags::NESTED_VIRTUALIZATION);

        // Initialize nested virtualization state
        vcpu.nested_virt = Some(VcpuNestedVirt {
            l2_vmid,
            nested_csr: VcpuNestedCsr {
                vsstatus: 0,
                vstvec: 0,
                vsscratch: 0,
                vsepc: 0,
                vscause: 0,
                vstval: 0,
                vsatp: 0,
            },
            nested_pt: VcpuNestedPageTable {
                root_ppn: 0,
                mode: crate::arch::riscv64::mmu::TranslationMode::Bare,
            },
        });

        vcpu
    }

    /// Get current timestamp (nanoseconds)
    fn get_timestamp() -> u64 {
        // In a real implementation, this would read from a hardware timer
        // For now, use a simple implementation
        use core::sync::atomic::{AtomicU64, Ordering};
        static TIMESTAMP: AtomicU64 = AtomicU64::new(0);
        TIMESTAMP.fetch_add(1, Ordering::Relaxed) * 1000 // Assume 1ms increment
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
        let old_state = self.state;
        let current_time = Self::get_timestamp();

        // Update state time tracking
        if let Some(state_index) = self.state_to_index(old_state) {
            self.time_mgmt.state_times_ns[state_index] += current_time - self.state_tstamp;
        }

        // Perform state transition validation and actions
        self.validate_state_transition(old_state, new_state)?;

        self.state = new_state;
        self.state_tstamp = current_time;

        log::debug!("VCPU {} state change: {:?} -> {:?}", self.id, old_state, new_state);

        // Perform state-specific actions
        self.on_state_change(old_state, new_state);
    }

    /// Validate state transition
    fn validate_state_transition(&self, from: VcpuState, to: VcpuState) -> Result<(), &'static str> {
        use VcpuState::*;

        match (from, to) {
            // Valid transitions
            (Uninitialized, Ready) => Ok(()),
            (Ready, Running) => Ok(()),
            (Running, Ready) => Ok(()),
            (Running, Blocked) => Ok(()),
            (Running, Exited) => Ok(()),
            (Blocked, Ready) => Ok(()),
            (Ready, Exited) => Ok(()),

            // Invalid transitions
            (Uninitialized, Running) => Err("Cannot transition from Uninitialized to Running"),
            (Uninitialized, Blocked) => Err("Cannot transition from Uninitialized to Blocked"),
            (Uninitialized, Exited) => Err("Cannot transition from Uninitialized to Exited"),
            (Exited, _) => Err("Cannot transition from Exited state"),

            // Allow same state (no-op)
            (state1, state2) if state1 == state2 => Ok(()),

            // All other transitions are invalid
            _ => Err("Invalid state transition"),
        }
    }

    /// Actions to perform on state change
    fn on_state_change(&mut self, old_state: VcpuState, new_state: VcpuState) {
        use VcpuState::*;

        match (old_state, new_state) {
            // Entering running state - start execution timing
            (_, Running) => {
                self.time_mgmt.last_schedule_ns = Self::get_timestamp();
            }

            // Leaving running state - update execution time
            (Running, _) => {
                let current_time = Self::get_timestamp();
                if current_time > self.time_mgmt.last_schedule_ns {
                    self.time_mgmt.total_exec_ns += current_time - self.time_mgmt.last_schedule_ns;
                }
            }

            // Exiting - cleanup resources
            (_, Exited) => {
                self.cleanup_on_exit();
            }

            _ => {} // No special action needed
        }
    }

    /// Cleanup resources when VCPU exits
    fn cleanup_on_exit(&mut self) {
        // Clear pending interrupts
        self.pending_interrupts = 0;
        self.interrupt_enable = 0;

        // Clear exit info
        self.exit_info = None;

        // Clear wait queue
        self.wait_queue = None;

        // Update statistics
        self.stats.virtual_interrupts_injected = 0;

        log::info!("VCPU {} cleaned up on exit", self.id);
    }

    /// Convert VcpuState to array index
    fn state_to_index(&self, state: VcpuState) -> Option<usize> {
        use VcpuState::*;
        match state {
            Uninitialized => Some(0),
            Ready => Some(1),
            Running => Some(2),
            Blocked => Some(3),
            Exited => Some(4),
        }
    }

    /// Get state transition history
    pub fn get_state_history(&self) -> VcpuStateHistory {
        VcpuStateHistory {
            current_state: self.state,
            state_tstamp: self.state_tstamp,
            time_in_state: self.time_mgmt.state_times_ns.iter().sum(),
            time_per_state: self.time_mgmt.state_times_ns,
        }
    }

    /// Check if VCPU can transition to a given state
    pub fn can_transition_to(&self, target_state: VcpuState) -> bool {
        self.validate_state_transition(self.state, target_state).is_ok()
    }

    /// Get time spent in current state
    pub fn get_time_in_current_state(&self) -> u64 {
        let current_time = Self::get_timestamp();
        if current_time > self.state_tstamp {
            current_time - self.state_tstamp
        } else {
            0
        }
    }

    /// Block VCPU with timeout
    pub fn block_with_timeout(&mut self, reason: VcpuWaitReason, timeout_ns: u64) -> Result<(), &'static str> {
        if !self.can_transition_to(VcpuState::Blocked) {
            return Err("Cannot transition to Blocked state");
        }

        self.wait_queue = Some(VcpuWaitQueue {
            id: self.id as u32,
            reason,
            timeout_ns,
            wakeup_condition: None,
        });

        self.set_state(VcpuState::Blocked);
        Ok(())
    }

    /// Unblock VCPU
    pub fn unblock(&mut self) -> Result<(), &'static str> {
        if self.state != VcpuState::Blocked {
            return Err("VCPU is not in Blocked state");
        }

        self.wait_queue = None;
        self.set_state(VcpuState::Ready);
        Ok(())
    }

    /// Check if VCPU is blocked for specific reason
    pub fn is_blocked_for(&self, reason: VcpuWaitReason) -> bool {
        if let (VcpuState::Blocked, Some(ref wait_queue)) = (self.state, &self.wait_queue) {
            wait_queue.reason == reason
        } else {
            false
        }
    }

    /// Wake up VCPU if wait condition is satisfied
    pub fn try_wakeup(&mut self) -> bool {
        if self.state != VcpuState::Blocked {
            return false;
        }

        if let Some(ref wait_queue) = self.wait_queue {
            // Check timeout
            let current_time = Self::get_timestamp();
            if wait_queue.timeout_ns > 0 && current_time > wait_queue.timeout_ns {
                self.unblock().unwrap();
                log::debug!("VCPU {} unblocked due to timeout", self.id);
                return true;
            }

            // Check wakeup condition
            if let Some(ref condition) = wait_queue.wakeup_condition {
                if condition(self) {
                    self.unblock().unwrap();
                    log::debug!("VCPU {} unblocked due to condition", self.id);
                    return true;
                }
            }
        }

        false
    }

    /// Reset VCPU to initial state
    pub fn reset(&mut self) -> Result<(), &'static str> {
        if self.state == VcpuState::Running {
            return Err("Cannot reset VCPU while running");
        }

        // Reset CPU state
        self.cpu_state = CpuState::new();

        // Reset CSR state
        self.guest_csr = GuestCsrState::new();
        self.virtual_csr = VirtualCsrState::new(self.vmid);

        // Reset statistics
        self.stats = VcpuStats::default();

        // Reset interrupt state
        self.pending_interrupts = 0;
        self.interrupt_enable = 0;
        self.last_interrupt_time = 0;

        // Reset time management
        self.time_mgmt = VcpuTimeManagement::default();

        // Reset exit info and wait queue
        self.exit_info = None;
        self.wait_queue = None;

        // Set to uninitialized state
        self.state = VcpuState::Uninitialized;
        self.state_tstamp = Self::get_timestamp();

        log::info!("VCPU {} reset to initial state", self.id);
        Ok(())
    }

    /// Pause VCPU execution
    pub fn pause(&mut self) -> Result<(), &'static str> {
        if self.state != VcpuState::Running {
            return Err("VCPU is not running");
        }

        self.set_state(VcpuState::Blocked);
        self.wait_queue = Some(VcpuWaitQueue {
            id: self.id as u32,
            reason: VcpuWaitReason::Custom,
            timeout_ns: 0, // Infinite
            wakeup_condition: None,
        });

        log::info!("VCPU {} paused", self.id);
        Ok(())
    }

    /// Resume VCPU execution
    pub fn resume(&mut self) -> Result<(), &'static str> {
        if self.state != VcpuState::Blocked {
            return Err("VCPU is not paused/blocked");
        }

        self.wait_queue = None;
        self.set_state(VcpuState::Ready);

        log::info!("VCPU {} resumed", self.id);
        Ok(())
    }

    /// Shutdown VCPU gracefully
    pub fn shutdown(&mut self) -> Result<(), &'static str> {
        if self.state == VcpuState::Exited {
            return Err("VCPU is already shut down");
        }

        // Set exit reason to normal shutdown
        self.exit_info = Some(VcpuExitInfo {
            reason: VcpuExitReason::Unknown,
            trap_cause: 0,
            trap_val: 0,
            instruction: 0,
        });

        self.set_state(VcpuState::Exited);
        log::info!("VCPU {} shut down", self.id);
        Ok(())
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
        // Save guest CSR state using enhanced virtual CSR
        self.virtual_csr = VirtualCsrState::save_from_hw(self.vmid)?;

        // Also update legacy guest CSR for compatibility
        self.guest_csr = GuestCsrState::save();

        // Update statistics
        self.stats.instructions_executed += read_csr!(crate::arch::riscv64::cpu::csr::MINSTRET);
        self.stats.cycles_spent += read_csr!(crate::arch::riscv64::cpu::csr::MCYCLE);

        log::debug!("Saved state for VCPU {} using VirtualCsrState", self.id);
        Ok(())
    }

    /// Restore VCPU state
    pub fn restore_state(&self) -> Result<(), &'static str> {
        // Restore using enhanced virtual CSR
        self.virtual_csr.restore_to_hw()?;

        // Also update legacy guest CSR for compatibility
        self.guest_csr.load();

        // In a real implementation, this would also restore:
        // - General purpose registers
        // - Floating point registers
        // - Vector registers (if V extension is enabled)

        log::debug!("Restored state for VCPU {} using VirtualCsrState", self.id);
        Ok(())
    }

    /// Save VCPU state with validation
    pub fn save_state_validated(&mut self) -> Result<(), &'static str> {
        // Save state
        self.save_state()?;

        // Validate saved state
        self.virtual_csr.validate().map_err(|e| {
            log::error!("Validation failed for VCPU {} state: {}", self.id, e);
            e
        })?;

        log::debug!("Saved and validated state for VCPU {}", self.id);
        Ok(())
    }

    /// Restore VCPU state with optimized switching
    pub fn restore_state_optimized(&self, from_state: &VirtualCsrState) -> Result<(), &'static str> {
        // Use optimized state switching
        switch_state(from_state, &self.virtual_csr)?;

        log::debug!("Optimized state restore for VCPU {}", self.id);
        Ok(())
    }

    /// Get virtual CSR state
    pub fn get_virtual_csr(&self) -> &VirtualCsrState {
        &self.virtual_csr
    }

    /// Get mutable virtual CSR state
    pub fn get_virtual_csr_mut(&mut self) -> &mut VirtualCsrState {
        &mut self.virtual_csr
    }

    /// Update virtual CSR from legacy state
    pub fn sync_virtual_from_legacy(&mut self) {
        self.virtual_csr = VirtualCsrState::from(self.guest_csr.clone());
        self.virtual_csr.vmid = self.vmid;
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

/// VCPU state history information
#[derive(Debug, Clone)]
pub struct VcpuStateHistory {
    /// Current VCPU state
    pub current_state: VcpuState,
    /// Timestamp of last state change
    pub state_tstamp: u64,
    /// Total time spent across all states
    pub time_in_state: u64,
    /// Time spent in each state
    pub time_per_state: [u64; 5], // One for each non-Exited state
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

        let vcpu = Vcpu::new(vcpu_id, vmid, format!("vcpu-{}", vcpu_id), flags);
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

    // ===== SCHEDULING INTERFACES =====

  
    /// Get VCPU with highest priority that is ready
    pub fn get_highest_priority_ready_vcpu(&self) -> Option<&Vcpu> {
        let mut best_vcpu = None;
        let mut best_priority = VcpuPriority::Idle;

        for vcpu in &self.vcpus {
            if vcpu.is_ready() && vcpu.priority > best_priority {
                best_vcpu = Some(vcpu);
                best_priority = vcpu.priority;
            }
        }

        best_vcpu
    }

    /// Get next VCPU based on scheduling policy
    pub fn get_next_vcpu(&mut self, policy: VcpuSchedulingPolicy) -> Option<&mut Vcpu> {
        match policy {
            VcpuSchedulingPolicy::RoundRobin => self.get_next_ready_vcpu(),
            VcpuSchedulingPolicy::Priority => {
                if let Some(best_vcpu) = self.get_highest_priority_ready_vcpu() {
                    self.get_vcpu_mut(best_vcpu.id)
                } else {
                    self.get_next_ready_vcpu()
                }
            }
            VcpuSchedulingPolicy::Fair => self.get_next_vcpu_fair(),
            VcpuSchedulingPolicy::RealTime => self.get_next_vcpu_realtime(),
        }
    }

    /// Get next VCPU using fair scheduling
    fn get_next_vcpu_fair(&mut self) -> Option<&mut Vcpu> {
        // Find VCPU with least execution time
        let mut best_vcpu = None;
        let mut min_exec_time = u64::MAX;

        for vcpu in &mut self.vcpus {
            if vcpu.is_ready() && vcpu.time_mgmt.total_exec_ns < min_exec_time {
                best_vcpu = Some(vcpu);
                min_exec_time = vcpu.time_mgmt.total_exec_ns;
            }
        }

        best_vcpu
    }

    /// Get next VCPU for real-time scheduling
    fn get_next_vcpu_realtime(&mut self) -> Option<&mut Vcpu> {
        // Prioritize real-time VCPUs
        let mut best_vcpu = None;
        let mut highest_deadline = u64::MAX;

        let current_time = Vcpu::get_timestamp();

        for vcpu in &mut self.vcpus {
            if vcpu.is_ready() && vcpu.priority >= VcpuPriority::High {
                let deadline = if vcpu.time_mgmt.deadline_ns > 0 {
                    vcpu.time_mgmt.deadline_ns
                } else {
                    current_time + vcpu.time_mgmt.time_slice_ns
                };

                if deadline < highest_deadline {
                    best_vcpu = Some(vcpu);
                    highest_deadline = deadline;
                }
            }
        }

        // If no real-time VCPU, fall back to priority scheduling
        if best_vcpu.is_none() {
            best_vcpu = self.get_next_vcpu(VcpuSchedulingPolicy::Priority);
        }

        best_vcpu
    }

    /// Preempt current VCPU if a higher priority VCPU becomes ready
    pub fn check_preemption(&mut self) -> bool {
        let current_id = self.current_vcpu?;
        let current_vcpu = self.get_vcpu(current_id)?;

        if !current_vcpu.is_running() {
            return false; // Current VCPU is not running
        }

        // Check if there's a higher priority VCPU ready
        if let Some(higher_vcpu) = self.get_highest_priority_ready_vcpu() {
            if higher_vcpu.priority > current_vcpu.priority {
                // Preempt current VCPU
                current_vcpu.set_state(VcpuState::Ready);
                higher_vcpu.set_state(VcpuState::Running);
                self.current_vcpu = Some(higher_vcpu.id);

                log::info!("Preempted VCPU {} for higher priority VCPU {}",
                          current_id, higher_vcpu.id);
                return true;
            }
        }

        false
    }

    /// Update time quotas for all VCPUs
    pub fn update_time_quotas(&mut self, delta_ns: u64) {
        for vcpu in &mut self.vcpus {
            if vcpu.time_mgmt.time_quota_ns > 0 {
                vcpu.time_mgmt.time_quota_ns = vcpu.time_mgmt.time_quota_ns.saturating_sub(delta_ns);
            }

            // Update deadline if periodic
            if vcpu.time_mgmt.periodicity_ns > 0 {
                vcpu.time_mgmt.deadline_ns = vcpu.time_mgmt.deadline_ns.saturating_add(delta_ns);
                if vcpu.time_mgmt.deadline_ns >= vcpu.time_mgmt.periodicity_ns {
                    vcpu.time_mgmt.deadline_ns = 0; // Reset for next period
                }
            }
        }
    }

    /// Get VCPUs that need scheduling (time quota exhausted)
    pub fn get_vcpus_needing_schedule(&self) -> Vec<&Vcpu> {
        let current_time = Vcpu::get_timestamp();
        self.vcpus.iter()
            .filter(|v| {
                v.is_ready() && (
                    v.time_mgmt.time_quota_ns == 0 ||
                    (v.time_mgmt.deadline_ns > 0 && current_time >= v.time_mgmt.deadline_ns)
                )
            })
            .collect()
    }

    /// Balance VCPUs across available host CPUs
    pub fn balance_vcpus(&mut self, available_cpus: usize) -> Result<(), &'static str> {
        if available_cpus == 0 {
            return Err("No available CPUs");
        }

        // Simple load balancing based on current host CPU assignment
        let mut cpu_loads = vec![0usize; available_cpus];
        for vcpu in &self.vcpus {
            if let Some(host_cpu) = vcpu.host_cpu {
                if host_cpu < available_cpus {
                    cpu_loads[host_cpu] += 1;
                }
            }
        }

        // Find least loaded CPU and reassign some VCPUs
        for vcpu in &mut self.vcpus {
            let current_cpu = vcpu.host_cpu.unwrap_or(0);

            // If current CPU is overloaded and VCPU can migrate
            if cpu_loads[current_cpu] > cpu_loads.iter().min().unwrap() + 1 &&
               vcpu.affinity.allow_migration &&
               vcpu.host_cpu.is_none() || vcpu.affinity.allow_migration {

                let least_loaded_cpu = cpu_loads.iter()
                    .enumerate()
                    .min_by_key(|&(_, &load)| *load)
                    .map(|(idx, _)| idx)
                    .unwrap();

                if ((1 << least_loaded_cpu) & vcpu.affinity.cpu_mask) != 0 {
                    if let Some(old_cpu) = vcpu.host_cpu {
                        cpu_loads[old_cpu] -= 1;
                    }
                    vcpu.host_cpu = Some(least_loaded_cpu);
                    cpu_loads[least_loaded_cpu] += 1;
                    log::debug!("Balanced VCPU {} to CPU {}", vcpu.id, least_loaded_cpu);
                }
            }
        }

        Ok(())
    }

    /// Get scheduling statistics
    pub fn get_scheduling_stats(&self) -> VcpuSchedulingStats {
        let mut stats = VcpuSchedulingStats::default();

        for vcpu in &self.vcpus {
            stats.total_vcpus += 1;

            match vcpu.state {
                VcpuState::Uninitialized => stats.uninitialized += 1,
                VcpuState::Ready => stats.ready += 1,
                VcpuState::Running => stats.running += 1,
                VcpuState::Blocked => stats.blocked += 1,
                VcpuState::Exited => stats.exited += 1,
            }

            if let Some(host_cpu) = vcpu.host_cpu {
                if host_cpu < stats.vcpus_per_cpu.len() {
                    stats.vcpus_per_cpu[host_cpu] += 1;
                }
            }

            stats.total_instructions += vcpu.stats.instructions_executed;
            stats.total_exits += vcpu.stats.hypervisor_traps;
            stats.total_interrupts += vcpu.stats.virtual_interrupts_injected;

            stats.total_exec_time_ns += vcpu.time_mgmt.total_exec_ns;
        }

        stats.average_exec_time_per_vcpu = if stats.total_vcpus > 0 {
            stats.total_exec_time_ns / stats.total_vcpus as u64
        } else {
            0
        };

        stats
    }
  // ===== ADVANCED VCPU MANAGEMENT METHODS =====

    /// Get mutable VCPU by ID (helper method)
    fn get_vcpu_mut(&mut self, vcpu_id: u8) -> Option<&mut Vcpu> {
        self.vcpus.iter_mut().find(|v| v.id == vcpu_id)
    }

    /// Allocate a new VCPU with full configuration
    pub fn allocate_vcpu_with_config(
        &mut self,
        vmid: u16,
        name: String,
        flags: VcpuFlags,
        priority: VcpuPriority,
        affinity: VcpuAffinity,
        time_slice_ns: u64,
        periodicity_ns: u64,
    ) -> Result<&mut Vcpu, &'static str> {
        if self.next_vcpu_id >= 16 {
            return Err("Maximum VCPUs reached");
        }

        let vcpu_id = self.next_vcpu_id;
        self.next_vcpu_id += 1;

        let vcpu = Vcpu::new_with_config(
            vcpu_id, vmid, name, flags, priority, affinity, time_slice_ns, periodicity_ns
        );
        self.vcpus.push(vcpu);

        Ok(&mut self.vcpus[self.vcpus.len() - 1])
    }

    /// Allocate a new VCPU for nested virtualization
    pub fn allocate_nested_vcpu(
        &mut self,
        vmid: u16,
        name: String,
        flags: VcpuFlags,
        l2_vmid: u16,
    ) -> Result<&mut Vcpu, &'static str> {
        if self.next_vcpu_id >= 16 {
            return Err("Maximum VCPUs reached");
        }

        let vcpu_id = self.next_vcpu_id;
        self.next_vcpu_id += 1;

        let vcpu = Vcpu::new_nested(vcpu_id, vmid, name, flags, l2_vmid);
        self.vcpus.push(vcpu);

        Ok(&mut self.vcpus[self.vcpus.len() - 1])
    }

    /// Create orphan VCPU (not attached to any VM)
    pub fn create_orphan_vcpu(
        &mut self,
        name: String,
        entry_pc: usize,
        stack_size: usize,
        flags: VcpuFlags,
        priority: VcpuPriority,
    ) -> Result<&mut Vcpu, &'static str> {
        if self.next_vcpu_id >= 16 {
            return Err("Maximum VCPUs reached");
        }

        let vcpu_id = self.next_vcpu_id;
        self.next_vcpu_id += 1;

        // Create VCPU with VMID 0 (orphan)
        let mut vcpu = Vcpu::new_with_config(
            vcpu_id, 0, name, flags, priority,
            VcpuAffinity::default(), 10_000_000, 0
        );

        // Initialize with entry point and stack
        // In a real implementation, we'd allocate stack here
        let stack_pointer = 0x80000000 + stack_size; // Placeholder
        vcpu.init(entry_pc, stack_pointer)?;

        self.vcpus.push(vcpu);
        Ok(&mut self.vcpus[self.vcpus.len() - 1])
    }

    /// Destroy orphan VCPU
    pub fn destroy_orphan_vcpu(&mut self, vcpu_id: u8) -> Result<(), &'static str> {
        let index = self.vcpus.iter().position(|v| v.id == vcpu_id && v.vmid == 0)
            .ok_or("Orphan VCPU not found")?;

        // If this is the current VCPU, clear it
        if self.current_vcpu == Some(vcpu_id) {
            self.current_vcpu = None;
        }

        // Remove VCPU
        self.vcpus.remove(index);

        log::info!("Destroyed orphan VCPU {}", vcpu_id);
        Ok(())
    }

    /// Cleanup all VCPUs for a specific VM
    pub fn cleanup_vm_vcpus(&mut self, vmid: u16) -> Result<usize, &'static str> {
        let initial_count = self.vcpus.len();

        // Remove all VCPUs belonging to the VM
        self.vcpus.retain(|v| v.vmid != vmid);

        // Clear current VCPU if it belonged to the VM
        if let Some(current_id) = self.current_vcpu {
            if self.get_vcpu(current_id).map(|v| v.vmid) == Some(vmid) {
                self.current_vcpu = None;
            }
        }

        let removed_count = initial_count - self.vcpus.len();
        if removed_count > 0 {
            log::info!("Cleaned up {} VCPUs for VMID {}", removed_count, vmid);
        }

        Ok(removed_count)
    }

    /// Clone VCPU configuration
    pub fn clone_vcpu(
        &mut self,
        source_vcpu_id: u8,
        new_vcpu_id: u8,
        new_name: String,
    ) -> Result<&mut Vcpu, &'static str> {
        if self.next_vcpu_id <= new_vcpu_id {
            self.next_vcpu_id = new_vcpu_id + 1;
        }

        let source_vcpu = self.get_vcpu(source_vcpu_id)
            .ok_or("Source VCPU not found")?;

        let mut new_vcpu = Vcpu::new(
            new_vcpu_id,
            source_vcpu.vmid,
            new_name,
            source_vcpu.flags,
        );

        // Copy configuration
        new_vcpu.priority = source_vcpu.priority;
        new_vcpu.affinity = source_vcpu.affinity.clone();
        new_vcpu.time_mgmt = source_vcpu.time_mgmt;

        // Copy CPU state (but reset execution context)
        new_vcpu.cpu_state = source_vcpu.cpu_state.clone();

        // Copy CSR state
        new_vcpu.guest_csr = source_vcpu.guest_csr.clone();
        new_vcpu.virtual_csr = source_vcpu.virtual_csr.clone();

        // Copy resources
        new_vcpu.resources = VcpuResourceManager {
            memory_regions: source_vcpu.resources.memory_regions.clone(),
            io_regions: source_vcpu.resources.io_regions.clone(),
            devices: source_vcpu.resources.devices.clone(),
        };

        self.vcpus.push(new_vcpu);
        Ok(&mut self.vcpus[self.vcpus.len() - 1])
    }

    /// Migrate VCPU to different host CPU
    pub fn migrate_vcpu(&mut self, vcpu_id: u8, target_host_cpu: usize) -> Result<(), &'static str> {
        let vcpu = self.get_vcpu_mut(vcpu_id)
            .ok_or("VCPU not found")?;

        // Check if target CPU is in affinity mask
        if !vcpu.affinity.allow_migration &&
           vcpu.host_cpu.is_some() &&
           vcpu.host_cpu.unwrap() != target_host_cpu {
            return Err("VCPU migration not allowed");
        }

        if ((1 << target_host_cpu) & vcpu.affinity.cpu_mask) == 0 {
            return Err("Target CPU not in VCPU affinity mask");
        }

        let old_host_cpu = vcpu.host_cpu;
        vcpu.host_cpu = Some(target_host_cpu);

        log::info!("Migrated VCPU {} from host CPU {:?} to {}",
                  vcpu_id, old_host_cpu, target_host_cpu);
        Ok(())
    }

    /// Get VCPUs by priority
    pub fn get_vcpus_by_priority(&self, priority: VcpuPriority) -> Vec<&Vcpu> {
        self.vcpus.iter()
            .filter(|v| v.priority == priority)
            .collect()
    }

    /// Get VCPUs on specific host CPU
    pub fn get_vcpus_on_host_cpu(&self, host_cpu: usize) -> Vec<&Vcpu> {
        self.vcpus.iter()
            .filter(|v| v.host_cpu == Some(host_cpu))
            .collect()
    }

    /// Get VCPU execution statistics summary
    pub fn get_execution_summary(&self) -> VcpuExecutionSummary {
        let mut total_instructions = 0;
        let mut total_cycles = 0;
        let mut total_exec_time = 0;
        let mut total_exits = 0;
        let mut state_counts = [0usize; 6]; // One for each VcpuState

        for vcpu in &self.vcpus {
            total_instructions += vcpu.stats.instructions_executed;
            total_cycles += vcpu.stats.cycles_spent;
            total_exits += vcpu.stats.hypervisor_traps;
            total_exec_time += vcpu.time_mgmt.total_exec_ns;

            // Count states (convert VcpuState enum to index)
            let state_index = match vcpu.state {
                VcpuState::Uninitialized => 0,
                VcpuState::Ready => 1,
                VcpuState::Running => 2,
                VcpuState::Blocked => 3,
                VcpuState::Exited => 4,
            };
            if state_index < state_counts.len() {
                state_counts[state_index] += 1;
            }
        }

        VcpuExecutionSummary {
            total_vcpus: self.vcpus.len(),
            total_instructions,
            total_cycles,
            total_exec_time_ns: total_exec_time,
            total_exits,
            state_counts,
            average_instructions_per_vcpu: if self.vcpus.len() > 0 {
                total_instructions / self.vcpus.len() as u64
            } else {
                0
            },
        }
    }
}

/// VCPU scheduling policies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VcpuSchedulingPolicy {
    /// Round-robin scheduling
    RoundRobin,
    /// Priority-based scheduling
    Priority,
    /// Fair scheduling (least execution time)
    Fair,
    /// Real-time scheduling (earliest deadline first)
    RealTime,
}

/// VCPU scheduling statistics
#[derive(Debug, Default, Clone)]
pub struct VcpuSchedulingStats {
    pub total_vcpus: usize,
    pub uninitialized: usize,
    pub ready: usize,
    pub running: usize,
    pub blocked: usize,
    pub exited: usize,
    pub vcpus_per_cpu: [usize; 64], // Support up to 64 CPUs
    pub total_instructions: u64,
    pub total_exits: u64,
    pub total_interrupts: u64,
    pub total_exec_time_ns: u64,
    pub average_exec_time_per_vcpu: u64,
}

/// VCPU execution summary statistics
#[derive(Debug, Clone)]
pub struct VcpuExecutionSummary {
    pub total_vcpus: usize,
    pub total_instructions: u64,
    pub total_cycles: u64,
    pub total_exec_time_ns: u64,
    pub total_exits: u64,
    pub state_counts: [usize; 6],
    pub average_instructions_per_vcpu: u64,
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
        let vcpu = Vcpu::new(0, 100, "test-vcpu".to_string(), VcpuFlags::VIRTUAL_INTERRUPTS);

        assert_eq!(vcpu.id, 0);
        assert_eq!(vcpu.vmid, 100);
        assert_eq!(vcpu.state, VcpuState::Uninitialized);
        assert!(vcpu.flags.contains(VcpuFlags::VIRTUAL_INTERRUPTS));
    }

    #[test]
    fn test_vcpu_initialization() {
        let mut vcpu = Vcpu::new(0, 100, "test-vcpu".to_string(), VcpuFlags::empty());

        vcpu.init(0x80000000, 0x90000000).unwrap();

        assert_eq!(vcpu.state, VcpuState::Ready);
        assert_eq!(vcpu.cpu_state.get_pc(), 0x80000000);
        assert_eq!(vcpu.cpu_state.get_sp(), 0x90000000);
        assert_eq!(vcpu.cpu_state.get_privilege(), crate::arch::riscv64::PrivilegeLevel::Supervisor);
    }

    #[test]
    fn test_virtual_interrupts() {
        let mut vcpu = Vcpu::new(0, 100, "test-vcpu".to_string(), VcpuFlags::VIRTUAL_INTERRUPTS);

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

    #[test]
    fn test_vcpu_virtual_csr_state() {
        let mut vcpu = Vcpu::new(0, 1, "test-vcpu".to_string(), VcpuFlags::VIRTUAL_INTERRUPTS);

        // Test initial virtual CSR state
        assert_eq!(vcpu.virtual_csr.vmid, 1);
        assert_eq!(vcpu.virtual_csr.vsstatus, VsstatusFlags::empty());

        // Test state synchronization
        vcpu.sync_virtual_from_legacy();
        assert_eq!(vcpu.virtual_csr.vmid, vcpu.vmid);

        // Test virtual CSR access
        let virtual_csr = vcpu.get_virtual_csr();
        assert_eq!(virtual_csr.vmid, 1);

        let virtual_csr_mut = vcpu.get_virtual_csr_mut();
        virtual_csr_mut.set_vsstatus_raw(0x80000001);
        assert_eq!(virtual_csr_mut.get_vsstatus_raw(), 0x80000001);
    }

    #[test]
    fn test_vcpu_priority() {
        assert!(VcpuPriority::RealTime > VcpuPriority::High);
        assert!(VcpuPriority::High > VcpuPriority::Normal);
        assert!(VcpuPriority::Normal > VcpuPriority::Low);
        assert!(VcpuPriority::Low > VcpuPriority::Idle);
        assert_eq!(VcpuPriority::default(), VcpuPriority::Normal);
    }

    #[test]
    fn test_vcpu_affinity() {
        let affinity = VcpuAffinity {
            cpu_mask: 0b1010,
            preferred_cpu: Some(1),
            allow_migration: false,
        };

        assert_eq!(affinity.cpu_mask, 0b1010);
        assert_eq!(affinity.preferred_cpu, Some(1));
        assert!(!affinity.allow_migration);

        // Test default affinity
        let default_affinity = VcpuAffinity::default();
        assert_eq!(default_affinity.cpu_mask, u64::MAX);
        assert_eq!(default_affinity.preferred_cpu, None);
        assert!(default_affinity.allow_migration);
    }

    #[test]
    fn test_vcpu_time_management() {
        let time_mgmt = VcpuTimeManagement {
            time_slice_ns: 5_000_000,
            time_quota_ns: 5_000_000,
            deadline_ns: 0,
            periodicity_ns: 1_000_000,
            last_schedule_ns: 0,
            total_exec_ns: 0,
            state_times_ns: [0; 6],
        };

        assert_eq!(time_mgmt.time_slice_ns, 5_000_000);
        assert_eq!(time_mgmt.periodicity_ns, 1_000_000);

        // Test default time management
        let default_time_mgmt = VcpuTimeManagement::default();
        assert_eq!(default_time_mgmt.time_slice_ns, 10_000_000);
        assert_eq!(default_time_mgmt.periodicity_ns, 0); // Aperiodic
    }

    #[test]
    fn test_vcpu_creation_with_config() {
        let vcpu = Vcpu::new_with_config(
            1,
            100,
            "config-test-vcpu".to_string(),
            VcpuFlags::VIRTUAL_INTERRUPTS,
            VcpuPriority::High,
            VcpuAffinity {
                cpu_mask: 0b0011,
                preferred_cpu: Some(0),
                allow_migration: true,
            },
            5_000_000,
            1_000_000,
        );

        assert_eq!(vcpu.id, 1);
        assert_eq!(vcpu.vmid, 100);
        assert_eq!(vcpu.name, "config-test-vcpu");
        assert_eq!(vcpu.priority, VcpuPriority::High);
        assert_eq!(vcpu.affinity.cpu_mask, 0b0011);
        assert_eq!(vcpu.time_mgmt.time_slice_ns, 5_000_000);
        assert_eq!(vcpu.time_mgmt.periodicity_ns, 1_000_000);
    }

    #[test]
    fn test_nested_vcpu_creation() {
        let vcpu = Vcpu::new_nested(
            2,
            200,
            "nested-vcpu".to_string(),
            VcpuFlags::NESTED_VIRTUALIZATION,
            300,
        );

        assert_eq!(vcpu.id, 2);
        assert_eq!(vcpu.vmid, 200);
        assert!(vcpu.flags.contains(VcpuFlags::NESTED_VIRTUALIZATION));
        assert!(vcpu.nested_virt.is_some());

        let nested = vcpu.nested_virt.unwrap();
        assert_eq!(nested.l2_vmid, 300);
    }

    #[test]
    fn test_vcpu_manager_advanced_operations() {
        let mut manager = VcpuManager::new();

        // Create VCPU with full config
        let vcpu1 = manager.allocate_vcpu_with_config(
            1,
            "vcpu1".to_string(),
            VcpuFlags::VIRTUAL_INTERRUPTS,
            VcpuPriority::High,
            VcpuAffinity::default(),
            10_000_000,
            0,
        ).unwrap();

        // Create nested VCPU
        let vcpu2 = manager.allocate_nested_vcpu(
            1,
            "nested-vcpu".to_string(),
            VcpuFlags::NESTED_VIRTUALIZATION,
            2,
        ).unwrap();

        // Create orphan VCPU
        let vcpu3 = manager.create_orphan_vcpu(
            "orphan-vcpu".to_string(),
            0x80000000,
            0x10000,
            VcpuFlags::empty(),
            VcpuPriority::Low,
        ).unwrap();

        assert_eq!(manager.vcpu_count(), 3);
        assert_eq!(vcpu1.vmid, 1);
        assert_eq!(vcpu2.vmid, 1);
        assert_eq!(vcpu3.vmid, 0); // Orphan VCPU

        // Test VCPU cloning
        let vcpu4 = manager.clone_vcpu(
            vcpu1.id,
            4,
            "cloned-vcpu".to_string(),
        ).unwrap();

        assert_eq!(vcpu4.id, 4);
        assert_eq!(vcpu4.vmid, vcpu1.vmid);
        assert_eq!(vcpu4.priority, vcpu1.priority);

        // Test VCPU migration
        manager.migrate_vcpu(vcpu1.id, 2).unwrap();
        assert_eq!(vcpu1.host_cpu, Some(2));

        // Test cleanup
        let removed = manager.cleanup_vm_vcpus(1).unwrap();
        assert_eq!(removed, 2); // vcpu1 and vcpu2 removed

        // Test orphan VCPU destruction
        manager.destroy_orphan_vcpu(vcpu3.id).unwrap();
        assert_eq!(manager.vcpu_count(), 1); // Only cloned vcpu4 remains
    }

    #[test]
    fn test_vcpu_resource_management() {
        let mut vcpu = Vcpu::new(
            0, 100, "resource-test".to_string(), VcpuFlags::empty());

        // Add memory region
        vcpu.resources.memory_regions.push(VcpuMemoryRegion {
            start_addr: 0x80000000,
            size: 0x1000000,
            permissions: 0x7, // RWX
            name: "test-memory".to_string(),
        });

        // Add I/O region
        vcpu.resources.io_regions.push(VcpuIoRegion {
            start_port: 0x3F8,
            end_port: 0x3FF,
            permissions: 0x3, // RW
        });

        // Add device
        vcpu.resources.devices.push(VcpuDeviceResource {
            device_id: "virtio-blk".to_string(),
            config: VcpuDeviceConfig::Virtio(1),
        });

        assert_eq!(vcpu.resources.memory_regions.len(), 1);
        assert_eq!(vcpu.resources.io_regions.len(), 1);
        assert_eq!(vcpu.resources.devices.len(), 1);
    }

    #[test]
    fn test_vcpu_wait_queue() {
        let wait_queue = VcpuWaitQueue {
            id: 1,
            reason: VcpuWaitReason::Interrupt,
            timeout_ns: 1_000_000,
            wakeup_condition: None,
        };

        assert_eq!(wait_queue.id, 1);
        assert_eq!(wait_queue.reason, VcpuWaitReason::Interrupt);
        assert_eq!(wait_queue.timeout_ns, 1_000_000);
    }

    #[test]
    fn test_execution_summary() {
        let mut manager = VcpuManager::new();

        // Create some VCPUs
        manager.allocate_vcpu(1, VcpuFlags::empty()).unwrap();
        manager.allocate_vcpu(1, VcpuFlags::empty()).unwrap();

        let summary = manager.get_execution_summary();
        assert_eq!(summary.total_vcpus, 2);
        assert_eq!(summary.state_counts[0], 2); // All Uninitialized
    }
}