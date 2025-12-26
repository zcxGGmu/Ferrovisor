//! Memory attributes for ARM64
//!
//! Provides MAIR_EL2 and memory attribute configuration.
//! Reference: ARM DDI 0487I.a, D13.2.117 MAIR_EL2

/// MAIR_EL2 register bit definitions
pub mod mair {
    /// Each attribute is 8 bits
    pub const ATTR_SHIFT: usize = 8;

    /// Attr0: Device-nGnRnE
    /// Device memory, non-Gathering, non-Reordering, No Early write acknowledgement
    pub const ATTR0_DEVICE_nGnRnE: u64 = 0x00;

    /// Attr0: Device-nGnRE
    /// Device memory, non-Gathering, non-Reordering, Early write acknowledgement
    pub const ATTR0_DEVICE_nGnRE: u64 = 0x04;

    /// Attr0: Device-GRE
    /// Device memory, Gathering, Reordering, Early write acknowledgement
    pub const ATTR0_DEVICE_GRE: u64 = 0x0C;

    /// Attr1: Normal memory, Outer and Inner Write-Back Write-Allocate
    pub const ATTR1_NORMAL_WBWA: u64 = 0xFF;

    /// Attr1: Normal memory, Outer and Inner Write-Through
    pub const ATTR1_NORMAL_WT: u64 = 0xBB;

    /// Attr1: Normal memory, Outer and Inner Non-Cacheable
    pub const ATTR1_NORMAL_NC: u64 = 0x44;

    /// Attr2 index bits [15:8]
    pub const ATTR2_MASK: u64 = 0xFF << 16;
    pub const ATTR2_SHIFT: u64 = 16;

    /// Attr3 index bits [23:16]
    pub const ATTR3_MASK: u64 = 0xFF << 24;
    pub const ATTR3_SHIFT: u64 = 24;

    /// Attr4 index bits [31:24]
    pub const ATTR4_MASK: u64 = 0xFF << 32;
    pub const ATTR4_SHIFT: u64 = 32;

    /// Attr5 index bits [39:32]
    pub const ATTR5_MASK: u64 = 0xFF << 40;
    pub const ATTR5_SHIFT: u64 = 40;

    /// Attr6 index bits [47:40]
    pub const ATTR6_MASK: u64 = 0xFF << 48;
    pub const ATTR6_SHIFT: u64 = 48;

    /// Attr7 index bits [55:48]
    pub const ATTR7_MASK: u64 = 0xFF << 56;
    pub const ATTR7_SHIFT: u64 = 56;
}

/// Memory types for Stage-2 translation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryType {
    /// Device memory (nGnRnE - non-Gathering, non-Reordering, No Early write ack)
    Device,
    /// Device memory with early write ack (nGnRE)
    DeviceRE,
    /// Device memory with gathering and reordering (GRE)
    DeviceGRE,
    /// Normal memory, Write-Back Write-Allocate
    NormalWBWA,
    /// Normal memory, Write-Through
    NormalWT,
    /// Normal memory, Non-Cacheable
    NormalNC,
}

/// Shareability attributes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shareability {
    /// Non-shareable
    None,
    /// Outer shareable
    Outer,
    /// Inner shareable
    Inner,
}

impl Shareability {
    /// Convert to MAIR attribute value
    pub fn to_attr_bits(self) -> u8 {
        match self {
            Shareability::None => 0b00,
            Shareability::Outer => 0b10,
            Shareability::Inner => 0b11,
        }
    }
}

/// Memory attribute descriptor
#[derive(Debug, Clone, Copy)]
pub struct MemoryAttr {
    pub mem_type: MemoryType,
    pub shareability: Shareability,
}

impl MemoryAttr {
    /// Create device memory attribute (nGnRnE)
    pub fn device() -> Self {
        Self {
            mem_type: MemoryType::Device,
            shareability: Shareability::None,
        }
    }

