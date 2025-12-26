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
pub mod affinity;

// Re-export commonly used types
pub use chip::{Plic, Aplic, Imsic, AplicSourceCfg, AplicMsiConfig, ImsicGlobalConfig, ImsicLocalConfig};
pub use chip::{AplicStats, ImsicStats, create_aplic, create_imsic, init_nextgen_interrupts};
pub use msi::{MsiAddress, MsiController, MsiXController, MsiXVector, create_msi_controller, create_msix_controller};
pub use affinity::{InterruptAffinityManager, CpuMask, CpuTopology, AffinityHints, LoadBalanceStrategy};
pub use affinity::{CpuIrqStats, SystemIrqStats, init as init_affinity, get as get_affinity_manager};
pub use exception::IpiType;

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
    /// CPU affinity (legacy u64 mask for compatibility)
    pub cpu_affinity: u64,
    /// Advanced CPU affinity mask
    pub affinity_mask: Option<CpuMask>,
    /// Affinity hints for optimization
    pub affinity_hints: Option<AffinityHints>,
    /// Whether auto-affinity is enabled
    pub auto_affinity: bool,
    /// Handler function
    pub handler: Option<InterruptHandler>,
    /// Handler context
    pub context: Option<*mut core::ffi::c_void>,
    /// Last CPU that handled this interrupt
    pub last_cpu: Option<u32>,
    /// Migration count
    pub migration_count: u32,
}

impl InterruptDescriptor {
    /// Create a new interrupt descriptor
    pub fn new(irq: IrqNumber, irq_type: IrqType, priority: Priority) -> Self {
        Self {
            irq,
            irq_type,
            priority,
            cpu_affinity: u64::MAX, // All CPUs by default
            affinity_mask: None,
            affinity_hints: None,
            auto_affinity: true, // Enable auto-affinity by default
            handler: None,
            context: None,
            last_cpu: None,
            migration_count: 0,
        }
    }

    /// Set advanced CPU affinity mask
    pub fn set_affinity_mask(&mut self, mask: CpuMask) {
        self.affinity_mask = Some(mask);
        self.cpu_affinity = mask.bits();
    }

    /// Set affinity hints
    pub fn set_affinity_hints(&mut self, hints: AffinityHints) {
        self.affinity_hints = Some(hints);
    }

    /// Enable/disable auto-affinity
    pub fn set_auto_affinity(&mut self, enabled: bool) {
        self.auto_affinity = enabled;
    }

    /// Get current affinity mask (fallback to legacy field)
    pub fn get_affinity_mask(&self) -> CpuMask {
        self.affinity_mask.unwrap_or_else(|| CpuMask::from_bits(self.cpu_affinity))
    }

