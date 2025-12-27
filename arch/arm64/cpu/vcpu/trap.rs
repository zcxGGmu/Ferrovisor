//! ARM64 VCPU Trap Handling
//!
//! This module provides trap handling for virtual CPUs.
//!
//! ## Trap Overview
//!
//! Traps occur when a Guest OS tries to execute privileged operations
//! that must be handled by the hypervisor:
//!
//! - **System Register Access**: Reading/writing EL1 system registers
//! - **Protected Instructions**: Cache maintenance, barriers, etc.
//! - **Exception Level Changes**: Attempting to enter higher EL
//! - **Memory Access**: Stage-2 translation faults
//! - **Instruction Emulation**: Instructions that need emulation
//!
//! ## Trap Types
//!
//! | Trap Type | Description | ESR_EL2.EC |
//!------------|-------------|-------------|
//! | Trapped MRS/MRS | System register access | 0b000000 |
//! | Trapped I/O | I/O instructions | 0b000111 |
//! | Trapped FP/SIMD | VFP/NEON access | 0b000111 |
//! | Trapped Execution | CP15/CP14 | 0b000011 |
//! | Trapped SVE | SVE access | 0b001011 |
//! | Trapped ERET | ERET to EL2 | 0b010110 |
//! | Trapped SMC | SMC call | 0b011111 |
//!
//! ## References
//! - [ARM DDI 0487] ARMv8-A Architecture Reference Manual
//! - [Xvisor cpu_vcpu_helper.c](/home/zcxggmu/workspace/hello-projs/posp/xvisor/arch/arm/cpu/arm64/cpu_vcpu_helper.c)

use crate::arch::arm64::cpu::vcpu::context::{ExtendedVcpuContext, SavedGprs};
use crate::arch::arm64::cpu::regs::ExceptionLevel;
use crate::arch::arm64::mmu::fault::{Stage2Fault, FaultInfo};

/// Trap reason
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapReason {
    /// Trapped MRS/MRS (system register access)
    SysRegAccess,
    /// Trapped I/O instruction
    IoInstruction,
    /// Trapped FP/SIMD instruction
    FpSimdTrap,
    /// Trapped cache operation
    CacheOperation,
    /// Trapped barrier instruction
    Barrier,
    /// Trapped WFI/WFE
    WfiWfe,
    /// Trapped SVE access
    SveTrap,
    /// Trapped ERET (attempted ERET to EL2)
    EretTrap,
    /// Trapped SMC call
    SmcCall,
    /// Stage-2 translation fault
    Stage2Fault(Stage2Fault),
    /// Alignment fault
    AlignmentFault,
    /// Permission fault
    PermissionFault,
    /// Unknown trap
    Unknown,
}

impl TrapReason {
    /// Create from ESR_EL2 exception class
    pub fn from_esr(esr: u64) -> Self {
        let ec = (esr >> 26) & 0x3F;

        match ec {
            0b000000 => Self::SysRegAccess,
            0b000001 => Self::Unknown,          // Uncategorized
            0b000010 => Self::Unknown,          // Uncategorized (WO)
            0b000011 => Self::SysRegAccess,     // CP15/MRT/MRRC (ARM32)
            0b000111 => Self::FpSimdTrap,       // FP/SIMD
            0b001001 => Self::SysRegAccess,     // EL3 (ARM32)
            0b001011 => Self::SveTrap,          // SVE
            0b001101 => Self::SysRegAccess,     // EL2 (ARM32)
            0b010000 => Self::Unknown,          // Instruction abort from lower EL
            0b010001 => Self::Unknown,          // Instruction alignment fault
            0b010010 => Self::Unknown,          // Data abort from lower EL
            0b010011 => Self::AlignmentFault,  // Data alignment fault
            0b010100 => Self::PermissionFault,  // Data fault from EL0
            0b010101 => Self::PermissionFault,  // Data fault from EL1
            0b010110 => Self::EretTrap,         // Trapped ERET
            0b011000 => Self::Unknown,          // Tracer exception
            0b011111 => Self::SmcCall,          // SMC call
            _ => Self::Unknown,
        }
    }

