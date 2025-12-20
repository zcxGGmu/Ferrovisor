//! Interrupt handling module
//!
//! This module provides interrupt management for the hypervisor,
//! including interrupt controllers, interrupt routing, and exception handling.

use crate::{Result, Error};
use crate::core::arch::common;
use crate::core::mm::VirtAddr;
use crate::core::sync::SpinLock;
use crate::utils::bitmap::Bitmap;

pub mod chip;
pub mod handler;
pub mod exception;

/// Interrupt number type
pub type IrqNumber = u32;

/// Interrupt vector number
pub type Vector = u32;

/// IRQ types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrqType {
    /// Software generated interrupt
    Software,
    /// Hardware interrupt
    Hardware,
    /// Inter-processor interrupt (IPI)
    Ipi,
    /// Local timer interrupt
    LocalTimer,
    /// Performance monitoring interrupt
    Pmi,
    /// Non-maskable interrupt (NMI)
    Nmi,
}

/// Interrupt priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IrqPriority {
    /// Lowest priority
    Lowest = 0,
    /// Low priority
    Low = 1,
    /// Normal priority
    Normal = 2,
    /// High priority
    High = 3,
    /// Highest priority
    Highest = 4,
}

impl Default for IrqPriority {
    fn default() -> Self {
        IrqPriority::Normal
    }
}

/// Interrupt flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IrqFlags {
    /// Interrupt is edge-triggered
    pub edge_triggered: bool,
    /// Interrupt is active-high
    pub active_high: bool,
    /// Interrupt is shared
    pub shared: bool,
    /// Interrupt is level-sensitive
    pub level_sensitive: bool,
}

impl Default for IrqFlags {
    fn default() -> Self {
        Self {
            edge_triggered: true,
            active_high: true,
            shared: false,
            level_sensitive: false,
        }
    }
}

/// Interrupt descriptor
#[derive(Debug)]
pub struct IrqDescriptor {
    /// IRQ number
    pub irq: IrqNumber,
    /// IRQ type
    pub irq_type: IrqType,
    /// Priority
    pub priority: IrqPriority,
    /// Flags
    pub flags: IrqFlags,
    /// Handler function
    pub handler: Option<IrqHandler>,
    /// Handler argument
    pub handler_arg: Option<*mut u8>,
    /// Is this IRQ enabled
    pub enabled: bool,
    /// Is this IRQ currently pending
    pub pending: bool,
    /// Count of times this IRQ was triggered
    pub trigger_count: u64,
    /// Last trigger timestamp
    pub last_trigger_time: u64,
}

/// Interrupt handler function type
pub type IrqHandler = unsafe extern "C" fn(irq: IrqNumber, arg: *mut u8);

/// Interrupt controller interface
pub trait InterruptController {
    /// Initialize the interrupt controller
    fn init(&mut self) -> Result<()>;

    /// Enable an interrupt
    fn enable_irq(&mut self, irq: IrqNumber) -> Result<()>;

    /// Disable an interrupt
    fn disable_irq(&mut self, irq: IrqNumber) -> Result<()>;

    /// Acknowledge an interrupt
    fn ack_irq(&mut self, irq: IrqNumber) -> Result<()>;

    /// Set interrupt priority
    fn set_priority(&mut self, irq: IrqNumber, priority: IrqPriority) -> Result<()>;

    /// Set interrupt type (edge/level)
    fn set_type(&mut self, irq: IrqNumber, edge_triggered: bool) -> Result<()>;

    /// Get pending interrupts
    fn get_pending_irqs(&self) -> u64;

    /// Check if an interrupt is pending
    fn is_pending(&self, irq: IrqNumber) -> bool;

    /// Handle interrupt
    fn handle_interrupt(&mut self) -> Option<IrqNumber>;
}

/// Generic interrupt manager
pub struct InterruptManager {
    /// IRQ descriptors
    irqs: SpinLock<[Option<IrqDescriptor>; 256]>,
    /// IRQ allocation bitmap
    irq_bitmap: SpinLock<Bitmap>,
    /// Current pending interrupt bitmap
    pending_irqs: SpinLock<u64>,
    /// Interrupt controller
    controller: SpinLock<*mut dyn InterruptController>,
    /// Statistics
    stats: SpinLock<IrqStats>,
}

