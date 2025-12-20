//! RISC-V Interrupt Module
//!
//! This module provides interrupt handling functionality including:
//! - Exception handling
//! - Interrupt controller support
/// - Interrupt routing and delegation
/// - External interrupt handling

pub mod handler;
pub mod controller;

pub use handler::*;
pub use controller::*;

use crate::arch::riscv64::*;

/// Global interrupt manager
static mut INTERRUPT_MANAGER: Option<InterruptManager> = None;

/// Initialize interrupt subsystem
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing RISC-V interrupt subsystem");

    // Initialize trap handling first
    handler::init()?;

    // Create interrupt controller based on platform
    let plic = Box::new(PlicController::new(0x0C000000, 32, 4));
    let mut manager = InterruptManager::new(plic);

    // Add ACLINT for local interrupts
    let aclint = Box::new(AclintController::new(0x02000000, 4));
    manager.add_secondary(aclint);

    // Initialize all controllers
    manager.init()?;

    // Store global manager
    unsafe {
        INTERRUPT_MANAGER = Some(manager);
    }

    // Initialize interrupt delegation
    init_delegation()?;

    // Enable machine-mode interrupts
    enable_machine_interrupts();

    log::info!("RISC-V interrupt subsystem initialized successfully");
    Ok(())
}

/// Get the global interrupt manager
pub fn get_interrupt_manager() -> Option<&'static InterruptManager> {
    unsafe { INTERRUPT_MANAGER.as_ref() }
}

/// Get mutable reference to global interrupt manager
pub fn get_interrupt_manager_mut() -> Option<&'static mut InterruptManager> {
    unsafe { INTERRUPT_MANAGER.as_mut() }
}

/// Initialize interrupt delegation from machine mode to supervisor mode
fn init_delegation() -> Result<(), &'static str> {
    log::debug!("Initializing interrupt delegation");

    // Delegate standard exceptions to supervisor mode
    let mut medeleg = 0usize;
    medeleg |= 1 << ExceptionCode::InstructionMisaligned as usize;
    medeleg |= 1 << ExceptionCode::InstructionAccessFault as usize;
    medeleg |= 1 << ExceptionCode::IllegalInstruction as usize;
    medeleg |= 1 << ExceptionCode::Breakpoint as usize;
    medeleg |= 1 << ExceptionCode::LoadMisaligned as usize;
    medeleg |= 1 << ExceptionCode::LoadAccessFault as usize;
    medeleg |= 1 << ExceptionCode::StoreMisaligned as usize;
    medeleg |= 1 << ExceptionCode::StoreAccessFault as usize;
    medeleg |= 1 << ExceptionCode::ECallFromUMode as usize;
    medeleg |= 1 << ExceptionCode::InstructionPageFault as usize;
    medeleg |= 1 << ExceptionCode::LoadPageFault as usize;
    medeleg |= 1 << ExceptionCode::StorePageFault as usize;

    crate::arch::riscv64::cpu::csr::MEDELEG::write(medeleg);

    // Delegate standard interrupts to supervisor mode
    let mut mideleg = 0usize;
    mideleg |= 1 << InterruptCause::SupervisorSoftware as usize;
    mideleg |= 1 << InterruptCause::SupervisorTimer as usize;
    mideleg |= 1 << InterruptCause::SupervisorExternal as usize;

    crate::arch::riscv64::cpu::csr::MIDELEG::write(mideleg);

    // Set counter-enable to allow supervisor mode to read counters
    let mut mcounteren = 0usize;
    mcounteren |= 1 << 0; // Cycle counter
    mcounteren |= 1 << 1; // Time counter
    mcounteren |= 1 << 2; // Instret counter

    crate::arch::riscv64::cpu::csr::MCOUNTEREN::write(mcounteren);

    log::debug!("Interrupt delegation configured");
    Ok(())
}

