//! Interrupt handling module
//!
//! This module provides interrupt management for the hypervisor,
//! including interrupt controllers, interrupt routing, and exception handling.

use crate::{Result, Error};
use crate::arch::common;
use crate::core::mm::VirtAddr;
use crate::core::sync::SpinLock;
use crate::utils::bitmap::Bitmap;

pub mod chip;
pub mod handler;
pub mod exception;
pub mod msi;

// Re-export commonly used types
pub use chip::{Plic, Aplic, Imsic, AplicSourceCfg, AplicMsiConfig, ImsicGlobalConfig, ImsicLocalConfig};
pub use chip::{AplicStats, ImsicStats, create_aplic, create_imsic, init_nextgen_interrupts};
pub use msi::{MsiAddress, MsiController, MsiXController, MsiXVector, create_msi_controller, create_msix_controller};

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
}

/// Interrupt priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
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

/// Interrupt controller trait
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
    fn set_priority(&mut self, irq: IrqNumber, priority: Priority) -> Result<()>;

    /// Set interrupt type (edge/level triggered)
    fn set_type(&mut self, irq: IrqNumber, edge_triggered: bool) -> Result<()>;

    /// Get pending interrupts as bitmap
    fn get_pending_irqs(&self) -> u64;

    /// Check if a specific interrupt is pending
    fn is_pending(&self, irq: IrqNumber) -> bool;

    /// Handle the next pending interrupt
    fn handle_interrupt(&mut self) -> Option<IrqNumber>;
}

/// Interrupt descriptor
#[derive(Debug, Clone)]
pub struct InterruptDescriptor {
    /// IRQ number
    pub irq: IrqNumber,
    /// IRQ type
    pub irq_type: IrqType,
    /// Priority
    pub priority: Priority,
    /// CPU affinity
    pub cpu_affinity: u64,
    /// Handler function
    pub handler: Option<InterruptHandler>,
    /// Handler context
    pub context: Option<*mut core::ffi::c_void>,
}

impl InterruptDescriptor {
    /// Create a new interrupt descriptor
    pub fn new(irq: IrqNumber, irq_type: IrqType, priority: Priority) -> Self {
        Self {
            irq,
            irq_type,
            priority,
            cpu_affinity: 0, // All CPUs by default
            handler: None,
            context: None,
        }
    }

    /// Set handler
    pub fn set_handler(&mut self, handler: InterruptHandler, context: Option<*mut core::ffi::c_void>) {
        self.handler = Some(handler);
        self.context = context;
    }
}

/// Interrupt handler function type
pub type InterruptHandler = fn(irq: IrqNumber, context: Option<*mut core::ffi::c_void>) -> Result<()>;

/// IRQ manager
pub struct IrqManager {
    /// Interrupt descriptors
    descriptors: SpinLock<[Option<InterruptDescriptor>; 1024]>,
    /// IRQ bitmap for tracking active IRQs
    irq_bitmap: SpinLock<Bitmap>,
    /// Statistics
    stats: SpinLock<IrqStats>,
    /// Platform interrupt controller
    controller: SpinLock<Option<Box<dyn InterruptController>>>,
}

/// IRQ statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct IrqStats {
    /// Total interrupts handled
    pub total_interrupts: u64,
    /// Hardware interrupts
    pub hardware_interrupts: u64,
    /// Software interrupts
    pub software_interrupts: u64,
    /// IPIs handled
    pub ipi_count: u64,
    /// Spurious interrupts
    pub spurious_interrupts: u64,
}

impl IrqManager {
    /// Create a new IRQ manager
    pub const fn new() -> Self {
        Self {
            descriptors: SpinLock::new([None; 1024]),
            irq_bitmap: SpinLock::new(unsafe { Bitmap::new(core::ptr::null_mut(), 1024) }),
            stats: SpinLock::new(IrqStats::default()),
            controller: SpinLock::new(None),
        }
    }

    /// Set the platform interrupt controller
    pub fn set_controller(&self, controller: Box<dyn InterruptController>) {
        *self.controller.lock() = Some(controller);
    }

