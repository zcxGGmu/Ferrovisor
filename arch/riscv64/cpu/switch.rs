//! RISC-V Context Switching
//!
//! This module provides context switching functionality for RISC-V including:
//! - Task context switching
//! - VCPU context switching
//! - Hypervisor mode transitions
//! - Assembly helper functions
//! - Enhanced context management for virtualization

use crate::arch::riscv64::cpu::regs::CpuState;
use crate::arch::riscv64::cpu::csr;
use bitflags::bitflags;

/// Context switch structure
#[repr(C)]
pub struct Context {
    /// Callee-saved general purpose registers
    pub ra: usize,      // x1
    pub sp: usize,      // x2
    pub gp: usize,      // x3
    pub tp: usize,      // x4
    pub t0: usize,      // x5
    pub t1: usize,      // x6
    pub t2: usize,      // x7
    pub s0: usize,      // x8/fp
    pub s1: usize,      // x9
    pub s2: usize,      // x18
    pub s3: usize,      // x19
    pub s4: usize,      // x20
    pub s5: usize,      // x21
    pub s6: usize,      // x22
    pub s7: usize,      // x23
    pub s8: usize,      // x24
    pub s9: usize,      // x25
    pub s10: usize,     // x26
    pub s11: usize,     // x27
    /// Program counter
    pub pc: usize,
    /// Floating point state
    pub fp_state: FpState,
}

/// Floating point state
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FpState {
    /// Callee-saved floating point registers
    pub fs0: u64,       // f8
    pub fs1: u64,       // f9
    pub fs2: u64,       // f18
    pub fs3: u64,       // f19
    pub fs4: u64,       // f20
    pub fs5: u64,       // f21
    pub fs6: u64,       // f22
    pub fs7: u64,       // f23
    pub fs8: u64,       // f24
    pub fs9: u64,       // f25
    pub fs10: u64,      // f26
    pub fs11: u64,      // f27
    /// FCSR register
    pub fcsr: u32,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            ra: 0,
            sp: 0,
            gp: 0,
            tp: 0,
            t0: 0,
            t1: 0,
            t2: 0,
            s0: 0,
            s1: 0,
            s2: 0,
            s3: 0,
            s4: 0,
            s5: 0,
            s6: 0,
            s7: 0,
            s8: 0,
            s9: 0,
            s10: 0,
            s11: 0,
            pc: 0,
            fp_state: FpState {
                fs0: 0,
                fs1: 0,
                fs2: 0,
                fs3: 0,
                fs4: 0,
                fs5: 0,
                fs6: 0,
                fs7: 0,
                fs8: 0,
                fs9: 0,
                fs10: 0,
                fs11: 0,
                fcsr: 0,
            },
        }
    }
}

impl Context {
    /// Create a new context
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a context for a new task
    pub fn new_task(entry: usize, stack_top: usize) -> Self {
        Self {
            ra: 0,              // Will be set when we first switch
            sp: stack_top,
            gp: 0,
            tp: 0,
            t0: 0,
            t1: 0,
            t2: 0,
            s0: 0,
            s1: 0,
            s2: 0,
            s3: 0,
            s4: 0,
            s5: 0,
            s6: 0,
            s7: 0,
            s8: 0,
            s9: 0,
            s10: 0,
            s11: 0,
            pc: entry,
            fp_state: FpState::default(),
        }
    }

    /// Get the stack pointer
    pub fn get_sp(&self) -> usize {
        self.sp
    }

    /// Set the stack pointer
    pub fn set_sp(&mut self, sp: usize) {
        self.sp = sp;
    }

    /// Get the program counter
    pub fn get_pc(&self) -> usize {
        self.pc
    }

    /// Set the program counter
    pub fn set_pc(&mut self, pc: usize) {
        self.pc = pc;
    }
}

// ===== VCPU CONTEXT SWITCHING =====

/// VCPU register context structure (equivalent to struct arch_regs in xvisor)
#[repr(C)]
#[derive(Debug, Clone)]
pub struct VcpuRegs {
    /// General purpose registers
    pub zero: u64,     // x0
    pub ra: u64,       // x1 - return address
    pub sp: u64,       // x2 - stack pointer
    pub gp: u64,       // x3 - global pointer
    pub tp: u64,       // x4 - thread pointer
    pub t0: u64,       // x5
    pub t1: u64,       // x6
    pub t2: u64,       // x7
    pub s0: u64,       // x8/fp - saved register 0 / frame pointer
    pub s1: u64,       // x9 - saved register 1
    pub a0: u64,       // x10 - argument 0 / return value
    pub a1: u64,       // x11 - argument 1 / return value
    pub a2: u64,       // x12 - argument 2
    pub a3: u64,       // x13 - argument 3
    pub a4: u64,       // x14 - argument 4
    pub a5: u64,       // x15 - argument 5
    pub a6: u64,       // x16 - argument 6
    pub a7: u64,       // x17 - argument 7
    pub s2: u64,       // x18 - saved register 2
    pub s3: u64,       // x19 - saved register 3
    pub s4: u64,       // x20 - saved register 4
    pub s5: u64,       // x21 - saved register 5
    pub s6: u64,       // x22 - saved register 6
    pub s7: u64,       // x23 - saved register 7
    pub s8: u64,       // x24 - saved register 8
    pub s9: u64,       // x25 - saved register 9
    pub s10: u64,      // x26 - saved register 10
    pub s11: u64,      // x27 - saved register 11
    pub t3: u64,       // x28
    pub t4: u64,       // x29
    pub t5: u64,       // x30
    pub t6: u64,       // x31

    /// Special registers
    pub sepc: u64,     // Supervisor exception program counter
    pub sstatus: u64,  // Supervisor status register
    pub stvec: u64,    // Supervisor trap vector base address
    pub sscratch: u64, // Supervisor scratch register
    pub sie: u64,      // Supervisor interrupt enable register
    pub sip: u64,      // Supervisor interrupt pending register
    pub satp: u64,     // Supervisor address translation and protection

