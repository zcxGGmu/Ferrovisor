//! ARM64 Exception Handlers
//!
//! This module contains the C-compatible exception handlers called from assembly.
//!
//! ## Exception Types
//!
//! | Type | Name              | Description                         |
//! |------|-------------------|-------------------------------------|
//! | 0    | EL2_SYNC_SP0      | Synchronous from EL1t               |
//! | 1    | EL2_IRQ_SP0       | IRQ from EL1t                       |
//! | 2    | EL2_FIQ_SP0       | FIQ from EL1t                       |
//! | 3    | EL2_SERROR_SP0    | SError from EL1t                    |
//! | 4    | EL2_SYNC_SPX      | Synchronous from EL1h               |
//! | 5    | EL2_IRQ_SPX       | IRQ from EL1h                       |
//! | 6    | EL2_FIQ_SPX       | FIQ from EL1h                       |
//! | 7    | EL2_SERROR_SPX    | SError from EL1h                    |
//! | 8    | GUEST_SYNC_A64    | Synchronous from 64-bit EL0 (guest) |
//! | 9    | GUEST_IRQ_A64     | IRQ from 64-bit EL0 (guest)         |
//! | 10   | GUEST_FIQ_A64     | FIQ from 64-bit EL0 (guest)         |
//! | 11   | GUEST_SERROR_A64  | SError from 64-bit EL0 (guest)      |
//! | 12   | GUEST_SYNC_A32    | Synchronous from 32-bit EL0 (guest) |
//! | 13   | GUEST_IRQ_A32     | IRQ from 32-bit EL0 (guest)         |
//! | 14   | GUEST_FIQ_A32     | FIQ from 32-bit EL0 (guest)         |
//! | 15   | GUEST_SERROR_A32  | SError from 32-bit EL0 (guest)      |
//!
//! ## Exception Context
//!
//! The assembly code pushes all registers onto the stack before calling
//! these handlers. The context structure represents the saved register state.

use core::ffi::c_void;

use crate::arch::arm64::cpu::vcpu::{
    ExtendedVcpuContext, TrapInfo, TrapHandler, DefaultTrapHandler, handle_trap,
};

/// Exception type identifier
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceptionType {
    /// Synchronous exception from EL1t (SP0)
    El2SyncSp0 = 0,
    /// IRQ from EL1t (SP0)
    El2IrqSp0 = 1,
    /// FIQ from EL1t (SP0)
    El2FiqSp0 = 2,
    /// SError from EL1t (SP0)
    El2SerrorSp0 = 3,
    /// Synchronous exception from EL1h (SPx)
    El2SyncSpx = 4,
    /// IRQ from EL1h (SPx)
    El2IrqSpx = 5,
    /// FIQ from EL1h (SPx)
    El2FiqSpx = 6,
    /// SError from EL1h (SPx)
    El2SerrorSpx = 7,
    /// Synchronous from 64-bit EL0 (guest)
    GuestSyncA64 = 8,
    /// IRQ from 64-bit EL0 (guest)
    GuestIrqA64 = 9,
    /// FIQ from 64-bit EL0 (guest)
    GuestFiqA64 = 10,
    /// SError from 64-bit EL0 (guest)
    GuestSerrorA64 = 11,
    /// Synchronous from 32-bit EL0 (guest)
    GuestSyncA32 = 12,
    /// IRQ from 32-bit EL0 (guest)
    GuestIrqA32 = 13,
    /// FIQ from 32-bit EL0 (guest)
    GuestFiqA32 = 14,
    /// SError from 32-bit EL0 (guest)
    GuestSerrorA32 = 15,
}

impl ExceptionType {
    /// Create from raw value
    pub fn from_raw(raw: u32) -> Self {
        match raw {
            0 => Self::El2SyncSp0,
            1 => Self::El2IrqSp0,
            2 => Self::El2FiqSp0,
            3 => Self::El2SerrorSp0,
            4 => Self::El2SyncSpx,
            5 => Self::El2IrqSpx,
            6 => Self::El2FiqSpx,
            7 => Self::El2SerrorSpx,
            8 => Self::GuestSyncA64,
            9 => Self::GuestIrqA64,
            10 => Self::GuestFiqA64,
            11 => Self::GuestSerrorA64,
            12 => Self::GuestSyncA32,
            13 => Self::GuestIrqA32,
            14 => Self::GuestFiqA32,
            15 => Self::GuestSerrorA32,
            _ => Self::El2SyncSp0,
        }
    }

    /// Check if this is a guest exception
    pub fn is_guest(&self) -> bool {
        matches!(self,
            Self::GuestSyncA64 | Self::GuestIrqA64 | Self::GuestFiqA64 | Self::GuestSerrorA64 |
            Self::GuestSyncA32 | Self::GuestIrqA32 | Self::GuestFiqA32 | Self::GuestSerrorA32
        )
    }