    /// Check if trap is recoverable
    pub fn is_recoverable(&self) -> bool {
        matches!(self,
            Self::SysRegAccess | Self::IoInstruction |
            Self::FpSimdTrap | Self::CacheOperation |
            Self::Barrier | Self::WfiWfe | Self::SveTrap |
            Self::Stage2Fault(_)
        )
    }

    /// Get trap name
    pub fn name(&self) -> &'static str {
        match self {
            Self::SysRegAccess => "System Register Access",
            Self::IoInstruction => "I/O Instruction",
            Self::FpSimdTrap => "FP/SIMD Trap",
            Self::CacheOperation => "Cache Operation",
            Self::Barrier => "Barrier",
            Self::WfiWfe => "WFI/WFE",
            Self::SveTrap => "SVE Trap",
            Self::EretTrap => "ERET Trap",
            Self::SmcCall => "SMC Call",
            Self::Stage2Fault(f) => f.name(),
            Self::AlignmentFault => "Alignment Fault",
            Self::PermissionFault => "Permission Fault",
            Self::Unknown => "Unknown",
        }
    }
}

/// Trap information
#[derive(Debug, Clone)]
pub struct TrapInfo {
    /// Trap reason
    pub reason: TrapReason,
    /// Exception Syndrome Register (ESR_EL2)
    pub esr: u64,
    /// Fault Address Register (FAR_EL2)
    pub far: u64,
    /// Instruction Syndrome (ISS)
    pub iss: u32,
    /// Instruction length (0 = 2 bytes, 1 = 4 bytes, 2 = 0-byte)
    pub il: u32,
    /// Instruction encoding (for trapped instructions)
    pub instr: u32,
    /// VCPU ID
    pub vcpu_id: u32,
    /// Program counter at trap time
    pub pc: u64,
    /// Processor state at trap time
    pub spsr: u64,
}

impl TrapInfo {
    /// Create new trap info
    pub fn new(vcpu_id: u32, esr: u64, far: u64, pc: u64, spsr: u64) -> Self {
        let iss = (esr & 0x1FFFFFF) as u32;
        let il = ((esr >> 25) & 0x3) as u32;
        let reason = TrapReason::from_esr(esr);

        Self {
            reason,
            esr,
            far,
            iss,
            il,
            instr: 0,
            vcpu_id,
            pc,
            spsr,
        }
    }

    /// Get exception class
    pub fn exception_class(&self) -> u32 {
        ((self.esr >> 26) & 0x3F) as u32
    }

    /// Check if this is a Stage-2 fault
    pub fn is_stage2_fault(&self) -> bool {
        matches!(self.reason, TrapReason::Stage2Fault(_))
    }

    /// Decode as Stage-2 fault
    pub fn as_stage2_fault(&self) -> Option<Stage2Fault> {
        match self.reason {
            TrapReason::Stage2Fault(f) => Some(f),
            _ => None,
        }
    }

    /// Create from Stage-2 fault info
    pub fn from_stage2_fault(
        vcpu_id: u32,
        fault: Stage2Fault,
        fault_info: &FaultInfo,
        pc: u64,
        spsr: u64,
    ) -> Self {
        let esr = fault_info.esr;
        let far = fault_info.fault.unwrap_or(0);

        Self {
            reason: TrapReason::Stage2Fault(fault),
            esr,
            far,
            iss: (esr & 0x1FFFFFF) as u32,
            il: ((esr >> 25) & 0x3) as u32,
            instr: 0,
            vcpu_id,
            pc,
            spsr,
        }
    }
}

/// Trap resolution result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapResolution {
    /// Trap was handled, resume guest
    Resume,
    /// Trap was handled, inject exception to guest
    InjectException,
    /// Trap cannot be handled, halt VCPU
    Halt,
    /// Trap should be emulated
    Emulate,
    /// Trap requires callback to higher level
    Callback,
}

