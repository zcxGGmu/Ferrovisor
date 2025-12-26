//! Virtual interrupt handling for ARM64
//!
//! Provides virtual interrupt injection, management, and EOI handling.
//! Reference: xvisor/arch/arm/cpu/arm64/cpu_vcpu_irq.c

use crate::arch::arm64::interrupt::gic::{self, gich};
use crate::arch::arm64::interrupt::vgic::{self, VgicLr, VgicLrFlags};
use crate::arch::arm64::cpu::regs::{hcr_el2_read, hcr_el2_write};

/// Virtual interrupt types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtIrqType {
    /// Reset interrupt
    Reset = 0,
    /// Undefined instruction
    Undefined = 1,
    /// Software interrupt
    Soft = 2,
    /// Prefetch abort
    PrefetchAbort = 3,
    /// Data abort
    DataAbort = 4,
    /// Hypervisor call trap
    HypCall = 5,
    /// External IRQ (via VGIC)
    External = 6,
    /// External FIQ (via VGIC)
    ExternalFiq = 7,
}

/// Virtual interrupt state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrqState {
    /// Inactive
    Inactive = 0,
    /// Pending
    Pending = 1,
    /// Active
    Active = 2,
    /// Active and pending
    ActiveAndPending = 3,
}

impl IrqState {
    /// Check if interrupt is pending
    pub fn is_pending(self) -> bool {
        matches!(self, Self::Pending | Self::ActiveAndPending)
    }

    /// Check if interrupt is active
    pub fn is_active(self) -> bool {
        matches!(self, Self::Active | Self::ActiveAndPending)
    }
}

/// Virtual interrupt descriptor
#[derive(Debug, Clone, Copy)]
pub struct VirtInterrupt {
    /// Virtual IRQ number
    pub irq: u32,
    /// Physical IRQ number (for hardware interrupts)
    pub phys_irq: Option<u32>,
    /// Priority (0-7, lower is higher priority)
    pub priority: u8,
    /// State
    pub state: IrqState,
    /// Interrupt type
    pub irq_type: VirtIrqType,
}

impl VirtInterrupt {
    /// Create a new virtual interrupt
    pub fn new(irq: u32, priority: u8, irq_type: VirtIrqType) -> Self {
        Self {
            irq,
            phys_irq: None,
            priority,
            state: IrqState::Pending,
            irq_type,
        }
    }

    /// Create with physical IRQ mapping
    pub fn with_phys_irq(irq: u32, phys_irq: u32, priority: u8, irq_type: VirtIrqType) -> Self {
        Self {
            irq,
            phys_irq: Some(phys_irq),
            priority,
            state: IrqState::Pending,
            irq_type,
        }
    }
}

/// HCR_EL2 bits for virtual interrupts
pub mod hcr_el2 {
    /// Virtual IRQ interrupt
    pub const VI: u64 = 1 << 0;
    /// Virtual FIQ interrupt
    pub const VF: u64 = 1 << 1;
    /// AMO/IMO/FMO enable
    pub const AMO: u64 = 1 << 5;
    pub const IMO: u64 = 1 << 4;
    pub const FMO: u64 = 1 << 3;
}

/// Check if VGIC is available
pub fn vgic_available() -> bool {
    vgic::get().is_some()
}

/// Inject virtual interrupt to guest via VGIC
///
/// # Arguments
/// * `vcpu_id` - Target VCPU ID
/// * `virq` - Virtual interrupt descriptor
///
/// # Returns
/// * `Ok(())` if injection succeeded
/// * `Err(&str)` if injection failed
pub fn inject_virq(vcpu_id: u32, virq: VirtInterrupt) -> Result<(), &'static str> {
    if !vgic_available() {
        // Fallback to HCR_EL2.VI/VF for simple interrupts
        return inject_hcr_virq(vcpu_id, virq);
    }

    let vgic = vgic::get_expect();

    // Build VGIC LR value
    let mut lr = VgicLr {
        virtid: virq.irq as u16,
        physid: virq.phys_irq.unwrap_or(0) as u16,
        prio: (virq.priority & 0x1F),
        flags: VgicLrFlags::STATE_PENDING,
    };

    if virq.state.is_pending() {
        lr.flags |= VgicLrFlags::STATE_PENDING;
    }
    if virq.state.is_active() {
        lr.flags |= VgicLrFlags::STATE_ACTIVE;
    }
    if virq.phys_irq.is_some() {
        lr.flags |= VgicLrFlags::HW;
    }

    // Inject via VGIC
    vgic.inject_irq(vcpu_id, virq.irq, virq.phys_irq)?;

    log::debug!("Injected virtual IRQ {} to VCPU {} (priority={})",
                virq.irq, vcpu_id, virq.priority);
    Ok(())
}