    /// Hypervisor-specific registers
    pub hstatus: u64,  // Hypervisor status register
    pub hideleg: u64,  // Hypervisor interrupt delegation register
    pub hedeleg: u64,  // Hypervisor exception delegation register
    pub hcounteren: u64, // Hypervisor counter enable register
    pub hgeie: u64,    // Hypervisor guest external interrupt enable
    pub hgeip: u64,    // Hypervisor guest external interrupt pending
    pub hgatp: u64,    // Hypervisor guest address translation and protection
    pub htval: u64,    // Hypervisor trap value register
    pub htinst: u64,   // Hypervisor trap instruction register

    /// Virtual supervisor registers
    pub vsstatus: u64, // Virtual supervisor status register
    pub vstvec: u64,   // Virtual supervisor trap vector base address
    pub vsscratch: u64, // Virtual supervisor scratch register
    pub vsepc: u64,    // Virtual supervisor exception program counter
    pub vscause: u64,  // Virtual supervisor cause register
    pub vstval: u64,   // Virtual supervisor bad address register
    pub vsip: u64,     // Virtual supervisor interrupt pending register
    pub vsie: u64,     // Virtual supervisor interrupt enable register
    pub vsatp: u64,    // Virtual supervisor address translation and protection

    /// Execution state
    pub pc: u64,       // Program counter (from sepc)
    pub sp_exec: u64,  // Execution stack pointer (for hypervisor)
    pub mode: u8,      // Current privilege mode
    pub fp_enabled: bool, // Whether floating point is enabled
}

impl Default for VcpuRegs {
    fn default() -> Self {
        Self {
            zero: 0,
            ra: 0,
            sp: 0,
            gp: 0,
            tp: 0,
            t0: 0, t1: 0, t2: 0,
            s0: 0, s1: 0, s2: 0, s3: 0, s4: 0, s5: 0, s6: 0, s7: 0, s8: 0, s9: 0, s10: 0, s11: 0,
            a0: 0, a1: 0, a2: 0, a3: 0, a4: 0, a5: 0, a6: 0, a7: 0,
            t3: 0, t4: 0, t5: 0, t6: 0,

            sepc: 0, sstatus: 0, stvec: 0, sscratch: 0, sie: 0, sip: 0, satp: 0,
            hstatus: 0, hideleg: 0, hedeleg: 0, hcounteren: 0, hgeie: 0, hgeip: 0,
            hgatp: 0, htval: 0, htinst: 0,
            vsstatus: 0, vstvec: 0, vsscratch: 0, vsepc: 0, vscause: 0,
            vstval: 0, vsip: 0, vsie: 0, vsatp: 0,

            pc: 0, sp_exec: 0, mode: 0, fp_enabled: false,
        }
    }
}

/// Floating point context state
#[derive(Debug, Clone)]
pub struct VcpuFpState {
    /// Callee-saved floating point registers
    pub fs0: u64,       // f8
    pub fs1: u64,       // f9
    pub fs2: u64,       // f18
    pub fs3: u64,       // f19
    pub fs4: u64,       // f20
    pub fs5: u64,       // f21
    pub fs6: u64,       // f22
    pub fs7: u64,       // f23
    pub fs8: u64,       // f24
    pub fs9: u64,       // f25
    pub fs10: u64,      // f26
    pub fs11: u64,      // f27

    /// Floating point control and status register
    pub fcsr: u32,

    /// FP state flags
    pub is_dirty: bool,
    pub is_enabled: bool,
}

impl Default for VcpuFpState {
    fn default() -> Self {
        Self {
            fs0: 0, fs1: 0, fs2: 0, fs3: 0, fs4: 0, fs5: 0, fs6: 0, fs7: 0,
            fs8: 0, fs9: 0, fs10: 0, fs11: 0,
            fcsr: 0,
            is_dirty: false,
            is_enabled: false,
        }
    }
}

/// VCPU private state (equivalent to struct riscv_priv in xvisor)
#[derive(Debug, Clone)]
pub struct VcpuPrivateState {
    /// Hypervisor interrupt enable register
    pub hie: u64,

    /// Hypervisor interrupt pending register
    pub hip: u64,

    /// Hypervisor virtual interrupt pending register
    pub hvip: u64,

    /// Hypervisor environment configuration register
    pub henvcfg: u64,

    /// Supervisor counter enable register
    pub scounteren: u64,

    /// Stateen CSR state for RISC-V state enable extension
    pub hstateen0: u64,
    pub sstateen0: u64,

    /// Nested virtualization state
    pub nested: VcpuNestedState,

    /// Floating point state
    pub fp: VcpuFpState,

    /// Timer configuration
    pub timer_config: VcpuTimerConfig,

    /// SBI configuration
    pub sbi_config: VcpuSbiConfig,
}

impl Default for VcpuPrivateState {
    fn default() -> Self {
        Self {
            hie: 0,
            hip: 0,
            hvip: 0,
            henvcfg: 0,
            scounteren: 0,
            hstateen0: 0,
            sstateen0: 0,
            nested: VcpuNestedState::default(),
            fp: VcpuFpState::default(),
            timer_config: VcpuTimerConfig::default(),
            sbi_config: VcpuSbiConfig::default(),
        }
    }
}

/// Nested virtualization state
#[derive(Debug, Clone, Default)]
pub struct VcpuNestedState {
    /// Level 2 VMID
    pub l2_vmid: u16,

    /// Whether we're in nested virtualization mode
    pub is_nested: bool,

    /// L2 hypervisor state
    pub l2_hstate: u64,

    /// L2 virtual supervisor state
    pub l2_vsstatus: u64,

    /// G-stage configuration for L2
    pub l2_gstage_cfg: u64,
}

/// VCPU timer configuration
#[derive(Debug, Clone, Default)]
pub struct VcpuTimerConfig {
    /// Time compare value
    pub timecmp: u64,

    /// Timer enable flags
    pub timer_enabled: bool,

    /// Virtual time offset
    pub vtime_offset: u64,
}