/// Interrupt statistics
#[derive(Debug, Clone, Copy)]
pub struct IrqStats {
    /// Total interrupts handled
    pub total_interrupts: u64,
    /// Interrupts per type
    pub interrupts_by_type: [u64; 4], // Software, Hardware, IPI, Other
    /// Spurious interrupts
    pub spurious_interrupts: u64,
    /// Missed interrupts
    pub missed_interrupts: u64,
    /// Interrupt latency (average)
    pub avg_latency_us: f64,
}

impl InterruptManager {
    /// Create a new interrupt manager
    pub const fn new() -> Self {
        Self {
            irqs: SpinLock::new([None; 256]),
            irq_bitmap: SpinLock::new(unsafe {
                Bitmap::new(core::ptr::null_mut(), 256)
            }),
            pending_irqs: SpinLock::new(0),
            controller: SpinLock::new(core::ptr::null_mut()),
            stats: SpinLock::new(IrqStats {
                total_interrupts: 0,
                interrupts_by_type: [0; 4],
                spurious_interrupts: 0,
                missed_interrupts: 0,
                avg_latency_us: 0.0,
            }),
        }
    }

    /// Set the interrupt controller
    pub fn set_controller(&self, controller: *mut dyn InterruptController) {
        let mut ctrl = self.controller.lock();
        *ctrl = controller;
    }

    /// Allocate an IRQ number
    pub fn allocate_irq(&self) -> Result<IrqNumber> {
        let mut bitmap = self.irq_bitmap.lock();
        if let Some(index) = bitmap.find_and_set() {
            Ok(index as IrqNumber)
        } else {
            Err(Error::ResourceUnavailable)
        }
    }

    /// Free an IRQ number
    pub fn free_irq(&self, irq: IrqNumber) -> Result<()> {
        if irq >= 256 {
            return Err(Error::InvalidArgument);
        }

        // Clear the descriptor first
        {
            let mut irqs = self.irqs.lock();
            if irqs[irq as usize].is_some() {
                irqs[irq as usize] = None;
            }
        }

        // Free the bitmap entry
        let mut bitmap = self.irq_bitmap.lock();
        if bitmap.clear_bit(irq as usize) {
            Ok(())
        } else {
            Err(Error::InvalidArgument)
        }
    }

    /// Register an interrupt handler
    pub fn register_irq_handler(
        &self,
        irq: IrqNumber,
        handler: IrqHandler,
        arg: *mut u8,
    ) -> Result<()> {
        if irq >= 256 {
            return Err(Error::InvalidArgument);
        }

        let mut irqs = self.irqs.lock();
        if let Some(desc) = &mut irqs[irq as usize] {
            desc.handler = Some(handler);
            desc.handler_arg = Some(arg);
            Ok(())
        } else {
            Err(Error::NotFound)
        }
    }

    /// Unregister an interrupt handler
    pub fn unregister_irq_handler(&self, irq: IrqNumber) -> Result<()> {
        if irq >= 256 {
            return Err(Error::InvalidArgument);
        }

        let mut irqs = self.irqs.lock();
        if let Some(desc) = &mut irqs[irq as usize] {
            desc.handler = None;
            desc.handler_arg = None;
            Ok(())
        } else {
            Err(Error::NotFound)
        }
    }

    /// Enable an interrupt
    pub fn enable_irq(&self, irq: IrqNumber) -> Result<()> {
        if irq >= 256 {
            return Err(Error::InvalidArgument);
        }

        // Mark as enabled in descriptor
        {
            let mut irqs = self.irqs.lock();
            if let Some(desc) = &mut irqs[irq as usize] {
                desc.enabled = true;
            } else {
                return Err(Error::NotFound);
            }
        }

        // Enable in hardware controller
        {
            let mut controller = self.controller.lock();
            if !controller.is_null() {
                unsafe { (*controller).enable_irq(irq)?; }
            }
        }

        Ok(())
    }

