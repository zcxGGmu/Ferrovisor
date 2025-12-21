//! Exception handling module
//!
//! This module provides exception handling support for different architectures,
//! including exception types, handlers, and context management.

use crate::{Result, Error};
use crate::core::irq::{IrqNumber, IrqHandler};
use crate::core::sync::SpinLock;
use core::sync::atomic::{AtomicU64, Ordering};

/// Exception types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceptionType {
    /// Reset exception
    Reset = 0,
    /// Undefined instruction
    UndefinedInstruction = 1,
    /// Supervisor call (SVC)
    SupervisorCall = 2,
    /// Prefetch abort
    PrefetchAbort = 3,
    /// Data abort
    DataAbort = 4,
    /// Hypervisor call (HVC)
    HypervisorCall = 5,
    /// IRQ (Interrupt Request)
    Irq = 6,
    /// FIQ (Fast Interrupt Request)
    Fiq = 7,
    /// System call
    SystemCall = 8,
    /// Page fault
    PageFault = 9,
    /// General protection fault
    GeneralProtectionFault = 10,
    /// Alignment fault
    AlignmentFault = 11,
    /// Divide by zero
    DivideByZero = 12,
    /// Overflow
    Overflow = 13,
    /// Underflow
    Underflow = 14,
    /// Invalid instruction
    InvalidInstruction = 15,
    /// Invalid opcode
    InvalidOpcode = 16,
    /// Stack overflow
    StackOverflow = 17,
    /// Stack underflow
    StackUnderflow = 18,
    /// Privilege violation
    PrivilegeViolation = 19,
    /// Access violation
    AccessViolation = 20,
    /// Other exception
    Other(u32),
}

impl From<u32> for ExceptionType {
    fn from(value: u32) -> Self {
        match value {
            0 => ExceptionType::Reset,
            1 => ExceptionType::UndefinedInstruction,
            2 => ExceptionType::SupervisorCall,
            3 => ExceptionType::PrefetchAbort,
            4 => ExceptionType::DataAbort,
            5 => ExceptionType::HypervisorCall,
            6 => ExceptionType::Irq,
            7 => ExceptionType::Fiq,
            8 => ExceptionType::SystemCall,
            9 => ExceptionType::PageFault,
            10 => ExceptionType::GeneralProtectionFault,
            11 => ExceptionType::AlignmentFault,
            12 => ExceptionType::DivideByZero,
            13 => ExceptionType::Overflow,
            14 => ExceptionType::Underflow,
            15 => ExceptionType::InvalidInstruction,
            16 => ExceptionType::InvalidOpcode,
            17 => ExceptionType::StackOverflow,
            18 => ExceptionType::StackUnderflow,
            19 => ExceptionType::PrivilegeViolation,
            20 => ExceptionType::AccessViolation,
            other => ExceptionType::Other(other),
        }
    }
}

/// Exception context information
#[repr(C)]
#[derive(Debug, Clone)]
pub struct ExceptionContext {
    /// Program counter at time of exception
    pub pc: u64,
    /// Processor state register
    pub psr: u64,
    /// Stack pointer
    pub sp: u64,
    /// General purpose registers
    pub regs: [u64; 31],
    /// Exception type
    pub exception_type: ExceptionType,
    /// Exception syndrome information
    pub syndrome: u64,
    /// Fault address register
    pub far: u64,
    /// Virtualization information
    pub virt_info: VirtExceptionInfo,
}

/// Virtualization-specific exception information
#[derive(Debug, Clone, Copy)]
pub struct VirtExceptionInfo {
    /// VM ID that caused the exception
    pub vm_id: u32,
    /// VCPU ID that caused the exception
    pub vcpu_id: u32,
    /// Exception originated from guest
    pub from_guest: bool,
    /// Exception was injected by hypervisor
    pub injected: bool,
    /// Exception class for virtualization
    pub virt_class: u8,
}

/// Exception handler trait
pub trait ExceptionHandler {
    /// Handle the exception
    fn handle(&mut self, ctx: &mut ExceptionContext) -> Result<ExceptionAction>;

    /// Get the handler name
    fn name(&self) -> &'static str;
}

/// Actions to take after exception handling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceptionAction {
    /// Resume execution
    Resume,
    /// Skip the faulting instruction
    SkipInstruction,
    /// Terminate current thread/process
    Terminate,
    /// Panic (critical error)
    Panic,
    /// Inject into guest VM
    InjectGuest,
    /// Emulate the faulting instruction
    Emulate,
    /// Retry the operation
    Retry,
}

