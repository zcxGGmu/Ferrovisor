//! VTCR_EL2 configuration for ARM64
//!
//! Provides VTCR_EL2 register configuration utilities.
//! Reference: ARM DDI 0487I.a, D13.2.130 VTCR_EL2

/// VTCR_EL2 register bit definitions
pub mod vtcr {
    /// T0SZ field bits [5:0] - Translation Table Size 0
    /// Size of the translation table for region 0
    pub const T0SZ_MASK: u64 = 0x3F;

    /// SL0 field bits [7:6] - Starting Level for region 0
    /// Level at which translation starts
    pub const SL0_MASK: u64 = 0x3 << 6;
    pub const SL0_SHIFT: u64 = 6;

    /// IRGN0 field bits [9:8] - Inner Region Normal Memory attributes
    pub const IRGN0_MASK: u64 = 0x3 << 8;
    pub const IRGN0_SHIFT: u64 = 8;
    /// IRGN0 values
    pub const IRGN0_WB_WA: u64 = 0x1 << 8;  // Write-Back Write-Allocate
    pub const IRGN0_WT: u64 = 0x2 << 8;     // Write-Through
    pub const IRGN0_NC: u64 = 0x0 << 8;     // Non-Cacheable

    /// ORGN0 field bits [11:10] - Outer Region Normal Memory attributes
    pub const ORGN0_MASK: u64 = 0x3 << 10;
    pub const ORGN0_SHIFT: u64 = 10;
    /// ORGN0 values
    pub const ORGN0_WB_WA: u64 = 0x1 << 10; // Write-Back Write-Allocate
    pub const ORGN0_WT: u64 = 0x2 << 10;    // Write-Through
    pub const ORGN0_NC: u64 = 0x0 << 10;    // Non-Cacheable

    /// SH0 field bits [13:12] - Shareability for region 0
    pub const SH0_MASK: u64 = 0x3 << 12;
    pub const SH0_SHIFT: u64 = 12;
    /// Shareability values
    pub const SH0_NONE: u64 = 0x0 << 12;         // Non-shareable
    pub const SH0_OUTER: u64 = 0x2 << 12;        // Outer shareable
    pub const SH0_INNER: u64 = 0x3 << 12;        // Inner shareable

    /// TG0 field bits [15:14] - Translation Granule for region 0
    pub const TG0_MASK: u64 = 0x3 << 14;
    pub const TG0_SHIFT: u64 = 14;
    /// Granule values
    pub const TG0_4KB: u64 = 0x0 << 14;   // 4KB granule
    pub const TG0_64KB: u64 = 0x1 << 14;  // 64KB granule
    pub const TG0_16KB: u64 = 0x2 << 14;  // 16KB granule

    /// PS field bits [18:16] - Physical Address Size
    pub const PS_MASK: u64 = 0x7 << 16;
    pub const PS_SHIFT: u64 = 16;
    /// PS values - PA size
    pub const PS_32BIT: u64 = 0x0 << 16;  // 32-bit PA (4GB)
    pub const PS_36BIT: u64 = 0x1 << 16;  // 36-bit PA (64GB)
    pub const PS_40BIT: u64 = 0x2 << 16;  // 40-bit PA (1TB)
    pub const PS_42BIT: u64 = 0x3 << 16;  // 42-bit PA (4TB)
    pub const PS_44BIT: u64 = 0x4 << 16;  // 44-bit PA (16TB)
    pub const PS_48BIT: u64 = 0x5 << 16;  // 48-bit PA (256TB)
    pub const PS_52BIT: u64 = 0x6 << 16;  // 52-bit PA (4PB)

    /// VS bit [19] - Virtualization Stage 2
    pub const VS_SHIFT: u64 = 19;

    /// RES1 bits [23:20] - Reserved as 1
    pub const RES1_MASK: u64 = 0xF << 20;

    /// TG1 field bits [25:24] - Translation Granule for region 1
    pub const TG1_MASK: u64 = 0x3 << 24;
    pub const TG1_SHIFT: u64 = 24;

    /// PS1 bit [27] - Physical Address Size for region 1
    pub const PS1_SHIFT: u64 = 27;