    /// Disable an interrupt
    pub fn disable_irq(&self, irq: IrqNumber) -> Result<()> {
        if irq >= 256 {
            return Err(Error::InvalidArgument);
        }

        // Mark as disabled in descriptor
        {
            let mut irqs = self.irqs.lock();
            if let Some(desc) = &mut irqs[irq as usize] {
                desc.enabled = false;
            } else {
                return Err(Error::NotFound);
            }
        }

        // Disable in hardware controller
        {
            let mut controller = self.controller.lock();
            if !controller.is_null() {
                unsafe { (*controller).disable_irq(irq)?; }
            }
        }

        Ok(())
    }

    /// Trigger a software interrupt
    pub fn trigger_soft_irq(&self, irq: IrqNumber) -> Result<()> {
        if irq >= 256 {
            return Err(Error::InvalidArgument);
        }

        let current_time = crate::utils::get_timestamp();

        {
            let mut irqs = self.irqs.lock();
            if let Some(desc) = &mut irqs[irq as usize] {
                if desc.enabled {
                    desc.pending = true;
                    desc.trigger_count += 1;
                    desc.last_trigger_time = current_time;

                    // Set pending in bitmap
                    let mut pending = self.pending_irqs.lock();
                    *pending |= 1u64 << irq;

                    crate::debug!("Triggered software IRQ {}", irq);
                }
            }
        }

        Ok(())
    }

    /// Clear an interrupt
    pub fn clear_irq(&self, irq: IrqNumber) -> Result<()> {
        if irq >= 256 {
            return Err(Error::InvalidArgument);
        }

        {
            let mut irqs = self.irqs.lock();
            if let Some(desc) = &mut irqs[irq as usize] {
                desc.pending = false;

                // Clear pending in bitmap
                let mut pending = self.pending_irqs.lock();
                *pending &= !(1u64 << irq);
            }
        }

        Ok(())
    }

    /// Get pending interrupts bitmap
    pub fn get_pending_irqs(&self) -> u64 {
        *self.pending_irqs.lock()
    }

    /// Check if an interrupt is pending
    pub fn is_pending(&self, irq: IrqNumber) -> bool {
        if irq >= 256 {
            return false;
        }

        let pending = *self.pending_irqs.lock();
        (pending & (1u64 << irq)) != 0
    }

    /// Handle pending interrupts
    pub fn handle_pending_interrupts(&self) -> Result<()> {
        let start_time = crate::utils::get_timestamp();

        // Handle hardware interrupts first
        {
            let mut controller = self.controller.lock();
            if !controller.is_null() {
                while let Some(irq) = unsafe { (*controller).handle_interrupt() } {
                    self.handle_irq(irq, start_time)?;
                }
            }
        }

        // Handle software interrupts
        let pending = self.get_pending_irqs();
        for irq in 0..256 {
            if (pending & (1u64 << irq)) != 0 {
                self.handle_irq(irq, start_time)?;
            }
        }

        Ok(())
    }