/// Exception handler function
pub type ExceptionHandlerFn = fn(&mut ExceptionContext) -> Result<ExceptionAction>;

/// Function-based exception handler
pub struct FnExceptionHandler {
    name: &'static str,
    handler: ExceptionHandlerFn,
}

impl FnExceptionHandler {
    /// Create a new function-based handler
    pub const fn new(name: &'static str, handler: ExceptionHandlerFn) -> Self {
        Self { name, handler }
    }
}

impl ExceptionHandler for FnExceptionHandler {
    fn handle(&mut self, ctx: &mut ExceptionContext) -> Result<ExceptionAction> {
        (self.handler)(ctx)
    }

    fn name(&self) -> &'static str {
        self.name
    }
}

/// Exception descriptor
#[derive(Debug)]
pub struct ExceptionDescriptor {
    /// Exception number
    pub number: u32,
    /// Exception type
    pub exception_type: ExceptionType,
    /// Exception name
    pub name: &'static str,
    /// Description
    pub description: &'static str,
    /// Default handler
    pub default_handler: Option<Box<dyn ExceptionHandler>>,
    /// Handler has been installed
    pub handler_installed: bool,
    /// Exception count
    pub count: AtomicU64,
}

impl ExceptionDescriptor {
    /// Create a new exception descriptor
    pub fn new(
        number: u32,
        exception_type: ExceptionType,
        name: &'static str,
        description: &'static str,
    ) -> Self {
        Self {
            number,
            exception_type,
            name,
            description,
            default_handler: None,
            handler_installed: false,
            count: AtomicU64::new(0),
        }
    }

    /// Get the exception count
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    /// Increment the exception count
    pub fn increment(&self) {
        self.count.fetch_add(1, Ordering::Relaxed);
    }
}

/// Exception manager
pub struct ExceptionManager {
    /// Exception descriptors
    exceptions: SpinLock<[Option<ExceptionDescriptor>; 64]>,
    /// Current exception context (for nesting)
    current_context: SpinLock<Option<ExceptionContext>>,
    /// Exception depth (nesting level)
    exception_depth: AtomicU64,
    /// Statistics
    stats: SpinLock<ExceptionStats>,
}

/// Exception statistics
#[derive(Debug, Default, Clone, Copy)]
pub struct ExceptionStats {
    /// Total exceptions handled
    pub total_exceptions: u64,
    /// Exceptions per type
    pub exceptions_per_type: [u64; 32],
    /// Maximum exception depth
    pub max_depth: u64,
    /// Handled successfully
    pub handled_successfully: u64,
    /// Failed to handle
    pub handling_failed: u64,
}

impl ExceptionManager {
    /// Create a new exception manager
    pub const fn new() -> Self {
        Self {
            exceptions: SpinLock::new([const { None }; 64]),
            current_context: SpinLock::new(None),
            exception_depth: AtomicU64::new(0),
            stats: SpinLock::new(ExceptionStats::default()),
        }
    }

    /// Initialize exception handling
    pub fn init(&self) -> Result<()> {
        crate::info!("Initializing exception manager");

        // Register standard exceptions
        self.register_standard_exceptions()?;

        // Set up exception vectors
        self.setup_exception_vectors()?;

        crate::info!("Exception manager initialized");

        Ok(())
    }

    /// Register an exception handler
    pub fn register_handler(
        &self,
        exception_num: u32,
        handler: Box<dyn ExceptionHandler>,
    ) -> Result<()> {
        let mut exceptions = self.exceptions.lock();
        if exception_num as usize >= exceptions.len() {
            return Err(Error::InvalidArgument);
        }

        if let Some(ref mut desc) = exceptions[exception_num as usize] {
            desc.default_handler = Some(handler);
            desc.handler_installed = true;
            crate::info!("Registered handler for exception {}: {}", exception_num, desc.name);
            Ok(())
        } else {
            Err(Error::NotFound)
        }
    }