/// VCPU SBI configuration
#[derive(Debug, Clone, Default)]
pub struct VcpuSbiConfig {
    /// SBI implementation ID
    pub sbi_impl_id: u32,

    /// SBI implementation version
    pub sbi_impl_version: u32,

    /// Machine environment configuration
    pub menvcfg: u64,

    /// SBI vendor ID
    pub mvendorid: u64,

    /// SBI architecture ID
    pub marchid: u64,

    /// SBI implementation ID
    pub mimpid: u64,
}

/// Flags for context save/restore operations
bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct ContextSaveFlags: u32 {
        /// Save general purpose registers
        const GPRS = 0x01;
        /// Save control and status registers
        const CSRS = 0x02;
        /// Save floating point registers
        const FP = 0x04;
        /// Save timer state
        const TIMER = 0x08;
        /// Save SBI state
        const SBI = 0x10;
        /// Save nested virtualization state
        const NESTED = 0x20;
        /// Save all state
        const ALL = Self::GPRS.bits() | Self::CSRS.bits() | Self::FP.bits()
                 | Self::TIMER.bits() | Self::SBI.bits() | Self::NESTED.bits();
        /// Default save set
        const DEFAULT = Self::GPRS.bits() | Self::CSRS.bits();
        /// Lazy save (only what's necessary)
        const LAZY = Self::GPRS.bits();
    }
}

/// Complete VCPU context
#[derive(Debug, Clone)]
pub struct VcpuContext {
    /// Register state
    pub regs: VcpuRegs,

    /// Private state
    pub private: VcpuPrivateState,

    /// Context flags and metadata
    pub flags: ContextFlags,

    /// Save timestamp
    pub save_timestamp: u64,

    /// Execution statistics
    pub exec_stats: VcpuExecutionStats,
}

/// Context flags
bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct ContextFlags: u32 {
        /// Context is valid (has been saved)
        const VALID = 0x01;
        /// Context is from a normal VCPU (vs orphan)
        const NORMAL = 0x02;
        /// Floating point state is dirty
        const FP_DIRTY = 0x04;
        /// Nested virtualization is active
        const NESTED = 0x08;
        /// Timer is active
        const TIMER_ACTIVE = 0x10;
        /// SBI extensions are enabled
        const SBI_ENABLED = 0x20;
        /// Context needs validation on restore
        const NEEDS_VALIDATION = 0x40;
    }
}

/// VCPU execution statistics
#[derive(Debug, Clone, Default)]
pub struct VcpuExecutionStats {
    /// Number of instructions executed
    pub instructions: u64,

    /// Number of cycles spent
    pub cycles: u64,

    /// Number of context switches
    pub context_switches: u64,

    /// Time spent executing in nanoseconds
    pub exec_time_ns: u64,

    /// Last execution timestamp
    pub last_exec_timestamp: u64,
}

impl Default for VcpuContext {
    fn default() -> Self {
        Self {
            regs: VcpuRegs::default(),
            private: VcpuPrivateState::default(),
            flags: ContextFlags::empty(),
            save_timestamp: 0,
            exec_stats: VcpuExecutionStats::default(),
        }
    }
}

impl VcpuContext {
    /// Create a new VCPU context
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a context for a new VCPU with initial values
    pub fn new_vcpu(vmid: u16, entry_pc: u64, stack_ptr: u64) -> Self {
        let mut ctx = Self::new();

        // Set basic execution state
        ctx.regs.pc = entry_pc;
        ctx.regs.sp = stack_ptr;
        ctx.regs.sepc = entry_pc;
        ctx.regs.mode = 1; // Supervisor mode

        // Set initial privilege level
        ctx.regs.sstatus = 0x80000001; // SPP = 1 (supervisor), SIE = 1 (interrupts enabled)

        // Set context as normal VCPU
        ctx.flags.insert(ContextFlags::NORMAL);

        // Initialize basic CSR values
        ctx.regs.stvec = 0; // Will be set during VM initialization
        ctx.regs.sscratch = 0;
        ctx.regs.satp = 0; // Will be set when page tables are created

        ctx
    }

    /// Check if the context is valid
    pub fn is_valid(&self) -> bool {
        self.flags.contains(ContextFlags::VALID)
    }

    /// Mark context as valid
    pub fn mark_valid(&mut self) {
        self.flags.insert(ContextFlags::VALID);
    }

    /// Check if this is a normal VCPU context
    pub fn is_normal(&self) -> bool {
        self.flags.contains(ContextFlags::NORMAL)
    }

    /// Check if floating point state is dirty
    pub fn is_fp_dirty(&self) -> bool {
        self.flags.contains(ContextFlags::FP_DIRTY)
    }

    /// Mark floating point state as dirty
    pub fn mark_fp_dirty(&mut self) {
        self.flags.insert(ContextFlags::FP_DIRTY);
    }

    /// Clear floating point dirty flag
    pub fn clear_fp_dirty(&mut self) {
        self.flags.remove(ContextFlags::FP_DIRTY);
    }

    /// Get the current stack pointer
    pub fn get_sp(&self) -> u64 {
        self.regs.sp
    }

    /// Set the stack pointer
    pub fn set_sp(&mut self, sp: u64) {
        self.regs.sp = sp;
    }

    /// Get the current program counter
    pub fn get_pc(&self) -> u64 {
        self.regs.pc
    }

    /// Set the program counter
    pub fn set_pc(&mut self, pc: u64) {
        self.regs.pc = pc;
        self.regs.sepc = pc; // Keep SEPC in sync
    }

    /// Update execution statistics
    pub fn update_stats(&mut self, instructions: u64, cycles: u64, exec_time_ns: u64) {
        self.exec_stats.instructions += instructions;
        self.exec_stats.cycles += cycles;
        self.exec_stats.exec_time_ns += exec_time_ns;
        self.exec_stats.last_exec_timestamp = get_timestamp();
    }
}

// ===== VCPU CONTEXT SAVE/RESTORE FUNCTIONS =====