    /// Update last CPU and check for migration
    pub fn update_cpu(&mut self, cpu: u32) -> bool {
        let migrated = self.last_cpu.map_or(true, |last| last != cpu);
        if migrated {
            self.migration_count += 1;
        }
        self.last_cpu = Some(cpu);
        migrated
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

            // Update affinity manager if available
            if let Some(affinity_mgr) = crate::core::irq::affinity::get() {
                let mask = CpuMask::from_bits(cpu_mask);
                affinity_mgr.set_irq_affinity(irq, mask, false)?;
            }

            Ok(())
        } else {
            Err(Error::NotFound)
        }
    }

    /// Set advanced CPU affinity for an IRQ
    pub fn set_advanced_affinity(&self, irq: IrqNumber, mask: CpuMask) -> Result<()> {
        let mut descriptors = self.descriptors.lock();
        let irq = irq as usize;

        if irq >= 1024 {
            return Err(Error::InvalidArgument);
        }

        if let Some(ref mut descriptor) = descriptors[irq] {
            descriptor.set_affinity_mask(mask);

            // Update affinity manager if available
            if let Some(affinity_mgr) = crate::core::irq::affinity::get() {
                affinity_mgr.set_irq_affinity(irq, mask, false)?;
            }

            Ok(())
        } else {
            Err(Error::NotFound)
        }
    }

    /// Set affinity hints for an IRQ
    pub fn set_affinity_hints(&self, irq: IrqNumber, hints: AffinityHints) -> Result<()> {
        let mut descriptors = self.descriptors.lock();
        let irq = irq as usize;

        if irq >= 1024 {
            return Err(Error::InvalidArgument);
        }

        if let Some(ref mut descriptor) = descriptors[irq] {
            descriptor.set_affinity_hints(hints);
            Ok(())
        } else {
            Err(Error::NotFound)
        }
    }

    /// Enable/disable auto-affinity for an IRQ
    pub fn set_auto_affinity(&self, irq: IrqNumber, enabled: bool) -> Result<()> {
        let mut descriptors = self.descriptors.lock();
        let irq = irq as usize;

        if irq >= 1024 {
            return Err(Error::InvalidArgument);
        }

        if let Some(ref mut descriptor) = descriptors[irq] {
            descriptor.set_auto_affinity(enabled);
            Ok(())
        } else {
            Err(Error::NotFound)
        }
    }

    /// Get optimal affinity for an IRQ
    pub fn get_optimal_affinity(&self, irq: IrqNumber) -> Option<CpuMask> {
        let descriptors = self.descriptors.lock();
        let irq = irq as usize;

        if irq >= 1024 {
            return None;
        }

        if let Some(ref descriptor) = descriptors[irq] {
            if let Some(affinity_mgr) = crate::core::irq::affinity::get() {
                Some(affinity_mgr.calculate_optimal_affinity(descriptor))
            } else {
                Some(descriptor.get_affinity_mask())
            }
        } else {
            None
        }
    }

    /// Handle an interrupt with affinity management
    pub fn handle_irq_with_affinity(&self, irq: IrqNumber) -> Result<(u32, u32)> {
        let start_time = crate::utils::time::timestamp_ns();

        // Get current CPU
        let current_cpu = crate::arch::cpu::get_current_cpu_id().unwrap_or(0);

        // Get descriptor and handle interrupt
        let result = self.handle_irq(irq);

        let end_time = crate::utils::time::timestamp_ns();
        let processing_time = (end_time - start_time) as u32;

        // Update affinity statistics
        if let Some(affinity_mgr) = crate::core::irq::affinity::get() {
            let descriptors = self.descriptors.lock();
            if (irq as usize) < 1024 {
                if let Some(ref descriptor) = descriptors[irq as usize] {
                    // Record statistics
                    affinity_mgr.record_interrupt(current_cpu, irq, descriptor, processing_time);

                    // Update last CPU
                    drop(descriptors); // Release lock before modifying
                    let mut descriptors = self.descriptors.lock();
                    if let Some(ref mut descriptor) = descriptors[irq as usize] {
                        descriptor.update_cpu(current_cpu);
                    }
                }
            }
        }

        Ok((current_cpu, processing_time))
    }

    /// Balance all interrupts
    pub fn balance_interrupts(&self) -> Result<usize> {
        if let Some(affinity_mgr) = crate::core::irq::affinity::get() {
            let descriptors = self.descriptors.lock();
            let descriptor_vec: Vec<InterruptDescriptor> = descriptors.iter()
                .filter_map(|d| d.clone())
                .collect();

            affinity_mgr.balance_interrupts(&descriptor_vec)
        } else {
            Ok(0)
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

    // Initialize interrupt affinity manager first
    let num_cpus = crate::arch::cpu::get_cpu_count().unwrap_or(4);
    affinity::init(num_cpus)?;

    // Initialize interrupt controller
    chip::init()?;

    // Initialize exception handling
    exception::init()?;

    // Initialize interrupt handlers
    handler::init()?;

    // Configure load balancing strategy
    if let Some(affinity_mgr) = affinity::get() {
        // Use package-aware strategy by default for better cache locality
        affinity_mgr.set_strategy(LoadBalanceStrategy::PackageAware);
    }

    crate::info!("Interrupt handling initialized successfully");
    Ok(())
}

/// Initialize interrupt handling with custom CPU count
pub fn init_with_cpus(num_cpus: u32) -> Result<()> {
    crate::info!("Initializing interrupt handling with {} CPUs", num_cpus);

    // Initialize interrupt affinity manager with specified CPU count
    affinity::init(num_cpus)?;

    // Initialize interrupt controller
    chip::init()?;

    // Initialize exception handling
    exception::init()?;

    // Initialize interrupt handlers
    handler::init()?;

    // Configure load balancing strategy
    if let Some(affinity_mgr) = affinity::get() {
        affinity_mgr.set_strategy(LoadBalanceStrategy::PackageAware);
    }

    crate::info!("Interrupt handling initialized successfully");
    Ok(())
}

/// Perform periodic interrupt affinity balancing
pub fn perform_affinity_balancing() -> Result<usize> {
    get().balance_interrupts()
}

/// Get interrupt affinity statistics
pub fn get_affinity_stats() -> Option<SystemIrqStats> {
    affinity::get().map(|mgr| mgr.get_system_stats())
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
        unsafe { core::arch::asm!("sti") };
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
            core::arch::asm!("csrc sie, {}", in(reg) 1u64);
        }
    }

    #[cfg(target_arch = "x86_64")]
    {
        unsafe { core::arch::asm!("cli") };
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
        let sie: u64;
        unsafe {
            core::arch::asm!("csrr {}, sie", out(reg) sie);
        }
        (sie & 1) != 0
    }

    #[cfg(target_arch = "x86_64")]
    {
        let rflags: u64;
        unsafe {
            core::arch::asm!("pushfq; pop {}", out(reg) rflags);
        }
        (rflags & (1 << 9)) != 0
    }
}

/// Send an IPI to a specific CPU
pub fn send_ipi(cpu_id: usize, ipi_type: IpiType) -> Result<()> {
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