    /// Create normal memory attribute (Write-Back Write-Allocate)
    pub fn normal_wb_wa() -> Self {
        Self {
            mem_type: MemoryType::NormalWBWA,
            shareability: Shareability::Inner,
        }
    }

    /// Create normal memory attribute (Write-Through)
    pub fn normal_wt() -> Self {
        Self {
            mem_type: MemoryType::NormalWT,
            shareability: Shareability::Inner,
        }
    }

    /// Create normal memory attribute (Non-Cacheable)
    pub fn normal_nc() -> Self {
        Self {
            mem_type: MemoryType::NormalNC,
            shareability: Shareability::Inner,
        }
    }

    /// Get the MAIR attribute index value (8 bits)
    pub fn to_attr_value(&self) -> u8 {
        match self.mem_type {
            MemoryType::Device => 0x00, // Device-nGnRnE
            MemoryType::DeviceRE => 0x04, // Device-nGnRE
            MemoryType::DeviceGRE => 0x0C, // Device-GRE
            MemoryType::NormalWBWA => {
                // Normal WB-WA: Inner=0b1111, Outer=0b1111
                0xFF
            }
            MemoryType::NormalWT => {
                // Normal WT: Inner=0b1011, Outer=0b1011
                0xBB
            }
            MemoryType::NormalNC => {
                // Normal NC: Inner=0b0100, Outer=0b0100
                0x44
            }
        }
    }

    /// Get Stage-2 memory attribute field value (bits [5:2] in PTE)
    pub fn to_stage2_attr(&self) -> u8 {
        match self.mem_type {
            MemoryType::Device => 0x0,      // Device
            MemoryType::DeviceRE => 0x0,    // Device (same)
            MemoryType::DeviceGRE => 0x0,   // Device (same)
            MemoryType::NormalNC => 0x4,    // Normal Non-Cacheable
            MemoryType::NormalWT => 0x5,    // Normal Write-Through
            MemoryType::NormalWBWA => 0x7,  // Normal Write-Back
        }
    }
}

/// MAIR_EL2 (Memory Attribute Indirection Register) configuration
///
/// The MAIR_EL2 defines 8 memory attribute indices (Attr0-Attr7), each 8 bits.
#[derive(Debug, Clone, Copy)]
pub struct MairConfig {
    attr0: u8,
    attr1: u8,
    attr2: u8,
    attr3: u8,
    attr4: u8,
    attr5: u8,
    attr6: u8,
    attr7: u8,
}

impl MairConfig {
    /// Create default MAIR configuration
    ///
    /// Attr0: Device-nGnRnE (0x00)
    /// Attr1: Device-nGnRE (0x04)
    /// Attr2: Normal WB-WA (0xFF)
    /// Attr3: Normal WT (0xBB)
    /// Attr4: Normal NC (0x44)
    pub fn default() -> Self {
        Self {
            attr0: 0x00,  // Device-nGnRnE
            attr1: 0x04,  // Device-nGnRE
            attr2: 0xFF,  // Normal WB-WA
            attr3: 0xBB,  // Normal WT
            attr4: 0x44,  // Normal NC
            attr5: 0x00,  // Reserved
            attr6: 0x00,  // Reserved
            attr7: 0x00,  // Reserved
        }
    }

    /// Create MAIR config from raw attributes
    pub fn from_attrs(attrs: [u8; 8]) -> Self {
        Self {
            attr0: attrs[0],
            attr1: attrs[1],
            attr2: attrs[2],
            attr3: attrs[3],
            attr4: attrs[4],
            attr5: attrs[5],
            attr6: attrs[6],
            attr7: attrs[7],
        }
    }