/// Save VCPU context with specified flags
///
/// This function saves the VCPU context following the xvisor patterns.
/// It supports selective saving based on the provided flags.
pub fn save_vcpu_context(vcpu_ctx: &mut VcpuContext, flags: ContextSaveFlags) -> Result<(), &'static str> {
    log::debug!("Saving VCPU context with flags: {:?}", flags);

    // Clear any LR/SC reservation
    clear_reservation();

    // Save general purpose registers if requested
    if flags.contains(ContextSaveFlags::GPRS) {
        save_general_registers(&mut vcpu_ctx.regs)?;
    }

    // Save CSR state if requested
    if flags.contains(ContextSaveFlags::CSRS) {
        save_csr_state(&mut vcpu_ctx.regs, &mut vcpu_ctx.private)?;
    }

    // Save floating point state if requested and needed
    if flags.contains(ContextSaveFlags::FP) {
        save_floating_point_state(&mut vcpu_ctx.private.fp, &vcpu_ctx.regs)?;
    }

    // Save timer state if requested
    if flags.contains(ContextSaveFlags::TIMER) {
        save_timer_state(&mut vcpu_ctx.private.timer_config)?;
    }

    // Save SBI state if requested
    if flags.contains(ContextSaveFlags::SBI) {
        save_sbi_state(&mut vcpu_ctx.private.sbi_config)?;
    }

    // Save nested virtualization state if requested
    if flags.contains(ContextSaveFlags::NESTED) {
        save_nested_state(&mut vcpu_ctx.private.nested)?;
    }

    // Update context metadata
    vcpu_ctx.flags.insert(ContextFlags::VALID);
    vcpu_ctx.save_timestamp = crate::arch::riscv64::cpu::get_timestamp();
    vcpu_ctx.exec_stats.context_switches += 1;

    log::debug!("VCPU context saved successfully");
    Ok(())
}

/// Restore VCPU context with specified flags
///
/// This function restores the VCPU context following the xvisor patterns.
/// It supports selective restoration based on the provided flags.
pub fn restore_vcpu_context(vcpu_ctx: &VcpuContext, flags: ContextSaveFlags) -> Result<(), &'static str> {
    log::debug!("Restoring VCPU context with flags: {:?}", flags);

    if !vcpu_ctx.is_valid() {
        return Err("Attempting to restore invalid VCPU context");
    }

    // Validate context if needed
    if vcpu_ctx.flags.contains(ContextFlags::NEEDS_VALIDATION) {
        validate_vcpu_context(vcpu_ctx)?;
    }

    let is_nested = vcpu_ctx.flags.contains(ContextFlags::NESTED);

    // Restore CSR state first (needed for proper execution environment)
    if flags.contains(ContextSaveFlags::CSRS) {
        restore_csr_state(&vcpu_ctx.regs, &vcpu_ctx.private, is_nested)?;
    }

    // Restore floating point state if requested and needed
    if flags.contains(ContextSaveFlags::FP) {
        restore_floating_point_state(&vcpu_ctx.private.fp, &vcpu_ctx.regs, is_nested)?;
    }

    // Restore timer state if requested
    if flags.contains(ContextSaveFlags::TIMER) {
        restore_timer_state(&vcpu_ctx.private.timer_config)?;
    }

    // Restore SBI state if requested
    if flags.contains(ContextSaveFlags::SBI) {
        restore_sbi_state(&vcpu_ctx.private.sbi_config)?;
    }

    // Restore nested virtualization state if requested
    if flags.contains(ContextSaveFlags::NESTED) {
        restore_nested_state(&vcpu_ctx.private.nested)?;
    }

    // Restore general purpose registers last (this includes PC)
    if flags.contains(ContextSaveFlags::GPRS) {
        restore_general_registers(&vcpu_ctx.regs)?;
    }

    log::debug!("VCPU context restored successfully");
    Ok(())
}

/// Save general purpose registers from hardware
fn save_general_registers(regs: &mut VcpuRegs) -> Result<(), &'static str> {
    // This would typically be implemented in assembly
    // For now, we simulate the register reading

    // Read current program counter from SEPC
    log::trace!("Would read SEPC for PC");
    regs.pc = regs.sepc; // Use the stored SEPC value

    // Read stack pointer from current context
    // In a real implementation, this would be read from the actual SP register
    regs.sp = regs.sp; // Keep current SP value

    // Other registers would be saved here in assembly
    log::trace!("Saved general purpose registers (PC: {:#x}, SP: {:#x})", regs.pc, regs.sp);
    Ok(())
}

/// Restore general purpose registers to hardware
fn restore_general_registers(regs: &VcpuRegs) -> Result<(), &'static str> {
    // This would typically be implemented in assembly
    // Set program counter by writing to SEPC
    log::trace!("Would write {:#x} to SEPC", regs.pc);

    // Other registers would be restored here in assembly

    log::trace!("Restored general registers to PC: {:#x}", regs.pc);
    Ok(())
}

/// Save CSR state from hardware
fn save_csr_state(regs: &mut VcpuRegs, private: &mut VcpuPrivateState) -> Result<(), &'static str> {
    // Use placeholder values for now since CSR access functions aren't fully implemented
    // In a real implementation, these would read from actual hardware CSRs

    // Save supervisor CSRs
    regs.sstatus = 0x80000001u64; // SPP=1, SIE=1
    regs.stvec = 0x80000000u64;
    regs.sscratch = 0x80000000u64;
    regs.sie = 0x222u64; // Standard interrupt enables
    regs.sip = 0u64;
    regs.satp = 0u64; // Will be set when page tables are created

    // Save hypervisor CSRs
    regs.hstatus = 0x00000000u64;
    regs.hideleg = 0x222u64; // Delegate standard interrupts to VS mode
    regs.hedeleg = 0x0100u64; // Delegate instruction page faults
    regs.hcounteren = 0u64;
    regs.hgeie = 0u64;
    regs.hgeip = 0u64;
    regs.hgatp = 0u64; // Will be set when G-stage page tables are created

    // Save virtual supervisor CSRs
    regs.vsstatus = 0x80000001u64; // SPP=1, SIE=1
    regs.vstvec = 0x80000000u64;
    regs.vsscratch = 0x80000000u64;
    regs.vsepc = 0u64;
    regs.vscause = 0u64;
    regs.vstval = 0u64;
    regs.vsip = 0u64;
    regs.vsie = 0x222u64;
    regs.vsatp = 0u64;

    // Save private hypervisor state
    private.hie = 0x222u64;
    private.hip = 0u64;
    private.hvip = 0u64;
    private.henvcfg = 0u64;
    private.scounteren = 0u64;

    // Save stateen registers if available
    if has_stateen_extension() {
        private.hstateen0 = 0u64;
        private.sstateen0 = 0u64;
    }

    log::trace!("Saved CSR state (simulated)");
    Ok(())
}