    /// IRGN1 field bits [29:28] - Inner Region attributes for region 1
    pub const IRGN1_MASK: u64 = 0x3 << 28;
    pub const IRGN1_SHIFT: u64 = 28;

    /// ORGN1 field bits [31:30] - Outer Region attributes for region 1
    pub const ORGN1_MASK: u64 = 0x3 << 30;
    pub const ORGN1_SHIFT: u64 = 30;

    /// SH1 field bits [33:32] - Shareability for region 1
    pub const SH1_MASK: u64 = 0x3 << 32;
    pub const SH1_SHIFT: u64 = 32;

    /// EAE bit [31] - Extended Address Enable
    pub const EAE_SHIFT: u64 = 31;

    /// HHD bit [34] - Hierarchical Hardware Dirty
    pub const HD_SHIFT: u64 = 34;

    /// HA bit [35] - Hardware Access flag
    pub const HA_SHIFT: u64 = 35;

    /// VSW bit [36] - VMSW enable
    pub const VSW_SHIFT: u64 = 36;

    /// TBI bits [39:38] - Top Byte
    pub const TBI_MASK: u64 = 0x3 << 38;
    pub const TBI_SHIFT: u64 = 38;
}

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
    pub vs: bool,   // Virtualization Stage 2
    pub hd: bool,   // Hierarchical Hardware Dirty
    pub ha: bool,   // Hardware Access flag
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
            ps: 5,         // 48-bit PA
            vs: false,
            hd: false,
            ha: false,
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
            ps: 2,         // 40-bit PA
            vs: false,
            hd: false,
            ha: false,
        }
    }

    /// Default configuration for 44-bit PA/VA
    pub fn default_44bit() -> Self {
        Self {
            t0sz: 20,      // 44-bit VA (64 - 20 = 44)
            sl0: 1,
            irgn0: 1,
            orgn0: 1,
            sh0: 3,
            tg0: 0,
            ps: 4,         // 44-bit PA
            vs: false,
            hd: false,
            ha: false,
        }
    }

    /// Encode to VTCR_EL2 value
    pub fn encode(&self) -> u64 {
        let mut value = 0u64;

        // T0SZ in bits [5:0]
        value |= (self.t0sz as u64) & vtcr::T0SZ_MASK;

        // SL0 in bits [7:6]
        value |= ((self.sl0 as u64) << vtcr::SL0_SHIFT) & vtcr::SL0_MASK;

        // IRGN0 in bits [9:8]
        value |= ((self.irgn0 as u64) << vtcr::IRGN0_SHIFT) & vtcr::IRGN0_MASK;

        // ORGN0 in bits [11:10]
        value |= ((self.orgn0 as u64) << vtcr::ORGN0_SHIFT) & vtcr::ORGN0_MASK;

        // SH0 in bits [13:12]
        value |= ((self.sh0 as u64) << vtcr::SH0_SHIFT) & vtcr::SH0_MASK;

        // TG0 in bits [15:14]
        value |= ((self.tg0 as u64) << vtcr::TG0_SHIFT) & vtcr::TG0_MASK;

        // PS in bits [18:16]
        value |= ((self.ps as u64) << vtcr::PS_SHIFT) & vtcr::PS_MASK;

        // VS in bit [19]
        if self.vs {
            value |= 1u64 << vtcr::VS_SHIFT;
        }

        // RES1 bits [23:20]
        value |= vtcr::RES1_MASK;

        // HD in bit [34]
        if self.hd {
            value |= 1u64 << vtcr::HD_SHIFT;
        }

        // HA in bit [35]
        if self.ha {
            value |= 1u64 << vtcr::HA_SHIFT;
        }

        value
    }

    /// Decode from VTCR_EL2 value
    pub fn decode(value: u64) -> Self {
        Self {
            t0sz: (value & vtcr::T0SZ_MASK) as u8,
            sl0: ((value & vtcr::SL0_MASK) >> vtcr::SL0_SHIFT) as u8,
            irgn0: ((value & vtcr::IRGN0_MASK) >> vtcr::IRGN0_SHIFT) as u8,
            orgn0: ((value & vtcr::ORGN0_MASK) >> vtcr::ORGN0_SHIFT) as u8,
            sh0: ((value & vtcr::SH0_MASK) >> vtcr::SH0_SHIFT) as u8,
            tg0: ((value & vtcr::TG0_MASK) >> vtcr::TG0_SHIFT) as u8,
            ps: ((value & vtcr::PS_MASK) >> vtcr::PS_SHIFT) as u8,
            vs: (value & (1u64 << vtcr::VS_SHIFT)) != 0,
            hd: (value & (1u64 << vtcr::HD_SHIFT)) != 0,
            ha: (value & (1u64 << vtcr::HA_SHIFT)) != 0,
        }
    }

    /// Get the starting level for page table walk
    pub fn start_level(&self) -> u8 {
        self.sl0
    }

    /// Get the VA size based on T0SZ
    pub fn va_size(&self) -> u8 {
        64 - self.t0sz
    }

    /// Get the PA size based on PS field
    pub fn pa_size(&self) -> u8 {
        match self.ps {
            0 => 32,
            1 => 36,
            2 => 40,
            3 => 42,
            4 => 44,
            5 => 48,
            6 => 52,
            _ => 48,
        }
    }
}