    /// Handle an exception
    pub fn handle_exception(&self, mut ctx: ExceptionContext) -> Result<()> {
        let depth = self.exception_depth.fetch_add(1, Ordering::Relaxed) + 1;

        // Update statistics
        {
            let mut stats = self.stats.lock();
            stats.total_exceptions += 1;
            if depth > stats.max_depth {
                stats.max_depth = depth;
            }
            let type_idx = (ctx.exception_type as u32) as usize % 32;
            stats.exceptions_per_type[type_idx] += 1;
        }

        // Save current context
        {
            let mut current = self.current_context.lock();
            *current = Some(ctx.clone());
        }

        crate::warn!("Exception occurred: {:?} at PC={:#x}, SP={:#x}",
                     ctx.exception_type, ctx.pc, ctx.sp);

        let action = {
            let exceptions = self.exceptions.lock();
            let exception_num = ctx.exception_type as u32;

            if let Some(ref desc) = exceptions[exception_num as usize] {
                desc.increment();

                if let Some(ref mut handler) = desc.default_handler {
                    match handler.handle(&mut ctx) {
                        Ok(action) => {
                            crate::info!("Exception handled by {}: {:?}",
                                       handler.name(), action);
                            {
                                let mut stats = self.stats.lock();
                                stats.handled_successfully += 1;
                            }
                            action
                        }
                        Err(e) => {
                            crate::error!("Exception handler failed: {:?}", e);
                            {
                                let mut stats = self.stats.lock();
                                stats.handling_failed += 1;
                            }
                            ExceptionAction::Panic
                        }
                    }
                } else {
                    crate::error!("No handler for exception {}: {}",
                                exception_num, desc.name);
                    ExceptionAction::Panic
                }
            } else {
                crate::error!("Unknown exception: {}", exception_num);
                ExceptionAction::Panic
            }
        };

        // Take action based on handler response
        self.take_action(action, &ctx)?;

        // Restore context and decrement depth
        {
            let mut current = self.current_context.lock();
            *current = None;
        }
        self.exception_depth.fetch_sub(1, Ordering::Relaxed);

        Ok(())
    }

    /// Take action based on exception handling result
    fn take_action(&self, action: ExceptionAction, ctx: &ExceptionContext) -> Result<()> {
        match action {
            ExceptionAction::Resume => {
                // Context will be restored automatically
                Ok(())
            }
            ExceptionAction::SkipInstruction => {
                // This would be implemented in assembly
                crate::info!("Skipping faulting instruction at {:#x}", ctx.pc);
                Ok(())
            }
            ExceptionAction::Terminate => {
                crate::warn!("Terminating due to exception");
                // Terminate current thread
                crate::panic!("Thread terminated due to exception");
            }
            ExceptionAction::Panic => {
                crate::error!("Panicking due to critical exception");
                crate::panic!("Critical exception occurred");
            }
            ExceptionAction::InjectGuest => {
                crate::info!("Injecting exception into guest VM {}", ctx.virt_info.vm_id);
                // This would inject the exception into the guest VM
                Ok(())
            }
            ExceptionAction::Emulate => {
                crate::info!("Emulating faulting instruction at {:#x}", ctx.pc);
                // This would trigger instruction emulation
                Ok(())
            }
            ExceptionAction::Retry => {
                crate::info!("Retrying faulting operation");
                // This would retry the faulting operation
                Ok(())
            }
        }
    }

    /// Register standard exceptions
    fn register_standard_exceptions(&self) -> Result<()> {
        let mut exceptions = self.exceptions.lock();

        // Reset
        exceptions[0] = Some(ExceptionDescriptor::new(
            0,
            ExceptionType::Reset,
            "Reset",
            "System reset exception"
        ));

        // Undefined instruction
        exceptions[1] = Some(ExceptionDescriptor::new(
            1,
            ExceptionType::UndefinedInstruction,
            "Undefined Instruction",
            "Undefined or illegal instruction"
        ));

        // Supervisor call
        exceptions[2] = Some(ExceptionDescriptor::new(
            2,
            ExceptionType::SupervisorCall,
            "Supervisor Call",
            "Supervisor mode call (SVC)"
        ));

        // Prefetch abort
        exceptions[3] = Some(ExceptionDescriptor::new(
            3,
            ExceptionType::PrefetchAbort,
            "Prefetch Abort",
            "Instruction prefetch abort"
        ));

        // Data abort
        exceptions[4] = Some(ExceptionDescriptor::new(
            4,
            ExceptionType::DataAbort,
            "Data Abort",
            "Data access abort"
        ));

        // Hypervisor call
        exceptions[5] = Some(ExceptionDescriptor::new(
            5,
            ExceptionType::HypervisorCall,
            "Hypervisor Call",
            "Hypervisor mode call (HVC)"
        ));

        // IRQ
        exceptions[6] = Some(ExceptionDescriptor::new(
            6,
            ExceptionType::Irq,
            "IRQ",
            "Interrupt request"
        ));

        // FIQ
        exceptions[7] = Some(ExceptionDescriptor::new(
            7,
            ExceptionType::Fiq,
            "FIQ",
            "Fast interrupt request"
        ));

        // System call
        exceptions[8] = Some(ExceptionDescriptor::new(
            8,
            ExceptionType::SystemCall,
            "System Call",
            "System call from user mode"
        ));

        // Page fault
        exceptions[9] = Some(ExceptionDescriptor::new(
            9,
            ExceptionType::PageFault,
            "Page Fault",
            "Page fault exception"
        ));

        // General protection fault
        exceptions[10] = Some(ExceptionDescriptor::new(
            10,
            ExceptionType::GeneralProtectionFault,
            "General Protection Fault",
            "General protection fault"
        ));

        Ok(())
    }