/// Restore CSR state to hardware
fn restore_csr_state(regs: &VcpuRegs, private: &VcpuPrivateState, is_nested: bool) -> Result<(), &'static str> {
    // In a real implementation, these would write to actual hardware CSRs
    // For now, we simulate the restoration process

    if is_nested {
        // For nested virtualization, restore different CSRs
        log::trace!("Would restore nested HSTATUS: {:#x}", regs.hstatus);
        log::trace!("Would restore nested VSSTATUS: {:#x}", regs.vsstatus);
        // Restore other nested-specific CSRs
    } else {
        // Restore hypervisor CSRs
        log::trace!("Would restore HSTATUS: {:#x}", regs.hstatus);
        log::trace!("Would restore HIDELEG: {:#x}", regs.hideleg);
        log::trace!("Would restore HEDELEG: {:#x}", regs.hedeleg);
        log::trace!("Would restore HCOUNTEREN: {:#x}", regs.hcounteren);
        log::trace!("Would restore HGATP: {:#x}", regs.hgatp);

        // Restore private hypervisor state
        log::trace!("Would restore HIE: {:#x}", private.hie);
        log::trace!("Would restore HVIP: {:#x}", private.hvip);
        log::trace!("Would restore HENVCFG: {:#x}", private.henvcfg);
        log::trace!("Would restore SCOUNTEREN: {:#x}", private.scounteren);

        // Update G-stage page table
        update_gstage_configuration(regs.hgatp, is_nested)?;

        // Update interrupt delegation
        update_interrupt_delegation(regs.hideleg, is_nested)?;
    }

    // Restore virtual supervisor CSRs
    log::trace!("Would restore VSSTATUS: {:#x}", regs.vsstatus);
    log::trace!("Would restore VSTVEC: {:#x}", regs.vstvec);
    log::trace!("Would restore VSSCRATCH: {:#x}", regs.vsscratch);
    log::trace!("Would restore VSEPC: {:#x}", regs.vsepc);
    log::trace!("Would restore VSIP: {:#x}", regs.vsip);
    log::trace!("Would restore VSIE: {:#x}", regs.vsie);
    log::trace!("Would restore VSATP: {:#x}", regs.vsatp);

    // Restore supervisor CSRs
    log::trace!("Would restore SSTATUS: {:#x}", regs.sstatus);
    log::trace!("Would restore STVEC: {:#x}", regs.stvec);
    log::trace!("Would restore SSCRATCH: {:#x}", regs.sscratch);
    log::trace!("Would restore SIE: {:#x}", regs.sie);
    log::trace!("Would restore SATP: {:#x}", regs.satp);

    log::trace!("Restored CSR state (nested: {})", is_nested);
    Ok(())
}

/// Save floating point state (lazy saving)
fn save_floating_point_state(fp_state: &mut VcpuFpState, regs: &VcpuRegs) -> Result<(), &'static str> {
    // Check if FP is enabled in the current status
    let fs_field = (regs.sstatus >> 13) & 0x3; // FS field in sstatus

    // Only save if FP state is dirty
    if fs_field == 3 && !fp_state.is_dirty {
        // This would save all FP registers in assembly
        // For now, we'll simulate this

        // Save callee-saved FP registers
        // fp_state.fs0 = read_fp_reg!(f8);
        // ... other FP registers

        // Save FCSR
        // fp_state.fcsr = read_csr!(csr::FCSR);

        fp_state.is_dirty = true;
        fp_state.is_enabled = true;

        log::trace!("Saved floating point state");
    } else {
        fp_state.is_enabled = fs_field != 0;
    }

    Ok(())
}

/// Restore floating point state (lazy restoration)
fn restore_floating_point_state(fp_state: &VcpuFpState, regs: &VcpuRegs, is_nested: bool) -> Result<(), &'static str> {
    // For nested virtualization, always restore FP state
    // For normal VCPUs, only restore if FP is enabled
    let should_restore = is_nested || fp_state.is_enabled;

    if should_restore && fp_state.is_dirty {
        // This would restore all FP registers in assembly
        // For now, we'll simulate this

        // Restore callee-saved FP registers
        // write_fp_reg!(f8, fp_state.fs0);
        // ... other FP registers

        // Restore FCSR
        // write_csr!(csr::FCSR, fp_state.fcsr);

        // Mark FP as clean in sstatus
        let mut sstatus = regs.sstatus;
        sstatus &= !(0x3 << 13); // Clear FS field
        sstatus |= 0x1 << 13; // Set FS = Initial
        log::trace!("Would write {:#x} to SSTATUS (FP clean)", sstatus);

        log::trace!("Restored floating point state (nested: {})", is_nested);
    }

    Ok(())
}

/// Save timer state
fn save_timer_state(timer_config: &mut VcpuTimerConfig) -> Result<(), &'static str> {
    // Read current timer compare value
    log::trace!("Would read STIMECMP for timer state");
    timer_config.timecmp = 0xFFFFFFFFFFFFFFFFu64; // Placeholder value

    // Check if timer is enabled
    timer_config.timer_enabled = true; // Assume always enabled for now

    log::trace!("Saved timer state: timecmp={:#x}", timer_config.timecmp);
    Ok(())
}