    /// Set an attribute index
    pub fn set_attr(&mut self, index: usize, value: u8) -> Result<(), &'static str> {
        match index {
            0 => { self.attr0 = value; Ok(()) }
            1 => { self.attr1 = value; Ok(()) }
            2 => { self.attr2 = value; Ok(()) }
            3 => { self.attr3 = value; Ok(()) }
            4 => { self.attr4 = value; Ok(()) }
            5 => { self.attr5 = value; Ok(()) }
            6 => { self.attr6 = value; Ok(()) }
            7 => { self.attr7 = value; Ok(()) }
            _ => Err("Invalid attribute index"),
        }
    }

    /// Get an attribute index
    pub fn get_attr(&self, index: usize) -> Result<u8, &'static str> {
        match index {
            0 => Ok(self.attr0),
            1 => Ok(self.attr1),
            2 => Ok(self.attr2),
            3 => Ok(self.attr3),
            4 => Ok(self.attr4),
            5 => Ok(self.attr5),
            6 => Ok(self.attr6),
            7 => Ok(self.attr7),
            _ => Err("Invalid attribute index"),
        }
    }

    /// Encode to MAIR_EL2 value
    pub fn encode(&self) -> u64 {
        (self.attr0 as u64) << 0 |
        (self.attr1 as u64) << 8 |
        (self.attr2 as u64) << 16 |
        (self.attr3 as u64) << 24 |
        (self.attr4 as u64) << 32 |
        (self.attr5 as u64) << 40 |
        (self.attr6 as u64) << 48 |
        (self.attr7 as u64) << 56
    }

    /// Decode from MAIR_EL2 value
    pub fn decode(value: u64) -> Self {
        Self {
            attr0: ((value >> 0) & 0xFF) as u8,
            attr1: ((value >> 8) & 0xFF) as u8,
            attr2: ((value >> 16) & 0xFF) as u8,
            attr3: ((value >> 24) & 0xFF) as u8,
            attr4: ((value >> 32) & 0xFF) as u8,
            attr5: ((value >> 40) & 0xFF) as u8,
            attr6: ((value >> 48) & 0xFF) as u8,
            attr7: ((value >> 56) & 0xFF) as u8,
        }
    }

    /// Get attribute index for Stage-2 Device memory
    pub fn stage2_device_attr() -> u8 {
        // Stage-2 uses direct encoding, not MAIR indices
        // Return 0x0 which means Device in Stage-2
        0x0
    }

    /// Get attribute index for Stage-2 Normal WB-WA memory
    pub fn stage2_normal_wbwa_attr() -> u8 {
        // Stage-2 uses direct encoding, not MAIR indices
        // Return 0x7 which means Normal WB-WA in Stage-2
        0x7
    }

    /// Get attribute index for Stage-2 Normal WT memory
    pub fn stage2_normal_wt_attr() -> u8 {
        // Return 0x5 which means Normal WT in Stage-2
        0x5
    }

    /// Get attribute index for Stage-2 Normal NC memory
    pub fn stage2_normal_nc_attr() -> u8 {
        // Return 0x4 which means Normal NC in Stage-2
        0x4
    }
}

impl Default for MairConfig {
    fn default() -> Self {
        Self::default()
    }
}

/// Get current MAIR_EL2 value
///
/// # Safety
/// Must be called at EL2
pub unsafe fn read_mair_el2() -> u64 {
    let value: u64;
    core::arch::asm!("mrs {}, mair_el2", out(reg) value);
    value
}

/// Set MAIR_EL2 value
///
/// # Safety
/// Must be called at EL2
pub unsafe fn write_mair_el2(value: u64) {
    core::arch::asm!("msr mair_el2, {}", in(reg) value);
    // Ensure the change takes effect
    core::arch::asm!("isb", options(nostack, nomem));
}

/// Initialize MAIR_EL2 with default configuration
///
/// # Safety
/// Must be called at EL2
pub unsafe fn init_default() {
    let config = MairConfig::default();
    write_mair_el2(config.encode());
}

/// Initialize MAIR_EL2 with specified configuration
///
/// # Safety
/// Must be called at EL2
pub unsafe fn init(config: MairConfig) {
    write_mair_el2(config.encode());
}