/// Inject virtual interrupt via HCR_EL2.VI/VF (fallback)
///
/// This is used when VGIC is not available or for simple virtual interrupts.
fn inject_hcr_virq(vcpu_id: u32, virq: VirtInterrupt) -> Result<(), &'static str> {
    let mut hcr = hcr_el2_read();

    match virq.irq_type {
        VirtIrqType::External => {
            hcr |= hcr_el2::VI;
            log::debug!("Set HCR_EL2.VI for VCPU {}", vcpu_id);
        }
        VirtIrqType::ExternalFiq => {
            hcr |= hcr_el2::VF;
            log::debug!("Set HCR_EL2.VF for VCPU {}", vcpu_id);
        }
        _ => {
            return Err("Interrupt type not supported without VGIC");
        }
    }

    hcr_el2_write(hcr);
    Ok(())
}

/// Deassert virtual interrupt via HCR_EL2
///
/// # Arguments
/// * `vcpu_id` - Target VCPU ID
/// * `irq_type` - Interrupt type to deassert
pub fn deassert_virq(vcpu_id: u32, irq_type: VirtIrqType) -> Result<(), &'static str> {
    if vgic_available() {
        // VGIC handles deassertion automatically
        return Ok(());
    }

    let mut hcr = hcr_el2_read();

    match irq_type {
        VirtIrqType::External => {
            hcr &= !hcr_el2::VI;
            log::debug!("Cleared HCR_EL2.VI for VCPU {}", vcpu_id);
        }
        VirtIrqType::ExternalFiq => {
            hcr &= !hcr_el2::VF;
            log::debug!("Cleared HCR_EL2.VF for VCPU {}", vcpu_id);
        }
        _ => {
            return Err("Interrupt type not supported without VGIC");
        }
    }

    hcr_el2_write(hcr);
    Ok(())
}

/// Check if any virtual interrupts are pending
///
/// # Arguments
/// * `vcpu_id` - VCPU ID to check
///
/// # Returns
/// * `true` if interrupts are pending
pub fn virq_pending(vcpu_id: u32) -> bool {
    if vgic_available() {
        // VGIC will handle pending check
        // For now, return true if any VGIC state exists
        if let Some(vgic) = vgic::get() {
            if let Some(state) = vgic.gic().hyp_interface() {
                let misr = unsafe {
                    let addr = (state.base_addr() + gich::MISR) as *const u32;
                    addr.read_volatile()
                };
                return misr != 0;
            }
        }
    }

    // Check HCR_EL2.VI/VF bits
    let hcr = hcr_el2_read();
    (hcr & (hcr_el2::VI | hcr_el2::VF)) != 0
}

/// Execute virtual interrupt handling
///
/// # Arguments
/// * `vcpu_id` - Target VCPU ID
/// * `virq` - Virtual interrupt to execute
///
/// # Returns
/// * `Ok(())` if execution succeeded
pub fn execute_virq(vcpu_id: u32, virq: VirtInterrupt) -> Result<(), &'static str> {
    match virq.irq_type {
        VirtIrqType::Undefined => {
            // Inject undefined instruction exception
            inject_undef_exception(vcpu_id)?;
        }
        VirtIrqType::PrefetchAbort => {
            // Inject prefetch abort
            inject_prefetch_abort(vcpu_id, 0)?;
        }
        VirtIrqType::DataAbort => {
            // Inject data abort (reason should be fault address)
            inject_data_abort(vcpu_id, virq.phys_irq.unwrap_or(0))?;
        }
        VirtIrqType::External | VirtIrqType::ExternalFiq => {
            // Handled by VGIC
            if !vgic_available() {
                inject_virq(vcpu_id, virq)?;
            }
        }
        _ => {
            log::warn!("Unhandled virtual IRQ type: {:?}", virq.irq_type);
        }
    }

    Ok(())
}

/// Inject undefined instruction exception
fn inject_undef_exception(vcpu_id: u32) -> Result<(), &'static str> {
    log::debug!("Injecting undefined exception to VCPU {}", vcpu_id);
    // TODO: Update VCPU registers to inject undefined exception
    // This requires access to VCPU state
    Ok(())
}

/// Inject prefetch abort exception
fn inject_prefetch_abort(vcpu_id: u32, fault_addr: u64) -> Result<(), &'static str> {
    log::debug!("Injecting prefetch abort to VCPU {} (addr={:#x})", vcpu_id, fault_addr);
    // TODO: Update VCPU registers to inject prefetch abort
    Ok(())
}

/// Inject data abort exception
fn inject_data_abort(vcpu_id: u32, fault_addr: u64) -> Result<(), &'static str> {
    log::debug!("Injecting data abort to VCPU {} (addr={:#x})", vcpu_id, fault_addr);
    // TODO: Update VCPU registers to inject data abort
    Ok(())
}

/// Get interrupt priority
///
/// # Arguments
/// * `irq_type` - Interrupt type
///
/// # Returns
/// * Priority value (0-7, lower is higher priority)
pub fn get_irq_priority(irq_type: VirtIrqType) -> u8 {
    match irq_type {
        VirtIrqType::Reset => 0,
        VirtIrqType::Undefined => 1,
        VirtIrqType::Soft => 2,
        VirtIrqType::PrefetchAbort => 2,
        VirtIrqType::DataAbort => 2,
        VirtIrqType::HypCall => 2,
        VirtIrqType::External => 2,
        VirtIrqType::ExternalFiq => 2,
    }
}