    /// Handle a specific interrupt
    pub fn handle_irq(&self, irq: IrqNumber, start_time: u64) -> Result<()> {
        if irq >= 256 {
            return Err(Error::InvalidArgument);
        }

        let end_time = crate::utils::get_timestamp();
        let latency = end_time.saturating_sub(start_time);

        {
            let mut irqs = self.irqs.lock();
            let mut stats = self.stats.lock();

            if let Some(desc) = &mut irqs[irq as usize] {
                if desc.pending && desc.enabled {
                    // Clear pending flag
                    desc.pending = false;
                    {
                        let mut pending = self.pending_irqs.lock();
                        *pending &= !(1u64 << irq);
                    }

                    // Call handler if registered
                    if let Some(handler) = desc.handler {
                        let arg = desc.handler_arg.unwrap_or(core::ptr::null_mut());
                        unsafe {
                            handler(irq, arg);
                        }

                        // Update statistics
                        stats.total_interrupts += 1;

                        match desc.irq_type {
                            IrqType::Software => stats.interrupts_by_type[0] += 1,
                            IrqType::Hardware => stats.interrupts_by_type[1] += 1,
                            IrqType::Ipi => stats.interrupts_by_type[2] += 1,
                            _ => stats.interrupts_by_type[3] += 1,
                        }

                        // Update average latency
                        let alpha = 0.1;
                        stats.avg_latency_us = stats.avg_latency_us * (1.0 - alpha) + (latency as f64) * alpha;
                    } else {
                        // No handler registered
                        stats.spurious_interrupts += 1;
                        crate::warn!("No handler registered for IRQ {}", irq);
                    }

                    // Acknowledge in hardware
                    {
                        let mut controller = self.controller.lock();
                        if !controller.is_null() {
                            unsafe { (*controller).ack_irq(irq)?; }
                        }
                    }
                }
            } else {
                stats.spurious_interrupts += 1;
            }
        }

        Ok(())
    }

    /// Get interrupt statistics
    pub fn get_stats(&self) -> IrqStats {
        *self.stats.lock()
    }

    /// Create a new IRQ descriptor
    pub fn create_irq_descriptor(
        &self,
        irq: IrqNumber,
        irq_type: IrqType,
        priority: IrqPriority,
        flags: IrqFlags,
    ) -> Result<()> {
        if irq >= 256 {
            return Err(Error::InvalidArgument);
        }

        let mut irqs = self.irqs.lock();
        if irqs[irq as usize].is_none() {
            irqs[irq as usize] = Some(IrqDescriptor {
                irq,
                irq_type,
                priority,
                flags,
                handler: None,
                handler_arg: None,
                enabled: false,
                pending: false,
                trigger_count: 0,
                last_trigger_time: 0,
            });
            Ok(())
        } else {
            Err(Error::ResourceBusy)
        }
    }
}

/// Global interrupt manager instance
static mut IRQ_MANAGER: Option<InterruptManager> = None;
static IRQ_MANAGER_INIT: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// Initialize the interrupt management subsystem
pub fn init() -> Result<()> {
    crate::info!("Initializing interrupt management");

    unsafe {
        if IRQ_MANAGER.is_none() {
            // Initialize bitmap memory
            let bitmap_data = [0u64; 4]; // 256 bits / 64 bits per u64
            IRQ_MANAGER = Some(InterruptManager {
                irqs: SpinLock::new([None; 256]),
                irq_bitmap: SpinLock::new(unsafe {
                    Bitmap::new(bitmap_data.as_ptr() as *mut u64, 256)
                }),
                pending_irqs: SpinLock::new(0),
                controller: SpinLock::new(core::ptr::null_mut()),
                stats: SpinLock::new(IrqStats {
                    total_interrupts: 0,
                    interrupts_by_type: [0; 4],
                    spurious_interrupts: 0,
                    missed_interrupts: 0,
                    avg_latency_us: 0.0,
                }),
            });
            IRQ_MANAGER_INIT.store(true, core::sync::atomic::Ordering::Release);
        }
    }

    Ok(())
}

/// Get the global interrupt manager
pub fn get() -> &'static InterruptManager {
    unsafe {
        IRQ_MANAGER.as_ref().unwrap()
    }
}

/// Allocate an IRQ number
pub fn allocate_irq() -> Result<IrqNumber> {
    get().allocate_irq()
}

/// Free an IRQ number
pub fn free_irq(irq: IrqNumber) -> Result<()> {
    get().free_irq(irq)
}

/// Register an interrupt handler
pub fn register_irq_handler(irq: IrqNumber, handler: IrqHandler, arg: *mut u8) -> Result<()> {
    get().register_irq_handler(irq, handler, arg)
}

/// Enable an interrupt
pub fn enable_irq(irq: IrqNumber) -> Result<()> {
    get().enable_irq(irq)
}

/// Disable an interrupt
pub fn disable_irq(irq: IrqNumber) -> Result<()> {
    get().disable_irq(irq)
}

