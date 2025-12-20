//! RISC-V Interrupt and Exception Handler
//!
//! This module provides interrupt and exception handling functionality including:
//! - Exception entry and exit
//! - Interrupt dispatching
//! - Context save/restore
//! - Trap handling

use crate::arch::riscv64::*;
use crate::arch::riscv64::cpu::state::PerCpuData;

/// Trap context saved on exception entry
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TrapContext {
    /// General purpose registers
    pub x: [usize; 32],
    /// Floating point registers
    pub f: [u64; 32],
    /// Program counter
    pub pc: usize,
    /// Status register
    pub status: usize,
    /// Cause register
    pub cause: usize,
    /// Trap value register
    pub tval: usize,
    /// FPU control and status register
    pub fcsr: u32,
    /// Current privilege level
    pub privilege: u8,
    /// Reserved for alignment
    _reserved: [u8; 7],
}

impl Default for TrapContext {
    fn default() -> Self {
        Self {
            x: [0; 32],
            f: [0; 32],
            pc: 0,
            status: 0,
            cause: 0,
            tval: 0,
            fcsr: 0,
            privilege: 3, // Machine mode by default
            _reserved: [0; 7],
        }
    }
}

impl TrapContext {
    /// Create a new trap context
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a general purpose register
    pub fn get_gpr(&self, index: usize) -> usize {
        if index < 32 {
            self.x[index]
        } else {
            0
        }
    }

    /// Set a general purpose register
    pub fn set_gpr(&mut self, index: usize, value: usize) {
        if index < 32 {
            self.x[index] = value;
        }
    }

    /// Get the program counter
    pub fn get_pc(&self) -> usize {
        self.pc
    }

    /// Set the program counter
    pub fn set_pc(&mut self, pc: usize) {
        self.pc = pc;
    }

    /// Get the exception cause
    pub fn get_cause(&self) -> usize {
        self.cause
    }

    /// Get the exception code from cause
    pub fn get_exception_code(&self) -> Option<ExceptionCode> {
        if self.is_exception() {
            match self.cause & 0x7FFFFFFF {
                0 => Some(ExceptionCode::InstructionMisaligned),
                1 => Some(ExceptionCode::InstructionAccessFault),
                2 => Some(ExceptionCode::IllegalInstruction),
                3 => Some(ExceptionCode::Breakpoint),
                4 => Some(ExceptionCode::LoadMisaligned),
                5 => Some(ExceptionCode::LoadAccessFault),
                6 => Some(ExceptionCode::StoreMisaligned),
                7 => Some(ExceptionCode::StoreAccessFault),
                8 => Some(ExceptionCode::ECallFromUMode),
                9 => Some(ExceptionCode::ECallFromSMode),
                11 => Some(ExceptionCode::ECallFromMMode),
                12 => Some(ExceptionCode::InstructionPageFault),
                13 => Some(ExceptionCode::LoadPageFault),
                15 => Some(ExceptionCode::StorePageFault),
                _ => None,
            }
        } else {
            None
        }
    }

    /// Get the interrupt cause from cause
    pub fn get_interrupt_cause(&self) -> Option<InterruptCause> {
        if self.is_interrupt() {
            match self.cause & 0x7FFFFFFF {
                1 => Some(InterruptCause::SupervisorSoftware),
                3 => Some(InterruptCause::MachineSoftware),
                5 => Some(InterruptCause::SupervisorTimer),
                7 => Some(InterruptCause::MachineTimer),
                9 => Some(InterruptCause::SupervisorExternal),
                11 => Some(InterruptCause::MachineExternal),
                _ => None,
            }
        } else {
            None
        }
    }

    /// Check if this is an interrupt
    pub fn is_interrupt(&self) -> bool {
        (self.cause & 0x80000000) != 0
    }

    /// Check if this is an exception
    pub fn is_exception(&self) -> bool {
        (self.cause & 0x80000000) == 0
    }

    /// Get the trap value register
    pub fn get_tval(&self) -> usize {
        self.tval
    }