/// Trap handler trait
///
/// Implement this trait to handle specific trap types.
pub trait TrapHandler {
    /// Handle system register access trap
    fn handle_sysreg_access(
        &mut self,
        trap: &TrapInfo,
        op: u32,    // 0 = read, 1 = write
        reg: u32,   // System register encoding
        value: u64, // Value (for write) or result (for read)
    ) -> Result<TrapResolution, &'static str>;

    /// Handle FP/SIMD trap
    fn handle_fpsimd_trap(
        &mut self,
        trap: &TrapInfo,
    ) -> Result<TrapResolution, &'static str>;

    /// Handle WFI/WFE trap
    fn handle_wfi_wfe(
        &mut self,
        trap: &TrapInfo,
    ) -> Result<TrapResolution, &'static str>;

    /// Handle Stage-2 fault
    fn handle_stage2_fault(
        &mut self,
        trap: &TrapInfo,
        fault: Stage2Fault,
    ) -> Result<TrapResolution, &'static str>;

    /// Handle SMC call
    fn handle_smc_call(
        &mut self,
        trap: &TrapInfo,
        function_id: u32,
    ) -> Result<TrapResolution, &'static str>;
}

/// Default trap handler implementation
pub struct DefaultTrapHandler {
    /// Enable WFI emulation (just return instead of halt)
    pub emulate_wfi: bool,
    /// Enable cache operation emulation
    pub emulate_cache: bool,
}

impl Default for DefaultTrapHandler {
    fn default() -> Self {
        Self {
            emulate_wfi: true,
            emulate_cache: true,
        }
    }
}

impl TrapHandler for DefaultTrapHandler {
    fn handle_sysreg_access(
        &mut self,
        trap: &TrapInfo,
        op: u32,
        reg: u32,
        value: u64,
    ) -> Result<TrapResolution, &'static str> {
        log::debug!("Trap: SysReg access op={} reg={:#x} value={:#x}",
                   op, reg, value);

        // Decode system register from ISS
        // ISS format for SysReg access:
        // [23:20] = Op0, [19:16] = Op1, [15:12] = CRn, [11:8] = CRm, [7:5] = Op2

        let op0 = (trap.iss >> 20) & 0xF;
        let op1 = (trap.iss >> 16) & 0xF;
        let crn = (trap.iss >> 12) & 0xF;
        let crm = (trap.iss >> 8) & 0xF;
        let op2 = (trap.iss >> 5) & 0x7;

        log::debug!("  Decoded: Op0={} Op1={} CRn={} CRm={} Op2={}",
                   op0, op1, crn, crm, op2);

        // For now, just return 0 for reads and ignore writes
        // A real implementation would emulate the register
        if op == 0 {
            // Read - return 0
            Ok(TrapResolution::Resume)
        } else {
            // Write - ignore
            Ok(TrapResolution::Resume)
        }
    }

    fn handle_fpsimd_trap(
        &mut self,
        trap: &TrapInfo,
    ) -> Result<TrapResolution, &'static str> {
        log::debug!("Trap: FP/SIMD trap at PC={:#x}", trap.pc);

        // Enable FP/SIMD for the guest
        // Set CPTR_EL2.TFP = 0 to untrap
        unsafe {
            let mut cptr: u64;
            core::arch::asm!("mrs {}, cptr_el2", out(reg) cptr);
            cptr &= !(1u64 << 10); // Clear TFP
            core::arch::asm!("msr cptr_el2, {}", in(reg) cptr);
        }

        log::info!("Enabled FP/SIMD for guest");
        Ok(TrapResolution::Resume)
    }

    fn handle_wfi_wfe(
        &mut self,
        trap: &TrapInfo,
    ) -> Result<TrapResolution, &'static str> {
        log::debug!("Trap: WFI/WFE at PC={:#x}", trap.pc);

        if self.emulate_wfi {
            // Emulate WFI as NOP
            log::debug!("  Emulated WFI as NOP");
            Ok(TrapResolution::Resume)
        } else {
            // Let WFI execute (will wait for interrupt)
            Ok(TrapResolution::Emulate)
        }
    }

    fn handle_stage2_fault(
        &mut self,
        trap: &TrapInfo,
        fault: Stage2Fault,
    ) -> Result<TrapResolution, &'static str> {
        log::warn!("Trap: Stage-2 fault: {:?} IPA={:#x}",
                  fault, trap.far);

        // Check if fault is recoverable
        if !fault.is_recoverable() {
            log::error!("  Unrecoverable fault, halting VCPU");
            return Ok(TrapResolution::Halt);
        }

        // Inject data/abort exception to guest
        Ok(TrapResolution::InjectException)
    }

    fn handle_smc_call(
        &mut self,
        trap: &TrapInfo,
        function_id: u32,
    ) -> Result<TrapResolution, &'static str> {
        log::debug!("Trap: SMC call function_id={:#x}", function_id);

        // For now, just return undefined exception
        // A real implementation would handle PSCI calls
        log::warn!("  SMC not fully implemented, injecting undef");
        Ok(TrapResolution::InjectException)
    }
}