    /// Check if this is a synchronous exception
    pub fn is_sync(&self) -> bool {
        matches!(self,
            Self::El2SyncSp0 | Self::El2SyncSpx |
            Self::GuestSyncA64 | Self::GuestSyncA32
        )
    }

    /// Check if this is an IRQ
    pub fn is_irq(&self) -> bool {
        matches!(self,
            Self::El2IrqSp0 | Self::El2IrqSpx |
            Self::GuestIrqA64 | Self::GuestIrqA32
        )
    }

    /// Check if this is an SError
    pub fn is_serror(&self) -> bool {
        matches!(self,
            Self::El2SerrorSp0 | Self::El2SerrorSpx |
            Self::GuestSerrorA64 | Self::GuestSerrorA32
        )
    }

    /// Get exception name
    pub fn name(&self) -> &'static str {
        match self {
            Self::El2SyncSp0 => "EL2_SYNC_SP0",
            Self::El2IrqSp0 => "EL2_IRQ_SP0",
            Self::El2FiqSp0 => "EL2_FIQ_SP0",
            Self::El2SerrorSp0 => "EL2_SERROR_SP0",
            Self::El2SyncSpx => "EL2_SYNC_SPX",
            Self::El2IrqSpx => "EL2_IRQ_SPX",
            Self::El2FiqSpx => "EL2_FIQ_SPX",
            Self::El2SerrorSpx => "EL2_SERROR_SPX",
            Self::GuestSyncA64 => "GUEST_SYNC_A64",
            Self::GuestIrqA64 => "GUEST_IRQ_A64",
            Self::GuestFiqA64 => "GUEST_FIQ_A64",
            Self::GuestSerrorA64 => "GUEST_SERROR_A64",
            Self::GuestSyncA32 => "GUEST_SYNC_A32",
            Self::GuestIrqA32 => "GUEST_IRQ_A32",
            Self::GuestFiqA32 => "GUEST_FIQ_A32",
            Self::GuestSerrorA32 => "GUEST_SERROR_A32",
        }
    }
}

/// Exception context (saved registers from assembly)
///
/// This structure must match the layout used in vectors.S for push_regs/pop_regs.
/// Total size: 272 bytes = 34 × 8 bytes
#[repr(C)]
#[derive(Debug, Clone)]
pub struct ExceptionContext {
    /// General-purpose registers x0-x28
    pub x: [u64; 29],
    /// Stack pointer at exception time
    pub sp: u64,
    /// Exception link register (ELR_EL2)
    pub elr: u64,
    /// Saved processor state (SPSR_EL2)
    pub spsr: u64,
}

impl ExceptionContext {
    /// Create new exception context
    pub fn new() -> Self {
        Self {
            x: [0; 29],
            sp: 0,
            elr: 0,
            spsr: 0,
        }
    }

    /// Get GPR value
    pub fn gpr(&self, index: usize) -> u64 {
        self.x.get(index).copied().unwrap_or(0)
    }

    /// Set GPR value
    pub fn set_gpr(&mut self, index: usize, value: u64) {
        if let Some(reg) = self.x.get_mut(index) {
            *reg = value;
        }
    }

    /// Get exception return address
    pub fn elr(&self) -> u64 {
        self.elr
    }

    /// Set exception return address
    pub fn set_elr(&mut self, addr: u64) {
        self.elr = addr;
    }

    /// Get saved PSTATE
    pub fn spsr(&self) -> u64 {
        self.spsr
    }

    /// Set saved PSTATE
    pub fn set_spsr(&mut self, psr: u64) {
        self.spsr = psr;
    }

    /// Get SP at exception time
    pub fn sp(&self) -> u64 {
        self.sp
    }

    /// Set SP
    pub fn set_sp(&mut self, sp: u64) {
        self.sp = sp;
    }
}