/// Trigger a software interrupt
pub fn trigger_soft_irq(irq: IrqNumber) -> Result<()> {
    get().trigger_soft_irq(irq)
}

/// Clear an interrupt
pub fn clear_irq(irq: IrqNumber) -> Result<()> {
    get().clear_irq(irq)
}

/// Handle pending interrupts
pub fn handle_pending_interrupts() -> Result<()> {
    get().handle_pending_interrupts()
}

/// Create an IRQ descriptor
pub fn create_irq_descriptor(
    irq: IrqNumber,
    irq_type: IrqType,
    priority: IrqPriority,
    flags: IrqFlags,
) -> Result<()> {
    get().create_irq_descriptor(irq, irq_type, priority, flags)
}

/// Get interrupt statistics
pub fn get_stats() -> IrqStats {
    get().get_stats()
}

/// Exception handling
pub mod exception {
    use super::*;

    /// Exception types
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ExceptionType {
        /// Undefined instruction
        UndefinedInstruction,
        /// Software interrupt
        SoftwareInterrupt,
        /// Prefetch abort
        PrefetchAbort,
        /// Data abort
        DataAbort,
        /// Hypervisor call
        HypervisorCall,
        /// IRQ or FIQ
        IrqOrFiq,
        /// Debug
        Debug,
    }

    /// Exception handler function type
    pub type ExceptionHandler = unsafe extern "C" fn(
        exception_type: ExceptionType,
        exception_code: u32,
        fault_address: VirtAddr,
    );

    /// Register an exception handler
    pub fn register_handler(
        exception_type: ExceptionType,
        handler: ExceptionHandler,
    ) -> Result<()> {
        // TODO: Implement exception handler registration
        Err(Error::NotImplemented)
    }

    /// Handle an exception
    pub fn handle_exception(
        exception_type: ExceptionType,
        exception_code: u32,
        fault_address: VirtAddr,
    ) -> Result<()> {
        crate::error!(
            "Exception: {:?}, code: {}, addr: 0x{:x}",
            exception_type,
            exception_code,
            fault_address
        );

        // For now, just return error
        Err(Error::InvalidState)
    }
}

/// Timer interrupt handling
pub mod timer {
    use super::*;

    /// Timer tick handler
    pub fn handle_timer_tick() -> Result<()> {
        // Update scheduler
        crate::core::sched::handle_tick()?;

        // Handle timer-based scheduling
        let cpu_id = crate::core::cpu_id();
        crate::core::sched::schedule(cpu_id)?;

        Ok(())
    }
}

/// Inter-processor interrupt handling
pub mod ipi {
    use super::*;

    /// IPI types
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum IpiType {
        /// Reschedule
        Reschedule,
        /// Function call
        FunctionCall,
        /// TLB flush
        TlbFlush,
        /// Stop CPU
        StopCpu,
    }

    /// Send an IPI to a specific CPU
    pub fn send_ipi(cpu_id: usize, ipi_type: IpiType) -> Result<()> {
        crate::debug!("Sending IPI {:?} to CPU {}", ipi_type, cpu_id);

        // TODO: Implement IPI sending
        match ipi_type {
            IpiType::Reschedule => {
                // Trigger scheduler tick
                crate::core::sched::handle_tick()?;
            }
            _ => {
                // TODO: Implement other IPI types
            }
        }

        Ok(())
    }

    /// Broadcast an IPI to all CPUs
    pub fn broadcast_ipi(ipi_type: IpiType) -> Result<()> {
        crate::debug!("Broadcasting IPI {:?}", ipi_type);

        // TODO: Implement IPI broadcasting
        match ipi_type {
            IpiType::Reschedule => {
                // Trigger scheduler tick on all CPUs
                for cpu_id in 0..64 {
                    if let Err(e) = send_ipi(cpu_id, ipi_type) {
                        crate::error!("Failed to send IPI to CPU {}: {:?}", cpu_id, e);
                    }
                }
            }
            _ => {
                // TODO: Implement other IPI types
            }
        }

        Ok(())
    }
}