    /// Set up exception vectors
    fn setup_exception_vectors(&self) -> Result<()> {
        // This would set up the exception vector table
        // Implementation depends on architecture
        crate::info!("Setting up exception vectors");
        Ok(())
    }

    /// Get exception statistics
    pub fn get_stats(&self) -> ExceptionStats {
        *self.stats.lock()
    }

    /// Get exception descriptor
    pub fn get_exception(&self, num: u32) -> Option<ExceptionDescriptor> {
        let exceptions = self.exceptions.lock();
        if (num as usize) < exceptions.len() {
            exceptions[num as usize].clone()
        } else {
            None
        }
    }

    /// Get current exception depth
    pub fn exception_depth(&self) -> u64 {
        self.exception_depth.load(Ordering::Relaxed)
    }

    /// Get current exception context
    pub fn current_context(&self) -> Option<ExceptionContext> {
        self.current_context.lock().clone()
    }
}

/// Default exception handlers
pub mod handlers {
    use super::*;

    /// Default reset handler
    pub fn reset_handler(ctx: &mut ExceptionContext) -> Result<ExceptionAction> {
        crate::warn!("Reset exception at PC={:#x}", ctx.pc);
        Ok(ExceptionAction::Panic)
    }

    /// Default undefined instruction handler
    pub fn undefined_instruction_handler(ctx: &mut ExceptionContext) -> Result<ExceptionAction> {
        crate::warn!("Undefined instruction at PC={:#x}", ctx.pc);

        if ctx.virt_info.from_guest {
            // Inject into guest
            Ok(ExceptionAction::InjectGuest)
        } else {
            // Terminate
            Ok(ExceptionAction::Terminate)
        }
    }

    /// Default data abort handler
    pub fn data_abort_handler(ctx: &mut ExceptionContext) -> Result<ExceptionAction> {
        crate::warn!("Data abort at PC={:#x}, FAR={:#x}", ctx.pc, ctx.far);

        if ctx.virt_info.from_guest {
            // Inject into guest
            Ok(ExceptionAction::InjectGuest)
        } else {
            // Terminate
            Ok(ExceptionAction::Terminate)
        }
    }

    /// Default page fault handler
    pub fn page_fault_handler(ctx: &mut ExceptionContext) -> Result<ExceptionAction> {
        crate::warn!("Page fault at PC={:#x}, FAR={:#x}", ctx.pc, ctx.far);

        // This would trigger page fault handling
        Ok(ExceptionAction::Retry)
    }

    /// Default supervisor call handler
    pub fn supervisor_call_handler(ctx: &mut ExceptionContext) -> Result<ExceptionAction> {
        crate::info!("Supervisor call at PC={:#x}", ctx.pc);
        // Handle system call
        Ok(ExceptionAction::Resume)
    }

    /// Default hypervisor call handler
    pub fn hypervisor_call_handler(ctx: &mut ExceptionContext) -> Result<ExceptionAction> {
        crate::info!("Hypervisor call at PC={:#x}", ctx.pc);
        // Handle hypervisor call
        Ok(ExceptionAction::Resume)
    }
}

/// Global exception manager instance
static EXCEPTION_MANAGER: SpinLock<Option<ExceptionManager>> = SpinLock::new(None);

/// Initialize the exception subsystem
pub fn init() -> Result<()> {
    let manager = ExceptionManager::new();
    manager.init()?;

    {
        let mut global = EXCEPTION_MANAGER.lock();
        *global = Some(manager);
    }

    Ok(())
}

/// Get the global exception manager
pub fn get() -> &'static SpinLock<Option<ExceptionManager>> {
    &EXCEPTION_MANAGER
}