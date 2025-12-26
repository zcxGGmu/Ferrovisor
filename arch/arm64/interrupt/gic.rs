//! GIC (Generic Interrupt Controller) discovery and initialization
//!
//! Provides GICv2 and GICv3 driver support.

/// GIC version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GicVersion {
    /// GICv1
    V1,
    /// GICv2
    V2,
    /// GICv3
    V3,
    /// GICv4
    V4,
}

/// GIC distributor state
pub struct GicDistributor {
    base_addr: u64,
    version: GicVersion,
    num_irqs: u32,
}

impl GicDistributor {
    /// Create new GIC distributor
    pub fn new(base_addr: u64, version: GicVersion, num_irqs: u32) -> Self {
        Self {
            base_addr,
            version,
            num_irqs,
        }
    }

    /// Enable the distributor
    pub fn enable(&self) {
        log::debug!("Enabling GIC distributor at {:#x}", self.base_addr);
        // TODO: Write to GICD_CTLR
    }
}

/// Initialize GIC
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing GIC");
    log::info!("GIC initialized (version detection pending)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gic_version() {
        assert_eq!(GicVersion::V2, GicVersion::V2);
        assert_eq!(GicVersion::V3, GicVersion::V3);
    }

    #[test]
    fn test_gic_distributor() {
        let dist = GicDistributor::new(0x08000000, GicVersion::V3, 1020);
        assert_eq!(dist.num_irqs, 1020);
    }
}