/// Convert Stage-2 memory attribute to MAIR encoding
///
/// For Stage-2, memory attributes are encoded directly in the PTE,
/// not as MAIR indices like Stage-1.
pub fn stage2_memattr_to_encoding(mem_type: MemoryType) -> u64 {
    match mem_type {
        MemoryType::Device => 0x0,      // Device memory
        MemoryType::DeviceRE => 0x0,    // Device memory
        MemoryType::DeviceGRE => 0x0,   // Device memory
        MemoryType::NormalNC => 0x4,    // Normal Non-Cacheable
        MemoryType::NormalWT => 0x5,    // Normal Write-Through
        MemoryType::NormalWBWA => 0x7,  // Normal Write-Back
    }
}

/// Convert Stage-2 shareability to PTE encoding
pub fn shareability_to_encoding(sh: Shareability) -> u64 {
    match sh {
        Shareability::None => 0x0,      // Non-shareable
        Shareability::Outer => 0x2,     // Outer shareable
        Shareability::Inner => 0x3,     // Inner shareable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mair_default() {
        let config = MairConfig::default();
        let mair = config.encode();

        assert_eq!(config.attr0, 0x00); // Device-nGnRnE
        assert_eq!(config.attr2, 0xFF); // Normal WB-WA
        assert_eq!(config.attr3, 0xBB); // Normal WT
        assert_eq!(config.attr4, 0x44); // Normal NC
    }

    #[test]
    fn test_mair_encode_decode() {
        let config = MairConfig::default();
        let encoded = config.encode();
        let decoded = MairConfig::decode(encoded);

        assert_eq!(decoded.attr0, config.attr0);
        assert_eq!(decoded.attr1, config.attr1);
        assert_eq!(decoded.attr2, config.attr2);
        assert_eq!(decoded.attr3, config.attr3);
        assert_eq!(decoded.attr4, config.attr4);
    }

    #[test]
    fn test_mair_set_get_attr() {
        let mut config = MairConfig::default();

        assert!(config.set_attr(0, 0xAA).is_ok());
        assert_eq!(config.get_attr(0).unwrap(), 0xAA);

        assert!(config.set_attr(7, 0x55).is_ok());
        assert_eq!(config.get_attr(7).unwrap(), 0x55);

        assert!(config.set_attr(8, 0xFF).is_err());
        assert!(config.get_attr(8).is_err());
    }

    #[test]
    fn test_memory_attr_to_value() {
        let device = MemoryAttr::device();
        assert_eq!(device.to_attr_value(), 0x00);

        let wbwa = MemoryAttr::normal_wb_wa();
        assert_eq!(wbwa.to_attr_value(), 0xFF);

        let wt = MemoryAttr::normal_wt();
        assert_eq!(wt.to_attr_value(), 0xBB);

        let nc = MemoryAttr::normal_nc();
        assert_eq!(nc.to_attr_value(), 0x44);
    }

    #[test]
    fn test_stage2_memattr_encoding() {
        assert_eq!(stage2_memattr_to_encoding(MemoryType::Device), 0x0);
        assert_eq!(stage2_memattr_to_encoding(MemoryType::NormalNC), 0x4);
        assert_eq!(stage2_memattr_to_encoding(MemoryType::NormalWT), 0x5);
        assert_eq!(stage2_memattr_to_encoding(MemoryType::NormalWBWA), 0x7);
    }

    #[test]
    fn test_shareability_encoding() {
        assert_eq!(shareability_to_encoding(Shareability::None), 0x0);
        assert_eq!(shareability_to_encoding(Shareability::Outer), 0x2);
        assert_eq!(shareability_to_encoding(Shareability::Inner), 0x3);
    }

    #[test]
    fn test_memory_attr_stage2() {
        let device = MemoryAttr::device();
        assert_eq!(device.to_stage2_attr(), 0x0);

        let wbwa = MemoryAttr::normal_wb_wa();
        assert_eq!(wbwa.to_stage2_attr(), 0x7);

        let wt = MemoryAttr::normal_wt();
        assert_eq!(wt.to_stage2_attr(), 0x5);

        let nc = MemoryAttr::normal_nc();
        assert_eq!(nc.to_stage2_attr(), 0x4);
    }
}
