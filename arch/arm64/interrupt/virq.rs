//! Virtual interrupt handling
//!
//! Provides virtual interrupt injection and management.

/// Virtual interrupt
#[derive(Debug, Clone, Copy)]
pub struct VirtInterrupt {
    pub irq: u32,
    pub priority: u8,
    pub state: IrqState,
}

/// Interrupt state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrqState {
    Inactive,
    Pending,
    Active,
    ActiveAndPending,
}

/// Inject a virtual interrupt to guest
pub fn inject(virq: VirtInterrupt) -> Result<(), &'static str> {
    log::debug!("Injecting virtual IRQ {}", virq.irq);
    // TODO: Set VGIC LR register
    Ok(())
}

/// Initialize virtual interrupt handling
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing virtual interrupt handling");
    log::info!("Virtual interrupt handling initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virt_interrupt() {
        let virq = VirtInterrupt {
            irq: 32,
            priority: 0xA0,
            state: IrqState::Pending,
        };
        assert_eq!(virq.irq, 32);
    }

    #[test]
    fn test_irq_state() {
        assert_eq!(IrqState::Inactive, IrqState::Inactive);
        assert_eq!(IrqState::Pending, IrqState::Pending);
    }
}