/// Handle trap from guest
///
/// # Parameters
/// - `context`: VCPU extended context
/// - `trap`: Trap information
/// - `handler`: Trap handler implementation
///
/// # Returns
/// Trap resolution result
pub fn handle_trap(
    context: &ExtendedVcpuContext,
    trap: &TrapInfo,
    handler: &mut dyn TrapHandler,
) -> Result<TrapResolution, &'static str> {
    log::info!("VCPU {}: Handling trap: {} at PC={:#x}",
               trap.vcpu_id, trap.reason.name(), trap.pc);

    match trap.reason {
        TrapReason::SysRegAccess => {
            let op = (trap.iss >> 0) & 0x1; // Bit 0: 0=read, 1=write
            let reg = (trap.iss >> 5) & 0xFFFF; // Register encoding
            handler.handle_sysreg_access(trap, op, reg, 0)
        }
        TrapReason::FpSimdTrap => {
            handler.handle_fpsimd_trap(trap)
        }
        TrapReason::WfiWfe => {
            handler.handle_wfi_wfe(trap)
        }
        TrapReason::Stage2Fault(fault) => {
            handler.handle_stage2_fault(trap, fault)
        }
        TrapReason::SmcCall => {
            let function_id = (trap.iss >> 0) & 0xFFFF;
            handler.handle_smc_call(trap, function_id)
        }
        TrapReason::EretTrap => {
            log::warn!("Trap: ERET to EL2 is not allowed");
            Ok(TrapResolution::InjectException)
        }
        _ => {
            log::warn!("Trap: Unhandled trap: {:?}", trap.reason);
            Ok(TrapResolution::InjectException)
        }
    }
}

/// Decode trapped instruction
///
/// # Parameters
/// - `trap`: Trap information
///
/// # Returns
/// Decoded instruction or None if decode failed
pub fn decode_trapped_instruction(trap: &TrapInfo) -> Option<TrappedInstruction> {
    // TODO: Decode instruction from memory at PC
    // For now, return None
    None
}

/// Trapped instruction information
#[derive(Debug, Clone)]
pub struct TrappedInstruction {
    /// Instruction encoding
    pub encoding: u32,
    /// Instruction length (2 or 4 bytes)
    pub length: u32,
    /// Instruction mnemonic
    pub mnemonic: &'static str,
    /// Instruction operands
    pub operands: &'static str,
}

/// Create exception info for injection
///
/// # Parameters
/// - `trap`: Trap information
///
/// # Returns
/// Exception info for injection
pub fn create_exception_info(trap: &TrapInfo) -> ExceptionInfo {
    match trap.reason {
        TrapReason::SysRegAccess => ExceptionInfo {
            exception_type: 0, // Undefined
            esr_el2: trap.esr,
            far_el2: trap.far,
        },
        TrapReason::SmcCall => ExceptionInfo {
            exception_type: 0, // Undefined
            esr_el2: trap.esr,
            far_el2: trap.far,
        },
        TrapReason::EretTrap => ExceptionInfo {
            exception_type: 0, // Undefined
            esr_el2: trap.esr,
            far_el2: trap.far,
        },
        TrapReason::Stage2Fault(_) => {
            // Create data abort or prefetch abort
            ExceptionInfo {
                exception_type: 4, // Data abort from lower EL
                esr_el2: trap.esr,
                far_el2: trap.far,
            }
        }
        _ => ExceptionInfo {
            exception_type: 0, // Undefined
            esr_el2: trap.esr,
            far_el2: trap.far,
        },
    }
}