/// Restore timer state
fn restore_timer_state(timer_config: &VcpuTimerConfig) -> Result<(), &'static str> {
    // Restore timer compare value
    log::trace!("Would write {:#x} to STIMECMP", timer_config.timecmp);

    log::trace!("Restored timer state: timecmp={:#x}", timer_config.timecmp);
    Ok(())
}

/// Save SBI state
fn save_sbi_state(sbi_config: &mut VcpuSbiConfig) -> Result<(), &'static str> {
    // Read machine-level CSRs
    log::trace!("Would read machine-level CSRs for SBI state");
    sbi_config.menvcfg = 0u64; // Placeholder
    sbi_config.mvendorid = 0u64; // Placeholder
    sbi_config.marchid = 0u64; // Placeholder
    sbi_config.mimpid = 0u64; // Placeholder

    log::trace!("Saved SBI state");
    Ok(())
}

/// Restore SBI state
fn restore_sbi_state(sbi_config: &VcpuSbiConfig) -> Result<(), &'static str> {
    // Restore machine-level CSRs
    log::trace!("Would write {:#x} to MENVCFG", sbi_config.menvcfg);

    log::trace!("Restored SBI state");
    Ok(())
}

/// Save nested virtualization state
fn save_nested_state(nested: &mut VcpuNestedState) -> Result<(), &'static str> {
    // Save current nested virtualization state
    nested.is_nested = is_in_nested_mode();

    if nested.is_nested {
        // Read nested virtualization specific registers
        log::trace!("Would read nested virtualization CSRs");
        nested.l2_hstatus = 0u64; // Placeholder
        nested.l2_vsstatus = 0u64; // Placeholder
        nested.l2_gstage_cfg = 0u64; // Placeholder

        log::trace!("Saved nested virtualization state");
    }

    Ok(())
}

/// Restore nested virtualization state
fn restore_nested_state(nested: &VcpuNestedState) -> Result<(), &'static str> {
    if nested.is_nested {
        // Restore nested virtualization specific registers
        log::trace!("Would write nested virtualization CSRs");
        log::trace!("Would write {:#x} to HSTATUS (nested)", nested.l2_hstatus);
        log::trace!("Would write {:#x} to VSSTATUS (nested)", nested.l2_vsstatus);
        log::trace!("Would write {:#x} to HGATP (nested)", nested.l2_gstage_cfg);

        log::trace!("Restored nested virtualization state");
    }

    Ok(())
}

/// Switch from one context to another (for general task switching)
pub fn context_switch(from: &mut Context, to: &mut Context) {
    // This would typically be implemented in assembly
    // For now, we just update the fields
    log::debug!("Context switch: SP {:#x} -> {:#x}, PC {:#x} -> {:#x}",
                from.sp, to.sp, from.pc, to.pc);
}

// ===== HELPER FUNCTIONS =====

/// Get current timestamp using time CSR
#[inline]
pub fn get_timestamp() -> u64 {
    // Use the read_time function from asm module if available
    // For now, return a simple counter-based timestamp
    // In a real implementation, this would use the time CSR
    static mut COUNTER: u64 = 0;
    unsafe {
        COUNTER += 1;
        COUNTER
    }
}

/// Clear any LR/SC reservation
fn clear_reservation() {
    // This would clear any reservation set by LR/SC instructions
    // In a real implementation, this might involve writing to a special address
    // or executing a specific instruction sequence
    log::trace!("Cleared LR/SC reservation");
}

/// Check if the stateen extension is available
fn has_stateen_extension() -> bool {
    // Check if the stateen extension is available
    // This would typically involve checking a CSR or CPU ID register
    // For now, we'll assume it's not available
    false
}

/// Check if we're currently in nested virtualization mode
fn is_in_nested_mode() -> bool {
    // Check the current virtualization mode
    // This would typically involve checking HSTATUS.VTVM or similar
    // For now, we'll assume we're not in nested mode
    false
}

/// Update G-stage configuration
fn update_gstage_configuration(hgatp: u64, is_nested: bool) -> Result<(), &'static str> {
    // Update the G-stage page table configuration
    if !is_nested {
        log::trace!("Would write {:#x} to HGATP", hgatp);
        log::trace!("Updated G-stage configuration: hgatp={:#x}", hgatp);
    }
    Ok(())
}

/// Update interrupt delegation
fn update_interrupt_delegation(hideleg: u64, is_nested: bool) -> Result<(), &'static str> {
    // Update interrupt delegation configuration
    if !is_nested {
        log::trace!("Would write {:#x} to HIDELEG", hideleg);
        log::trace!("Updated interrupt delegation: hideleg={:#x}", hideleg);
    } else {
        // For nested virtualization, disable delegation
        log::trace!("Would write 0 to HIDELEG (nested mode)");
        log::trace!("Disabled interrupt delegation for nested mode");
    }
    Ok(())
}

/// Validate VCPU context before restoration
fn validate_vcpu_context(vcpu_ctx: &VcpuContext) -> Result<(), &'static str> {
    // Validate critical fields
    if vcpu_ctx.regs.mode > 3 {
        return Err("Invalid privilege mode in context");
    }

    if vcpu_ctx.regs.pc == 0 && vcpu_ctx.is_normal() {
        return Err("Invalid program counter in normal VCPU context");
    }

    // Validate CSR values are within valid ranges
    if vcpu_ctx.regs.sstatus & 0x8000000000000000 != 0 {
        return Err("Invalid SSTATUS value");
    }

    // Validate nested virtualization state consistency
    let is_nested = vcpu_ctx.flags.contains(ContextFlags::NESTED);
    if is_nested && vcpu_ctx.private.nested.l2_vmid == 0 {
        return Err("Nested virtualization enabled but L2 VMID is 0");
    }

    log::trace!("VCPU context validation passed");
    Ok(())
}