impl Default for ExceptionContext {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// C-compatible Exception Handlers
// ============================================================================

/// Default exception handler callback type
pub type ExceptionHandler = fn(&mut ExceptionContext, ExceptionType);

/// Global exception handler (can be set by the hypervisor)
static mut EXCEPTION_HANDLER: Option<ExceptionHandler> = None;

/// VCPU trap handler (used for guest exceptions)
static mut VCPU_TRAP_HANDLER: Option<*mut dyn TrapHandler> = None;

/// Set the exception handler
pub fn set_exception_handler(handler: ExceptionHandler) {
    unsafe {
        EXCEPTION_HANDLER = Some(handler);
    }
}

/// Set the VCPU trap handler for guest exceptions
///
/// # Safety
/// The handler pointer must be valid for the lifetime of the hypervisor
pub unsafe fn set_vcpu_trap_handler(handler: *mut dyn TrapHandler) {
    VCPU_TRAP_HANDLER = Some(handler);
}

/// Clear the VCPU trap handler
pub unsafe fn clear_vcpu_trap_handler() {
    VCPU_TRAP_HANDLER = None;
}

/// Get current VCPU trap handler
pub unsafe fn get_vcpu_trap_handler() -> Option<*mut dyn TrapHandler> {
    VCPU_TRAP_HANDLER
}

/// Handle EL2 synchronous exception from SP0
#[no_mangle]
pub extern "C" fn rust_el2_sync_sp0(ctx: *mut ExceptionContext, exc_type: u32) {
    handle_exception(ctx, exc_type);
}

/// Handle EL2 IRQ from SP0
#[no_mangle]
pub extern "C" fn rust_el2_irq_sp0(ctx: *mut ExceptionContext, exc_type: u32) {
    handle_exception(ctx, exc_type);
}

/// Handle EL2 FIQ from SP0
#[no_mangle]
pub extern "C" fn rust_el2_fiq_sp0(ctx: *mut ExceptionContext, exc_type: u32) {
    handle_exception(ctx, exc_type);
}

/// Handle EL2 SError from SP0
#[no_mangle]
pub extern "C" fn rust_el2_serror_sp0(ctx: *mut ExceptionContext, exc_type: u32) {
    handle_exception(ctx, exc_type);
}

/// Handle EL2 synchronous exception from SPx
#[no_mangle]
pub extern "C" fn rust_el2_sync_spx(ctx: *mut ExceptionContext, exc_type: u32) {
    handle_exception(ctx, exc_type);
}

/// Handle EL2 IRQ from SPx
#[no_mangle]
pub extern "C" fn rust_el2_irq_spx(ctx: *mut ExceptionContext, exc_type: u32) {
    handle_exception(ctx, exc_type);
}

/// Handle EL2 FIQ from SPx
#[no_mangle]
pub extern "C" fn rust_el2_fiq_spx(ctx: *mut ExceptionContext, exc_type: u32) {
    handle_exception(ctx, exc_type);
}

/// Handle EL2 SError from SPx
#[no_mangle]
pub extern "C" fn rust_el2_serror_spx(ctx: *mut ExceptionContext, exc_type: u32) {
    handle_exception(ctx, exc_type);
}

/// Handle guest synchronous exception from AArch64
#[no_mangle]
pub extern "C" fn rust_guest_sync_a64(ctx: *mut ExceptionContext, exc_type: u32) {
    let exc_type = ExceptionType::from_raw(exc_type);

    // Try trap handler first for guest sync exceptions
    unsafe {
        if let Some(trap_handler) = VCPU_TRAP_HANDLER {
            if let Ok(()) = handle_guest_trap(ctx, exc_type, &mut *trap_handler) {
                return;
            }
        }
    }

    // Fall back to default exception handler
    handle_exception(ctx, exc_type);
}

/// Handle guest IRQ from AArch64
#[no_mangle]
pub extern "C" fn rust_guest_irq_a64(ctx: *mut ExceptionContext, exc_type: u32) {
    handle_exception(ctx, exc_type);
}

/// Handle guest FIQ from AArch64
#[no_mangle]
pub extern "C" fn rust_guest_fiq_a64(ctx: *mut ExceptionContext, exc_type: u32) {
    handle_exception(ctx, exc_type);
}

/// Handle guest SError from AArch64
#[no_mangle]
pub extern "C" fn rust_guest_serror_a64(ctx: *mut ExceptionContext, exc_type: u32) {
    handle_exception(ctx, exc_type);
}

/// Handle guest synchronous exception from AArch32
#[no_mangle]
pub extern "C" fn rust_guest_sync_a32(ctx: *mut ExceptionContext, exc_type: u32) {
    let exc_type = ExceptionType::from_raw(exc_type);

    // Try trap handler first for guest sync exceptions
    unsafe {
        if let Some(trap_handler) = VCPU_TRAP_HANDLER {
            if let Ok(()) = handle_guest_trap(ctx, exc_type, &mut *trap_handler) {
                return;
            }
        }
    }

    // Fall back to default exception handler
    handle_exception(ctx, exc_type);
}

/// Handle guest IRQ from AArch32
#[no_mangle]
pub extern "C" fn rust_guest_irq_a32(ctx: *mut ExceptionContext, exc_type: u32) {
    handle_exception(ctx, exc_type);
}

/// Handle guest FIQ from AArch32
#[no_mangle]
pub extern "C" fn rust_guest_fiq_a32(ctx: *mut ExceptionContext, exc_type: u32) {
    handle_exception(ctx, exc_type);
}

/// Handle guest SError from AArch32
#[no_mangle]
pub extern "C" fn rust_guest_serror_a32(ctx: *mut ExceptionContext, exc_type: u32) {
    handle_exception(ctx, exc_type);
}

// ============================================================================
// Guest Trap Handling
// ============================================================================

/// Handle guest trap using VCPU trap handler
///
/// This function bridges the exception handler with the VCPU trap handler.
/// It reads ESR_EL2 and FAR_EL2 to create trap information and delegates
/// to the trap handler.
fn handle_guest_trap(
    ctx: *mut ExceptionContext,
    exc_type: ExceptionType,
    handler: &mut dyn TrapHandler,
) -> Result<(), &'static str> {
    unsafe {
        // Read exception syndrome and fault address
        let esr: u64;
        let far: u64;
        core::arch::asm!(
            "mrs {}, esr_el2",
            out(reg) esr
        );
        core::arch::asm!(
            "mrs {}, far_el2",
            out(reg) far
        );

        let ctx_ref = &*ctx;

        // Create trap info
        let trap = TrapInfo::new(
            0, // vcpu_id (TODO: get from context)
            esr,
            far,
            ctx_ref.elr,
            ctx_ref.spsr,
        );

        // Create extended VCPU context from exception context
        let mut vcpu_ctx = ExtendedVcpuContext::new();
        vcpu_ctx.gprs.x = ctx_ref.x;
        vcpu_ctx.sysregs.elr_el1 = ctx_ref.elr as u64;
        vcpu_ctx.sysregs.spsr_el1 = ctx_ref.spsr as u64;

        // Handle the trap
        let resolution = handle_trap(&vcpu_ctx, &trap, handler)?;

        // Apply resolution
        match resolution {
            crate::arch::arm64::cpu::vcpu::TrapResolution::Resume => {
                // Just return to guest
                log::debug!("Trap handled, resuming guest");
                Ok(())
            }
            crate::arch::arm64::cpu::vcpu::TrapResolution::InjectException => {
                // Exception will be injected to guest
                log::warn!("Trap handled, injecting exception to guest");
                Err("Exception injected")
            }
            crate::arch::arm64::cpu::vcpu::TrapResolution::Halt => {
                log::error!("Trap handler requested VCPU halt");
                Err("VCPU halted")
            }
            crate::arch::arm64::cpu::vcpu::TrapResolution::Emulate => {
                log::debug!("Trap requires emulation");
                // TODO: Implement instruction emulation
                Err("Emulation not implemented")
            }
            crate::arch::arm64::cpu::vcpu::TrapResolution::Callback => {
                log::debug!("Trap requires callback to higher level");
                Err("Callback required")
            }
        }
    }
}

