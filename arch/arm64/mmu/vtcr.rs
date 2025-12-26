//! VTCR_EL2 configuration for ARM64
//!
//! Provides VTCR_EL2 register configuration utilities.

/// VTCR_EL2 configuration
#[derive(Debug, Clone, Copy)]
pub struct VtcrConfig {
    pub t0sz: u8,   // Translation Table Size 0
    pub sl0: u8,    // Starting Level
    pub irgn0: u8,  // Inner Region Normal Memory
    pub orgn0: u8,  // Outer Region Normal Memory
    pub sh0: u8,    // Shareability
    pub tg0: u8,    // Translation Granule
    pub ps: u8,     // Physical Size
}

impl VtcrConfig {
    /// Default configuration for 48-bit PA/VA with 4KB pages
    pub fn default_48bit() -> Self {
        Self {
            t0sz: 16,      // 48-bit VA (64 - 16 = 48)
            sl0: 1,        // Start at level 2
            irgn0: 1,      // WB-WA Inner
            orgn0: 1,      // WB-WA Outer
            sh0: 3,        // Inner Shareable
            tg0: 0,        // 4KB granule
            ps: 2,         // 48-bit PA
        }
    }

    /// Default configuration for 40-bit PA/VA
    pub fn default_40bit() -> Self {
        Self {
            t0sz: 24,      // 40-bit VA (64 - 24 = 40)
            sl0: 1,
            irgn0: 1,
            orgn0: 1,
            sh0: 3,
            tg0: 0,
            ps: 1,         // 40-bit PA
        }
    }

    /// Encode to VTCR_EL2 value
    pub fn encode(&self) -> u64 {
        (self.t0sz as u64) << 0 |
        (self.sl0 as u64) << 6 |
        (self.irgn0 as u64) << 8 |
        (self.orgn0 as u64) << 10 |
        (self.sh0 as u64) << 12 |
        (self.tg0 as u64) << 14 |
        (self.ps as u64) << 16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vtcr_default_48bit() {
        let config = VtcrConfig::default_48bit();
        let vtcr = config.encode();

        assert_eq!(config.t0sz, 16);
        assert_eq!(config.sl0, 1);
        assert_eq!(config.ps, 2);

        // Check some bits are set correctly
        assert!((vtcr & 0x3F) == 16); // T0SZ in lower 6 bits
    }

    #[test]
    fn test_vtcr_default_40bit() {
        let config = VtcrConfig::default_40bit();
        assert_eq!(config.t0sz, 24);
        assert_eq!(config.ps, 1);
    }
}