    /// Get current privilege level
    pub fn get_privilege(&self) -> PrivilegeLevel {
        match self.privilege {
            0 => PrivilegeLevel::User,
            1 => PrivilegeLevel::Supervisor,
            3 => PrivilegeLevel::Machine,
            _ => PrivilegeLevel::Reserved,
        }
    }
}

/// Exception handler function type
pub type ExceptionHandler = fn(&mut TrapContext) -> Result<(), &'static str>;

/// Interrupt handler function type
pub type InterruptHandler = fn(&mut TrapContext) -> Result<(), &'static str>;

/// Trap handler table
pub struct TrapHandlerTable {
    /// Exception handlers (indexed by exception code)
    exception_handlers: [Option<ExceptionHandler>; 16],
    /// Interrupt handlers (indexed by interrupt cause)
    interrupt_handlers: [Option<InterruptHandler>; 16],
    /// Default exception handler
    default_exception_handler: Option<ExceptionHandler>,
    /// Default interrupt handler
    default_interrupt_handler: Option<InterruptHandler>,
}

impl TrapHandlerTable {
    /// Create a new trap handler table
    pub fn new() -> Self {
        Self {
            exception_handlers: [None; 16],
            interrupt_handlers: [None; 16],
            default_exception_handler: None,
            default_interrupt_handler: None,
        }
    }

    /// Register an exception handler
    pub fn register_exception_handler(
        &mut self,
        exception_code: ExceptionCode,
        handler: ExceptionHandler,
    ) {
        let index = exception_code as usize;
        if index < self.exception_handlers.len() {
            self.exception_handlers[index] = Some(handler);
        }
    }

    /// Register an interrupt handler
    pub fn register_interrupt_handler(
        &mut self,
        interrupt_cause: InterruptCause,
        handler: InterruptHandler,
    ) {
        let index = (interrupt_cause as usize) >> 1;
        if index < self.interrupt_handlers.len() {
            self.interrupt_handlers[index] = Some(handler);
        }
    }

    /// Set default exception handler
    pub fn set_default_exception_handler(&mut self, handler: ExceptionHandler) {
        self.default_exception_handler = Some(handler);
    }

    /// Set default interrupt handler
    pub fn set_default_interrupt_handler(&mut self, handler: InterruptHandler) {
        self.default_interrupt_handler = Some(handler);
    }

    /// Handle an exception
    pub fn handle_exception(
        &self,
        context: &mut TrapContext,
        exception_code: ExceptionCode,
    ) -> Result<(), &'static str> {
        let index = exception_code as usize;

        if let Some(handler) = self.exception_handlers.get(index).and_then(|h| *h) {
            handler(context)
        } else if let Some(handler) = self.default_exception_handler {
            handler(context)
        } else {
            Err("No exception handler registered")
        }
    }

    /// Handle an interrupt
    pub fn handle_interrupt(
        &self,
        context: &mut TrapContext,
        interrupt_cause: InterruptCause,
    ) -> Result<(), &'static str> {
        let index = (interrupt_cause as usize) >> 1;

        if let Some(handler) = self.interrupt_handlers.get(index).and_then(|h| *h) {
            handler(context)
        } else if let Some(handler) = self.default_interrupt_handler {
            handler(context)
        } else {
            Err("No interrupt handler registered")
        }
    }
}

/// Global trap handler table
static mut TRAP_HANDLERS: TrapHandlerTable = TrapHandlerTable {
    exception_handlers: [None; 16],
    interrupt_handlers: [None; 16],
    default_exception_handler: None,
    default_interrupt_handler: None,
};

/// Initialize trap handling
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing RISC-V trap handling");

    // Register default handlers
    unsafe {
        TRAP_HANDLERS.set_default_exception_handler(default_exception_handler);
        TRAP_HANDLERS.set_default_interrupt_handler(default_interrupt_handler);
    }

    // Set trap vector
    set_trap_vector(trap_entry as usize);

    log::info!("RISC-V trap handling initialized");
    Ok(())
}

/// Set trap vector address
pub fn set_trap_vector(addr: usize) {
    let mode = 1; // Direct mode
    let value = addr | mode;
    crate::arch::riscv64::cpu::csr::MTVEC::write(value);
}