/// Internal exception handler
fn handle_exception(ctx: *mut ExceptionContext, exc_type: u32) {
    let exc_type = ExceptionType::from_raw(exc_type);

    // Log exception
    log::error!("Exception: {}", exc_type.name());
    log::error!("  ELR={:#018x}, SPSR={:#08x}",
        unsafe { (*ctx).elr },
        unsafe { (*ctx).spsr }
    );

    // Call custom handler if set
    unsafe {
        if let Some(handler) = EXCEPTION_HANDLER {
            handler(&mut *ctx, exc_type);
            return;
        }
    }

    // Default handler: dump context and hang
    dump_exception_context(unsafe { &*ctx });
    panic!("Unhandled exception: {}", exc_type.name());
}

/// Dump exception context for debugging
fn dump_exception_context(ctx: &ExceptionContext) {
    log::error!("=== Exception Context ===");
    log::error!("  ELR   = {:#018x}", ctx.elr);
    log::error!("  SPSR  = {:#018x}", ctx.spsr);
    log::error!("  SP    = {:#018x}", ctx.sp);

    // Dump GPRs in groups of 4
    for i in (0..29).step_by(4) {
        log::error!("  x{:02}-x{:02} = {:#018x} {:#018x} {:#018x} {:#018x}",
            i, i + 3,
            ctx.x[i], ctx.x[i + 1],
            ctx.x.get(i + 2).copied().unwrap_or(0),
            ctx.x.get(i + 3).copied().unwrap_or(0)
        );
    }
    log::error!("========================");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exception_type() {
        let exc = ExceptionType::GuestSyncA64;
        assert!(exc.is_guest());
        assert!(exc.is_sync());
        assert!(!exc.is_irq());
        assert_eq!(exc.name(), "GUEST_SYNC_A64");
    }

    #[test]
    fn test_exception_context() {
        let mut ctx = ExceptionContext::new();
        ctx.set_gpr(0, 0x12345678);
        assert_eq!(ctx.gpr(0), 0x12345678);
        ctx.set_elr(0x40000000);
        assert_eq!(ctx.elr(), 0x40000000);
    }
}