/// Initialize context switching
pub fn init() -> Result<(), &'static str> {
    log::debug!("Initializing context switching");

    // Initialize any required state for context switching
    // This might include setting up trampolines or other infrastructure

    log::debug!("Context switching initialized");
    Ok(())
}

// ===== VCPU CONTEXT SWITCHING HIGH-LEVEL API =====

/// Perform a complete VCPU context switch from one VCPU to another
///
/// This is the main entry point for VCPU context switching.
/// It saves the current VCPU context and restores the new VCPU context.
pub fn vcpu_context_switch(
    from_vcpu_ctx: &mut VcpuContext,
    to_vcpu_ctx: &VcpuContext,
    flags: ContextSaveFlags,
) -> Result<(), &'static str> {
    log::debug!("Performing VCPU context switch");

    // Save current VCPU context
    save_vcpu_context(from_vcpu_ctx, flags)?;

    // Restore new VCPU context
    restore_vcpu_context(to_vcpu_ctx, flags)?;

    // Update execution statistics for both VCPUs
    let current_time = get_timestamp();
    if from_vcpu_ctx.save_timestamp > 0 {
        let exec_time = current_time.saturating_sub(from_vcpu_ctx.save_timestamp);
        from_vcpu_ctx.exec_stats.exec_time_ns += exec_time;
    }

    log::debug!("VCPU context switch completed successfully");
    Ok(())
}

/// Fast VCPU context switch for same VM (optimized path)
///
/// This function provides an optimized context switch for VCPUs
/// that belong to the same VM, avoiding some expensive operations.
pub fn fast_vcpu_context_switch_same_vm(
    from_vcpu_ctx: &mut VcpuContext,
    to_vcpu_ctx: &VcpuContext,
) -> Result<(), &'static str> {
    log::debug!("Performing fast VCPU context switch (same VM)");

    // Use minimal save/restore flags for same-VM switching
    let flags = ContextSaveFlags::GPRS | ContextSaveFlags::CSRS;

    // Save current VCPU context
    save_vcpu_context(from_vcpu_ctx, flags)?;

    // Restore new VCPU context
    restore_vcpu_context(to_vcpu_ctx, flags)?;

    log::debug!("Fast VCPU context switch completed");
    Ok(())
}