/// Enable machine-mode external interrupts
fn enable_machine_interrupts() {
    // Enable machine-mode interrupts
    let mut mstatus = crate::arch::riscv64::cpu::csr::MSTATUS::read();
    mstatus |= 1 << 3; // MIE bit
    crate::arch::riscv64::cpu::csr::MSTATUS::write(mstatus);

    // Enable machine-mode external interrupts
    crate::arch::riscv64::cpu::csr::MIE::set(
        crate::arch::riscv64::cpu::csr::Mie::MEIE |
        crate::arch::riscv64::cpu::csr::Mie::MTIE |
        crate::arch::riscv64::cpu::csr::Mie::MSIE
    );
}

/// Enable external interrupts (supervisor mode)
pub fn enable_external_interrupts() {
    let mut sstatus = crate::arch::riscv64::cpu::csr::SSTATUS::read();
    sstatus |= 1 << 1; // SIE bit
    crate::arch::riscv64::cpu::csr::SSTATUS::write(sstatus);

    // Enable supervisor external interrupts
    crate::arch::riscv64::cpu::csr::SIE::set(
        crate::arch::riscv64::cpu::csr::Sie::SEIE |
        crate::arch::riscv64::cpu::csr::Sie::STIE |
        crate::arch::riscv64::cpu::csr::Sie::SSIE
    );
}

/// Disable external interrupts (supervisor mode)
pub fn disable_external_interrupts() {
    let mut sstatus = crate::arch::riscv64::cpu::csr::SSTATUS::read();
    sstatus &= !(1 << 1); // SIE bit
    crate::arch::riscv64::cpu::csr::SSTATUS::write(sstatus);
}

/// Send IPI to a target CPU
pub fn send_ipi(target_cpu: usize) -> Result<(), &'static str> {
    let manager = get_interrupt_manager_mut().ok_or("Interrupt manager not initialized")?;

    // Find ACLINT controller for IPI
    for controller in &mut manager.secondary {
        if let Some(aclint) = controller.as_any().downcast_ref::<AclintController>() {
            return aclint.send_ipi(target_cpu);
        }
    }

    Err("No ACLINT controller available for IPI")
}

/// Configure timer interrupt
pub fn configure_timer(deadline: u64) -> Result<(), &'static str> {
    let manager = get_interrupt_manager_mut().ok_or("Interrupt manager not initialized")?;

    // Find ACLINT controller for timer
    for controller in &mut manager.secondary {
        if let Some(aclint) = controller.as_any().downcast_ref::<AclintController>() {
            let cpu = crate::arch::riscv64::cpu::current_cpu_id();
            return aclint.set_timer(cpu, deadline);
        }
    }

    Err("No ACLINT controller available for timer")
}

/// Enable specific interrupt
pub fn enable_interrupt(irq: u32) -> Result<(), &'static str> {
    let manager = get_interrupt_manager_mut().ok_or("Interrupt manager not initialized")?;
    let cpu = crate::arch::riscv64::cpu::current_cpu_id();
    manager.enable(irq, cpu)
}

/// Disable specific interrupt
pub fn disable_interrupt(irq: u32) -> Result<(), &'static str> {
    let manager = get_interrupt_manager_mut().ok_or("Interrupt manager not initialized")?;
    let cpu = crate::arch::riscv64::cpu::current_cpu_id();
    manager.disable(irq, cpu)
}

/// Set interrupt priority
pub fn set_interrupt_priority(irq: u32, priority: u8) -> Result<(), &'static str> {
    let manager = get_interrupt_manager_mut().ok_or("Interrupt manager not initialized")?;
    manager.set_priority(irq, priority)
}

/// Claim an interrupt
pub fn claim_interrupt() -> Result<u32, &'static str> {
    let manager = get_interrupt_manager_mut().ok_or("Interrupt manager not initialized")?;
    let cpu = crate::arch::riscv64::cpu::current_cpu_id();
    manager.claim(cpu)
}