/// Exception information for injection
#[derive(Debug, Clone)]
pub struct ExceptionInfo {
    /// Exception type
    pub exception_type: u32,
    /// ESR_EL2 value
    pub esr_el2: u64,
    /// FAR_EL2 value
    pub far_el2: u64,
}

/// Update VCPU PC after trap
///
/// # Parameters
/// - `context`: VCPU context
/// - `advance_bytes`: Number of bytes to advance PC
pub fn advance_pc(context: &mut ExtendedVcpuContext, advance_bytes: u64) {
    let pc = context.sysregs.elr_el1 as u64;
    context.sysregs.elr_el1 = (pc + advance_bytes) as u64;
    log::debug!("  Advanced PC: {:#x} -> {:#x}", pc, context.sysregs.elr_el1);
}

/// Check if trap is from AArch32 guest
///
/// # Parameters
/// - `trap`: Trap information
///
/// # Returns
/// true if trap is from AArch32 guest
pub fn is_aarch32_trap(trap: &TrapInfo) -> bool {
    // Check if PSTATE indicates AArch32
    // PSTATE.nRW = 1 means AArch32
    (trap.spsr & (1 << 5)) != 0
}

/// Get AArch32 mode from PSTATE
///
/// # Parameters
/// - `spsr`: Saved PSTATE
///
/// # Returns
/// AArch32 mode (CPSR mode bits)
pub fn get_aarch32_mode(spsr: u64) -> u32 {
    ((spsr >> 0) & 0x1F) as u32
}

/// AArch32 processor modes
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch32Mode {
    User = 0b10000,
    Fiq = 0b10001,
    Irq = 0b10010,
    Supervisor = 0b10011,
    Abort = 0b10111,
    Undefined = 0b11011,
    System = 0b11111,
}

/// Check if executing in AArch32
pub fn is_aarch32() -> bool {
    // Check PSTATE.nRW bit
    let spsr: u64;
    unsafe {
        core::arch::asm!("mrs {}, spsr_el2", out(reg) spsr);
    }
    (spsr & (1 << 5)) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trap_reason() {
        let reason = TrapReason::from_esr(0x00000000); // SysReg access
        assert_eq!(reason, TrapReason::SysRegAccess);
        assert!(reason.is_recoverable());

        let reason2 = TrapReason::from_esr(0x20000000); // Instr abort
        assert_eq!(reason2, TrapReason::Unknown);
    }

    #[test]
    fn test_trap_info() {
        let trap = TrapInfo::new(0, 0x00000000, 0x0, 0x40000000, 0x00000500);
        assert_eq!(trap.vcpu_id, 0);
        assert_eq!(trap.pc, 0x40000000);
        assert_eq!(trap.spsr, 0x500);
    }

    #[test]
    fn test_trap_resolution() {
        assert_eq!(TrapResolution::Resume, TrapResolution::Resume);
        assert_eq!(TrapResolution::Halt, TrapResolution::Halt);
    }

    #[test]
    fn test_default_trap_handler() {
        let mut handler = DefaultTrapHandler::default();
        assert!(handler.emulate_wfi);
        assert!(handler.emulate_cache);
    }

    #[test]
    fn test_exception_info() {
        let trap = TrapInfo::new(0, 0x00000000, 0x0, 0x40000000, 0x00000500);
        let exc = create_exception_info(&trap);
        assert_eq!(exc.exception_type, 0); // Undefined
    }

    #[test]
    fn test_aarch32_mode() {
        assert_eq!(Aarch32Mode::User as u32, 0b10000);
        assert_eq!(Aarch32Mode::Supervisor as u32, 0b10011);
    }
}