    /// Get a reference to the interrupt controller
    pub fn with_controller<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut dyn InterruptController) -> R,
    {
        let mut controller = self.controller.lock();
        if let Some(ref mut ctrl) = *controller {
            Some(f(ctrl.as_mut()))
        } else {
            None
        }
    }

    /// Register an interrupt
    pub fn register_irq(&self, descriptor: InterruptDescriptor) -> Result<()> {
        let mut descriptors = self.descriptors.lock();
        let irq = descriptor.irq as usize;

        if irq >= 1024 {
            return Err(Error::InvalidArgument);
        }

        if descriptors[irq].is_some() {
            return Err(Error::ResourceBusy);
        }

        descriptors[irq] = Some(descriptor);
        self.irq_bitmap.lock().set_bit(irq);

        Ok(())
    }

    /// Unregister an interrupt
    pub fn unregister_irq(&self, irq: IrqNumber) -> Result<()> {
        let mut descriptors = self.descriptors.lock();
        let irq = irq as usize;

        if irq >= 1024 {
            return Err(Error::InvalidArgument);
        }

        if descriptors[irq].is_none() {
            return Err(Error::NotFound);
        }

        descriptors[irq] = None;
        self.irq_bitmap.lock().clear_bit(irq);

        Ok(())
    }

    /// Handle an interrupt
    pub fn handle_irq(&self, irq: IrqNumber) -> Result<()> {
        let descriptors = self.descriptors.lock();
        let irq = irq as usize;

        if irq >= 1024 {
            return Err(Error::InvalidArgument);
        }

        if let Some(ref descriptor) = descriptors[irq] {
            // Update statistics
            {
                let mut stats = self.stats.lock();
                stats.total_interrupts += 1;

                match descriptor.irq_type {
                    IrqType::Hardware => stats.hardware_interrupts += 1,
                    IrqType::Software => stats.software_interrupts += 1,
                    IrqType::Ipi => stats.ipi_count += 1,
                }
            }

            // Call handler if present
            if let Some(handler) = descriptor.handler {
                handler(descriptor.irq, descriptor.context)
            } else {
                Err(Error::InvalidState)
            }
        } else {
            // Spurious interrupt
            let mut stats = self.stats.lock();
            stats.spurious_interrupts += 1;
            Err(Error::NotFound)
        }
    }

    /// Get IRQ statistics
    pub fn get_stats(&self) -> IrqStats {
        *self.stats.lock()
    }

    /// Get an IRQ descriptor
    pub fn get_irq(&self, irq: IrqNumber) -> Option<InterruptDescriptor> {
        let descriptors = self.descriptors.lock();
        let irq = irq as usize;

        if irq < 1024 {
            descriptors[irq].clone()
        } else {
            None
        }
    }

    /// Set CPU affinity for an IRQ
    pub fn set_affinity(&self, irq: IrqNumber, cpu_mask: u64) -> Result<()> {
        let mut descriptors = self.descriptors.lock();
        let irq = irq as usize;

        if irq >= 1024 {
            return Err(Error::InvalidArgument);
        }

        if let Some(ref mut descriptor) = descriptors[irq] {
            descriptor.cpu_affinity = cpu_mask;
            Ok(())
        } else {
            Err(Error::NotFound)
        }
    }
}

/// Global IRQ manager
static IRQ_MANAGER: IrqManager = IrqManager::new();

/// Get the global IRQ manager
pub fn get() -> &'static IrqManager {
    &IRQ_MANAGER
}

/// Initialize interrupt handling
pub fn init() -> Result<()> {
    crate::info!("Initializing interrupt handling");

    // Initialize interrupt controller
    chip::init()?;

    // Initialize exception handling
    exception::init()?;

    // Initialize interrupt handlers
    handler::init()?;

    crate::info!("Interrupt handling initialized successfully");
    Ok(())
}

/// Enable interrupts
pub fn enable_interrupts() {
    #[cfg(target_arch = "aarch64")]
    {
        unsafe {
            core::arch::asm!("msr daifclr, #2"); // Enable IRQ
        }
    }

    #[cfg(target_arch = "riscv64")]
    {
        unsafe {
            riscv::register::sie::set(riscv::register::sie::SIE::SEIE);
        }
    }

    #[cfg(target_arch = "x86_64")]
    {
        x86_64::instructions::interrupts::enable();
    }
}

/// Disable interrupts
pub fn disable_interrupts() {
    #[cfg(target_arch = "aarch64")]
    {
        unsafe {
            core::arch::asm!("msr daifset, #2"); // Disable IRQ
        }
    }

    #[cfg(target_arch = "riscv64")]
    {
        unsafe {
            riscv::register::sie::clear(riscv::register::sie::SIE::SEIE);
        }
    }

    #[cfg(target_arch = "x86_64")]
    {
        x86_64::instructions::interrupts::disable();
    }
}

/// Check if interrupts are enabled
pub fn are_interrupts_enabled() -> bool {
    #[cfg(target_arch = "aarch64")]
    {
        let daif: u64;
        unsafe {
            core::arch::asm!("mrs {}, daif", out(reg) daif);
        }
        (daif & 2) == 0
    }

    #[cfg(target_arch = "riscv64")]
    {
        riscv::register::sie::read().seie()
    }

    #[cfg(target_arch = "x86_64")]
    {
        x86_64::instructions::interrupts::are_enabled()
    }
}

/// Send an IPI to a specific CPU
pub fn send_ipi(cpu_id: usize, ipi_type: crate::core::irq::exception::IpiType) -> Result<()> {
    crate::debug!("Sending IPI {:?} to CPU {}", ipi_type, cpu_id);

    // TODO: Implement IPI sending
    match ipi_type {
        crate::core::irq::exception::IpiType::Reschedule => {
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
pub fn broadcast_ipi(ipi_type: crate::core::irq::exception::IpiType) -> Result<()> {
    crate::debug!("Broadcasting IPI {:?}", ipi_type);

    // TODO: Implement IPI broadcasting
    match ipi_type {
        crate::core::irq::exception::IpiType::Reschedule => {
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

/// Get interrupt statistics
pub fn get_stats() -> IrqStats {
    get().get_stats()
}