/// Get current VTCR_EL2 value
///
/// # Safety
/// Must be called at EL2
pub unsafe fn read_vtcr_el2() -> u64 {
    let value: u64;
    core::arch::asm!("mrs {}, vtcr_el2", out(reg) value);
    value
}

/// Set VTCR_EL2 value
///
/// # Safety
/// Must be called at EL2
pub unsafe fn write_vtcr_el2(value: u64) {
    core::arch::asm!("msr vtcr_el2, {}", in(reg) value);
    // Ensure the change takes effect
    core::arch::asm!("isb", options(nostack, nomem));
}

/// Initialize VTCR_EL2 with default 48-bit configuration
///
/// # Safety
/// Must be called at EL2
pub unsafe fn init_default_48bit() {
    let config = VtcrConfig::default_48bit();
    write_vtcr_el2(config.encode());
}

/// Initialize VTCR_EL2 with specified configuration
///
/// # Safety
/// Must be called at EL2
pub unsafe fn init(config: VtcrConfig) {
    write_vtcr_el2(config.encode());
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
        assert_eq!(config.ps, 5);
        assert_eq!(config.va_size(), 48);
        assert_eq!(config.pa_size(), 48);

        // Check some bits are set correctly
        assert!((vtcr & 0x3F) == 16); // T0SZ in lower 6 bits
        assert!((vtcr & vtcr::RES1_MASK) != 0); // RES1 should be set
    }

    #[test]
    fn test_vtcr_default_40bit() {
        let config = VtcrConfig::default_40bit();
        assert_eq!(config.t0sz, 24);
        assert_eq!(config.ps, 2);
        assert_eq!(config.va_size(), 40);
        assert_eq!(config.pa_size(), 40);
    }

    #[test]
    fn test_vtcr_encode_decode() {
        let config = VtcrConfig::default_48bit();
        let encoded = config.encode();
        let decoded = VtcrConfig::decode(encoded);

        assert_eq!(decoded.t0sz, config.t0sz);
        assert_eq!(decoded.sl0, config.sl0);
        assert_eq!(decoded.irgn0, config.irgn0);
        assert_eq!(decoded.orgn0, config.orgn0);
        assert_eq!(decoded.sh0, config.sh0);
        assert_eq!(decoded.tg0, config.tg0);
        assert_eq!(decoded.ps, config.ps);
        assert_eq!(decoded.vs, config.vs);
    }

    #[test]
    fn test_vtcr_start_level() {
        let config = VtcrConfig::default_48bit();
        assert_eq!(config.start_level(), 1);
    }

    #[test]
    fn test_vtcr_bit_definitions() {
        // Check that bit masks are correct
        assert_eq!(vtcr::T0SZ_MASK, 0x3F);
        assert_eq!(vtcr::SL0_MASK, 0x3 << 6);
        assert_eq!(vtcr::IRGN0_MASK, 0x3 << 8);
        assert_eq!(vtcr::ORGN0_MASK, 0x3 << 10);
        assert_eq!(vtcr::SH0_MASK, 0x3 << 12);
        assert_eq!(vtcr::TG0_MASK, 0x3 << 14);
        assert_eq!(vtcr::PS_MASK, 0x7 << 16);
    }
}