/// End of interrupt (EOI) handling
///
/// # Arguments
/// * `vcpu_id` - VCPU ID
/// * `irq` - Interrupt ID to EOI
///
/// # Returns
/// * `Ok(())` if EOI succeeded
pub fn eoi_interrupt(vcpu_id: u32, irq: u32) -> Result<(), &'static str> {
    if !vgic_available() {
        // Without VGIC, just clear HCR_EL2 bits
        if irq == 0 {
            deassert_virq(vcpu_id, VirtIrqType::External)?;
        }
        return Ok(());
    }

    let vgic = vgic::get_expect();

    // EOI will be handled by VGIC hardware
    // The list register state will be updated automatically
    log::debug!("EOI interrupt {} for VCPU {}", irq, vcpu_id);

    Ok(())
}

/// Configure interrupt delegation (HCR_EL2.AMO/IMO/FMO)
///
/// These bits control whether IRQ/FIQ/SError exceptions are routed to EL2
/// instead of EL1/EL0.
///
/// # Arguments
/// * `delegate_irq` - Delegate IRQ to EL2
/// * `delegate_fiq` - Delegate FIQ to EL2
/// * `delegate_serror` - Delegate SError to EL2
pub fn configure_interrupt_delegation(
    delegate_irq: bool,
    delegate_fiq: bool,
    delegate_serror: bool,
) {
    let mut hcr = hcr_el2_read();

    if delegate_irq {
        hcr |= hcr_el2::IMO;
    } else {
        hcr &= !hcr_el2::IMO;
    }

    if delegate_fiq {
        hcr |= hcr_el2::FMO;
    } else {
        hcr &= !hcr_el2::FMO;
    }

    if delegate_serror {
        hcr |= hcr_el2::AMO;
    } else {
        hcr &= !hcr_el2::AMO;
    }

    hcr_el2_write(hcr);

    log::debug!("Configured interrupt delegation: IRQ={}, FIQ={}, SError={}",
                delegate_irq, delegate_fiq, delegate_serror);
}

/// Initialize virtual interrupt handling
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing virtual interrupt handling");

    // Check if VGIC is available
    if vgic_available() {
        log::info!("VGIC available, using hardware virtualization");
    } else {
        log::info!("VGIC not available, using HCR_EL2.VI/VF fallback");
    }

    // Configure default interrupt delegation
    // By default, delegate IRQ to EL2 (for hypervisor handling)
    configure_interrupt_delegation(true, false, false);

    log::info!("Virtual interrupt handling initialized");
    Ok(())
}

/// Assert virtual interrupt for VCPU
///
/// This is the main entry point for interrupt injection.
pub fn assert_virq(vcpu_id: u32, virq: VirtInterrupt) -> Result<(), &'static str> {
    log::debug!("Asserting IRQ {} for VCPU {}", virq.irq, vcpu_id);
    inject_virq(vcpu_id, virq)
}

/// Deassert virtual interrupt for VCPU
pub fn deassert_irq(vcpu_id: u32, irq_type: VirtIrqType) -> Result<(), &'static str> {
    log::debug!("Deasserting IRQ type {:?} for VCPU {}", irq_type, vcpu_id);
    deassert_virq(vcpu_id, irq_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virt_irq_type() {
        assert_eq!(VirtIrqType::Reset as u32, 0);
        assert_eq!(VirtIrqType::External as u32, 6);
    }

    #[test]
    fn test_irq_state() {
        let state = IrqState::Pending;
        assert!(state.is_pending());
        assert!(!state.is_active());

        let state = IrqState::ActiveAndPending;
        assert!(state.is_pending());
        assert!(state.is_active());
    }

    #[test]
    fn test_virt_interrupt() {
        let virq = VirtInterrupt::new(32, 0xA0, VirtIrqType::External);
        assert_eq!(virq.irq, 32);
        assert_eq!(virq.priority, 0xA0);
        assert_eq!(virq.state, IrqState::Pending);
        assert!(virq.phys_irq.is_none());
    }

    #[test]
    fn test_virt_interrupt_with_phys() {
        let virq = VirtInterrupt::with_phys_irq(32, 48, 0xA0, VirtIrqType::External);
        assert_eq!(virq.irq, 32);
        assert_eq!(virq.phys_irq, Some(48));
        assert_eq!(virq.priority, 0xA0);
    }

    #[test]
    fn test_irq_priority() {
        let prio = get_irq_priority(VirtIrqType::Reset);
        assert_eq!(prio, 0);

        let prio = get_irq_priority(VirtIrqType::External);
        assert_eq!(prio, 2);
    }

    #[test]
    fn test_hcr_bits() {
        assert_eq!(hcr_el2::VI, 1 << 0);
        assert_eq!(hcr_el2::VF, 1 << 1);
        assert_eq!(hcr_el2::IMO, 1 << 4);
        assert_eq!(hcr_el2::FMO, 1 << 3);
        assert_eq!(hcr_el2::AMO, 1 << 5);
    }
}