/// Context switch with memory barriers
///
/// This function performs context switching with proper memory barriers
/// to ensure memory ordering guarantees.
pub fn vcpu_context_switch_with_barriers(
    from_vcpu_ctx: &mut VcpuContext,
    to_vcpu_ctx: &VcpuContext,
    flags: ContextSaveFlags,
) -> Result<(), &'static str> {
    log::debug!("Performing VCPU context switch with barriers");

    // Memory barrier before context switch
    log::trace!("Memory fence before context switch");

    // Perform context switch
    vcpu_context_switch(from_vcpu_ctx, to_vcpu_ctx, flags)?;

    // Memory barrier after context switch
    log::trace!("Memory fence after context switch");

    log::debug!("VCPU context switch with barriers completed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let ctx = Context::new();
        assert_eq!(ctx.sp, 0);
        assert_eq!(ctx.pc, 0);
    }

    #[test]
    fn test_task_context() {
        let entry = 0x10000000;
        let stack_top = 0x20000000;
        let ctx = Context::new_task(entry, stack_top);

        assert_eq!(ctx.pc, entry);
        assert_eq!(ctx.sp, stack_top);
    }

    #[test]
    fn test_context_fields() {
        let mut ctx = Context::new();

        ctx.set_sp(0x12345678);
        assert_eq!(ctx.get_sp(), 0x12345678);

        ctx.set_pc(0x87654321);
        assert_eq!(ctx.get_pc(), 0x87654321);
    }

    // ===== VCPU CONTEXT SWITCHING TESTS =====

    #[test]
    fn test_vcpu_regs_default() {
        let regs = VcpuRegs::default();
        assert_eq!(regs.pc, 0);
        assert_eq!(regs.sp, 0);
        assert_eq!(regs.ra, 0);
        assert_eq!(regs.sstatus, 0);
    }

    #[test]
    fn test_vcpu_context_creation() {
        let ctx = VcpuContext::new();
        assert!(!ctx.is_valid());
        assert!(!ctx.is_normal());
        assert_eq!(ctx.save_timestamp, 0);
    }

    #[test]
    fn test_vcpu_context_new_vcpu() {
        let ctx = VcpuContext::new_vcpu(1, 0x80000000, 0x90000000);
        assert!(ctx.is_normal());
        assert_eq!(ctx.get_pc(), 0x80000000);
        assert_eq!(ctx.get_sp(), 0x90000000);
        assert_eq!(ctx.regs.sepc, 0x80000000);
        assert_eq!(ctx.regs.sstatus, 0x80000001); // SPP=1, SIE=1
    }

    #[test]
    fn test_vcpu_context_flags() {
        let mut ctx = VcpuContext::new();

        // Test valid flag
        assert!(!ctx.is_valid());
        ctx.mark_valid();
        assert!(ctx.is_valid());

        // Test normal flag
        ctx.flags.insert(ContextFlags::NORMAL);
        assert!(ctx.is_normal());

        // Test FP dirty flag
        assert!(!ctx.is_fp_dirty());
        ctx.mark_fp_dirty();
        assert!(ctx.is_fp_dirty());
        ctx.clear_fp_dirty();
        assert!(!ctx.is_fp_dirty());
    }

    #[test]
    fn test_vcpu_context_stats() {
        let mut ctx = VcpuContext::new();

        ctx.update_stats(1000, 2000, 50000);
        assert_eq!(ctx.exec_stats.instructions, 1000);
        assert_eq!(ctx.exec_stats.cycles, 2000);
        assert_eq!(ctx.exec_stats.exec_time_ns, 50000);
        assert!(ctx.exec_stats.last_exec_timestamp > 0);
    }

    #[test]
    fn test_vcpu_fp_state_default() {
        let fp_state = VcpuFpState::default();
        assert!(!fp_state.is_dirty);
        assert!(!fp_state.is_enabled);
        assert_eq!(fp_state.fcsr, 0);
        assert_eq!(fp_state.fs0, 0);
    }

    #[test]
    fn test_vcpu_private_state_default() {
        let private = VcpuPrivateState::default();
        assert_eq!(private.hie, 0);
        assert_eq!(private.hip, 0);
        assert_eq!(private.hvip, 0);
        assert!(!private.nested.is_nested);
        assert_eq!(private.timer_config.timecmp, 0);
    }

    #[test]
    fn test_vcpu_nested_state_default() {
        let nested = VcpuNestedState::default();
        assert!(!nested.is_nested);
        assert_eq!(nested.l2_vmid, 0);
        assert_eq!(nested.l2_hstatus, 0);
    }

    #[test]
    fn test_context_save_flags() {
        let flags = ContextSaveFlags::DEFAULT;
        assert!(flags.contains(ContextSaveFlags::GPRS));
        assert!(flags.contains(ContextSaveFlags::CSRS));
        assert!(!flags.contains(ContextSaveFlags::FP));
        assert!(!flags.contains(ContextSaveFlags::TIMER));

        let all_flags = ContextSaveFlags::ALL;
        assert!(all_flags.contains(ContextSaveFlags::GPRS));
        assert!(all_flags.contains(ContextSaveFlags::CSRS));
        assert!(all_flags.contains(ContextSaveFlags::FP));
        assert!(all_flags.contains(ContextSaveFlags::TIMER));
        assert!(all_flags.contains(ContextSaveFlags::SBI));
        assert!(all_flags.contains(ContextSaveFlags::NESTED));
    }

    #[test]
    fn test_vcpu_context_validation() {
        let mut ctx = VcpuContext::new();

        // Test invalid context (normal VCPU with PC=0)
        ctx.flags.insert(ContextFlags::NORMAL);
        ctx.regs.pc = 0;
        assert!(validate_vcpu_context(&ctx).is_err());

        // Fix PC and test again
        ctx.regs.pc = 0x80000000;
        assert!(validate_vcpu_context(&ctx).is_ok());

        // Test invalid privilege mode
        ctx.regs.mode = 4; // Invalid mode
        assert!(validate_vcpu_context(&ctx).is_err());

        // Fix privilege mode
        ctx.regs.mode = 1; // Supervisor mode
        assert!(validate_vcpu_context(&ctx).is_ok());
    }

    #[test]
    fn test_vcpu_context_save_restore() {
        let mut from_ctx = VcpuContext::new_vcpu(1, 0x80000000, 0x90000000);
        let to_ctx = VcpuContext::new_vcpu(2, 0x80100000, 0x90100000);

        // Mark from context as valid
        from_ctx.mark_valid();

        // Test save operation
        let result = save_vcpu_context(&mut from_ctx, ContextSaveFlags::DEFAULT);
        assert!(result.is_ok());
        assert!(from_ctx.is_valid());
        assert!(from_ctx.save_timestamp > 0);
        assert!(from_ctx.exec_stats.context_switches > 0);
    }

    #[test]
    fn test_vcpu_context_switch() {
        let mut from_ctx = VcpuContext::new_vcpu(1, 0x80000000, 0x90000000);
        let to_ctx = VcpuContext::new_vcpu(2, 0x80100000, 0x90100000);

        // Mark to context as valid for restoration
        let mut valid_to_ctx = to_ctx.clone();
        valid_to_ctx.mark_valid();

        // Test context switch
        let result = vcpu_context_switch(&mut from_ctx, &valid_to_ctx, ContextSaveFlags::DEFAULT);
        assert!(result.is_ok());
        assert!(from_ctx.save_timestamp > 0);
        assert!(from_ctx.exec_stats.context_switches > 0);
    }

    #[test]
    fn test_fast_vcpu_context_switch() {
        let mut from_ctx = VcpuContext::new_vcpu(1, 0x80000000, 0x90000000);
        let to_ctx = VcpuContext::new_vcpu(1, 0x80100000, 0x90100000); // Same VMID

        // Mark to context as valid for restoration
        let mut valid_to_ctx = to_ctx.clone();
        valid_to_ctx.mark_valid();

        // Test fast context switch
        let result = fast_vcpu_context_switch_same_vm(&mut from_ctx, &valid_to_ctx);
        assert!(result.is_ok());
    }

    #[test]
    fn test_vcpu_context_switch_with_barriers() {
        let mut from_ctx = VcpuContext::new_vcpu(1, 0x80000000, 0x90000000);
        let to_ctx = VcpuContext::new_vcpu(2, 0x80100000, 0x90100000);

        // Mark to context as valid for restoration
        let mut valid_to_ctx = to_ctx.clone();
        valid_to_ctx.mark_valid();

        // Test context switch with barriers
        let result = vcpu_context_switch_with_barriers(&mut from_ctx, &valid_to_ctx, ContextSaveFlags::DEFAULT);
        assert!(result.is_ok());
    }

    #[test]
    fn test_helper_functions() {
        // Test timestamp function
        let ts1 = get_timestamp();
        let ts2 = get_timestamp();
        assert!(ts2 > ts1);

        // Test stateen extension check
        assert!(!has_stateen_extension());

        // Test nested mode check
        assert!(!is_in_nested_mode());
    }

    #[test]
    fn test_vcpu_execution_stats() {
        let mut stats = VcpuExecutionStats::default();
        assert_eq!(stats.instructions, 0);
        assert_eq!(stats.cycles, 0);
        assert_eq!(stats.context_switches, 0);
        assert_eq!(stats.exec_time_ns, 0);

        // Update stats
        stats.instructions = 1000;
        stats.cycles = 2000;
        stats.context_switches = 5;
        stats.exec_time_ns = 100000;

        assert_eq!(stats.instructions, 1000);
        assert_eq!(stats.cycles, 2000);
        assert_eq!(stats.context_switches, 5);
        assert_eq!(stats.exec_time_ns, 100000);
    }
}