/// Get trap vector address
pub fn get_trap_vector() -> usize {
    let value = crate::arch::riscv64::cpu::csr::MTVEC::read();
    value & !0x3
}

/// Main trap entry point (called from assembly)
pub extern "C" fn trap_entry() {
    // Save trap context
    let mut context = save_trap_context();

    // Handle the trap
    handle_trap(&mut context);

    // Restore trap context
    restore_trap_context(&context);
}

/// Save trap context from registers
fn save_trap_context() -> TrapContext {
    // This would typically be implemented in assembly
    // For now, create a dummy context
    let mut context = TrapContext::new();

    // Read trap cause and value
    context.cause = crate::arch::riscv64::cpu::csr::MCAUSE::read();
    context.tval = crate::arch::riscv64::cpu::csr::MTVAL::read();
    context.status = crate::arch::riscv64::cpu::csr::MSTATUS::read();
    context.pc = crate::arch::riscv64::cpu::csr::MEPC::read();

    // Get current privilege level
    let mstatus = crate::arch::riscv64::cpu::csr::MSTATUS::read();
    context.privilege = ((mstatus >> 11) & 0x3) as u8;

    context
}

/// Restore trap context to registers
fn restore_trap_context(context: &TrapContext) {
    // Restore trap cause and value
    crate::arch::riscv64::cpu::csr::MCAUSE::write(context.cause);
    crate::arch::riscv64::cpu::csr::MTVAL::write(context.tval);
    crate::arch::riscv64::cpu::csr::MSTATUS::write(context.status);
    crate::arch::riscv64::cpu::csr::MEPC::write(context.pc);

    // Restore general purpose registers would be done in assembly
}

/// Handle a trap (exception or interrupt)
fn handle_trap(context: &mut TrapContext) -> Result<(), &'static str> {
    let handlers = unsafe { &TRAP_HANDLERS };

    if context.is_interrupt() {
        // Handle interrupt
        if let Some(interrupt_cause) = context.get_interrupt_cause() {
            log::debug!("Handling interrupt: {:?}", interrupt_cause);
            handlers.handle_interrupt(context, interrupt_cause)?;
        } else {
            log::warn!("Unknown interrupt cause: {:#x}", context.cause);
            return Err("Unknown interrupt cause");
        }
    } else {
        // Handle exception
        if let Some(exception_code) = context.get_exception_code() {
            log::debug!("Handling exception: {:?}", exception_code);
            handlers.handle_exception(context, exception_code)?;
        } else {
            log::warn!("Unknown exception cause: {:#x}", context.cause);
            return Err("Unknown exception cause");
        }
    }

    Ok(())
}

/// Default exception handler
fn default_exception_handler(context: &mut TrapContext) -> Result<(), &'static str> {
    log::error!(
        "Unhandled exception: code={}, pc={:#x}, tval={:#x}",
        context.cause,
        context.pc,
        context.tval
    );

    // For debugging, we might want to halt or panic here
    // For now, just increment PC to skip the offending instruction
    context.pc += 4;

    Ok(())
}

/// Default interrupt handler
fn default_interrupt_handler(context: &mut TrapContext) -> Result<(), &'static str> {
    log::warn!(
        "Unhandled interrupt: cause={}, pc={:#x}",
        context.cause,
        context.pc
    );

    Ok(())
}

/// Register a trap handler
pub fn register_trap_handler(
    exception_code: Option<ExceptionCode>,
    interrupt_cause: Option<InterruptCause>,
    handler: extern "C" fn(&mut TrapContext) -> Result<(), &'static str>,
) {
    unsafe {
        if let Some(code) = exception_code {
            TRAP_HANDLERS.register_exception_handler(code, handler);
        }
        if let Some(cause) = interrupt_cause {
            TRAP_HANDLERS.register_interrupt_handler(cause, handler);
        }
    }
}

/// Enable/disable interrupts
pub mod control {
    /// Enable all interrupts
    pub fn enable_all() {
        let mut mstatus = crate::arch::riscv64::cpu::csr::MSTATUS::read();
        mstatus |= 1 << 3; // MIE bit
        crate::arch::riscv64::cpu::csr::MSTATUS::write(mstatus);
    }