/// Complete an interrupt
pub fn complete_interrupt(irq: u32) -> Result<(), &'static str> {
    let manager = get_interrupt_manager_mut().ok_or("Interrupt manager not initialized")?;
    let cpu = crate::arch::riscv64::cpu::current_cpu_id();
    manager.complete(irq, cpu)
}

/// Get interrupt statistics
pub fn get_interrupt_stats() -> InterruptStats {
    // TODO: Implement interrupt statistics collection
    InterruptStats {
        total_interrupts: 0,
        interrupts_per_type: [0; 32],
        max_latency_ns: 0,
        avg_latency_ns: 0,
    }
}

/// Interrupt statistics
#[derive(Debug, Clone)]
pub struct InterruptStats {
    pub total_interrupts: u64,
    pub interrupts_per_type: [u64; 32],
    pub max_latency_ns: u64,
    pub avg_latency_ns: u64,
}

/// Common interrupt handlers
pub mod handlers {
    use super::*;

    /// Timer interrupt handler
    pub extern "C" fn timer_handler(_context: &mut TrapContext) -> Result<(), &'static str> {
        log::debug!("Timer interrupt");

        // Clear timer interrupt
        // This is typically done by reading/writing timer compare registers

        // Update scheduler if needed

        Ok(())
    }

    /// IPI interrupt handler
    pub extern "C" fn ipi_handler(_context: &mut TrapContext) -> Result<(), &'static str> {
        log::debug!("IPI interrupt received");

        // Clear IPI
        let cpu = crate::arch::riscv64::cpu::current_cpu_id();
        let manager = get_interrupt_manager_mut().ok_or("Interrupt manager not initialized")?;

        for controller in &mut manager.secondary {
            if let Some(aclint) = controller.as_any().downcast_mut::<AclintController>() {
                aclint.clear_ipi(cpu)?;
                break;
            }
        }

        // Handle IPI (e.g., TLB shootdown, scheduler tick)

        Ok(())
    }

    /// External interrupt handler
    pub extern "C" fn external_handler(context: &mut TrapContext) -> Result<(), &'static str> {
        log::debug!("External interrupt");

        // Claim the interrupt
        let irq = claim_interrupt()?;

        // Call platform-specific handler based on IRQ
        handle_platform_irq(irq, context)?;

        // Complete the interrupt
        complete_interrupt(irq)?;

        Ok(())
    }

    /// Handle platform-specific IRQ
    fn handle_platform_irq(irq: u32, _context: &mut TrapContext) -> Result<(), &'static str> {
        match irq {
            10 => {
                // UART interrupt
                log::debug!("UART interrupt");
                // TODO: Handle UART interrupt
            }
            11 => {
                // Network interrupt
                log::debug!("Network interrupt");
                // TODO: Handle network interrupt
            }
            _ => {
                log::warn!("Unknown platform IRQ: {}", irq);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interrupt_stats() {
        let stats = get_interrupt_stats();
        assert_eq!(stats.total_interrupts, 0);
        assert!(stats.interrupts_per_type.iter().all(|&x| x == 0));
    }

    #[test]
    fn test_interrupt_delegation() {
        // Test delegation configuration
        init_delegation().unwrap();

        // Check that exceptions are delegated
        let medeleg = crate::arch::riscv64::cpu::csr::MEDELEG::read();
        assert!(medeleg & (1 << ExceptionCode::IllegalInstruction as usize) != 0);

        // Check that interrupts are delegated
        let mideleg = crate::arch::riscv64::cpu::csr::MIDELEG::read();
        assert!(mideleg & (1 << InterruptCause::SupervisorExternal as usize) != 0);
    }

    #[test]
    fn test_interrupt_control() {
        // Test that we can enable/disable interrupts
        // Note: This test would need to run in a proper RISC-V environment
        enable_external_interrupts();
        disable_external_interrupts();
    }
}