    /// Disable all interrupts
    pub fn disable_all() {
        let mut mstatus = crate::arch::riscv64::cpu::csr::MSTATUS::read();
        mstatus &= !(1 << 3); // MIE bit
        crate::arch::riscv64::cpu::csr::MSTATUS::write(mstatus);
    }

    /// Enable specific interrupt types
    pub fn enable_interrupts(interrupts: crate::arch::riscv64::cpu::csr::Mie) {
        crate::arch::riscv64::cpu::csr::MIE::set(interrupts);
    }

    /// Disable specific interrupt types
    pub fn disable_interrupts(interrupts: crate::arch::riscv64::cpu::csr::Mie) {
        crate::arch::riscv64::cpu::csr::MIE::clear(interrupts);
    }

    /// Check if interrupts are enabled
    pub fn are_enabled() -> bool {
        let mstatus = crate::arch::riscv64::cpu::csr::MSTATUS::read();
        (mstatus & (1 << 3)) != 0
    }

    /// Save and disable interrupts
    pub fn save_and_disable() -> usize {
        let mstatus = crate::arch::riscv64::cpu::csr::MSTATUS::read();
        let interrupts_enabled = (mstatus & (1 << 3)) != 0;

        if interrupts_enabled {
            disable_all();
        }

        interrupts_enabled as usize
    }

    /// Restore interrupt state
    pub fn restore(state: usize) {
        if state != 0 {
            enable_all();
        }
    }
}

/// Critical section guard
pub struct CriticalSection {
    _private: (),
}

impl CriticalSection {
    /// Enter a critical section (disable interrupts)
    pub fn enter() -> Self {
        let state = control::save_and_disable();
        Self { _private: () }
    }
}

impl Drop for CriticalSection {
    fn drop(&mut self) {
        control::restore(1); // Re-enable interrupts
    }
}

/// RAII macro for critical sections
#[macro_export]
macro_rules! critical_section {
    ($body:block) => {{
        let _guard = $crate::arch::riscv64::interrupt::handler::CriticalSection::enter();
        $body
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trap_context() {
        let mut context = TrapContext::new();

        context.set_pc(0x10000000);
        assert_eq!(context.get_pc(), 0x10000000);

        context.set_gpr(1, 0x12345678);
        assert_eq!(context.get_gpr(1), 0x12345678);

        // Test exception detection
        context.cause = 0x00000002; // Illegal instruction
        assert!(!context.is_interrupt());
        assert!(context.is_exception());
        assert_eq!(context.get_exception_code(), Some(ExceptionCode::IllegalInstruction));

        // Test interrupt detection
        context.cause = 0x80000005; // Supervisor timer interrupt
        assert!(context.is_interrupt());
        assert!(!context.is_exception());
        assert_eq!(context.get_interrupt_cause(), Some(InterruptCause::SupervisorTimer));
    }

    #[test]
    fn test_trap_handler_table() {
        let mut table = TrapHandlerTable::new();

        // Register a test exception handler
        extern "C" fn test_handler(_ctx: &mut TrapContext) -> Result<(), &'static str> {
            Ok(())
        }

        table.register_exception_handler(ExceptionCode::IllegalInstruction, test_handler);

        // Create a test context
        let mut context = TrapContext::new();
        context.cause = 0x00000002; // Illegal instruction

        // Handle the exception
        let result = table.handle_exception(&mut context, ExceptionCode::IllegalInstruction);
        assert!(result.is_ok());
    }

    #[test]
    fn test_interrupt_control() {
        let initial_state = control::save_and_disable();

        // Test enabling/disabling
        control::enable_all();
        assert!(control::are_enabled());

        control::disable_all();
        assert!(!control::are_enabled());

        // Restore initial state
        control::restore(initial_state);
    }

    #[test]
    fn test_critical_section() {
        control::enable_all();

        {
            let _guard = CriticalSection::enter();
            assert!(!control::are_enabled());
        }

        assert!(control::are_enabled());
    }
}