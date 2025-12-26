//! Memory attributes for ARM64
//!
//! Provides MAIR_EL2 and memory attribute configuration.

/// Memory types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryType {
    /// Device memory
    Device,
    /// Normal memory (Write-Back Write-Allocate)
    NormalWBWA,
    /// Normal memory (Write-Through)
    NormalWT,
    /// Normal memory (Non-Cacheable)
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

/// Memory attribute
#[derive(Debug, Clone, Copy)]
pub struct MemoryAttr {
    pub mem_type: MemoryType,
    pub shareability: Shareability,
}

/// MAIR_EL2 (Memory Attribute Indirection Register) configuration
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
    pub fn default() -> Self {
        // Attr0: Device-nGnRnE (0x00)
        // Attr1: Device-nGnRE (0x04)
        // Attr2: Normal WB-WA (0xFF)
        // Attr3: Normal WT (0xBB)
        // Attr4: Normal NC (0x44)
        Self {
            attr0: 0x00,
            attr1: 0x04,
            attr2: 0xFF,
            attr3: 0xBB,
            attr4: 0x44,
            attr5: 0x00,
            attr6: 0x00,
            attr7: 0x00,
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
}

impl Default for MairConfig {
    fn default() -> Self {
        Self::default()
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
    }

    #[test]
    fn test_memory_types() {
        assert_eq!(MemoryType::Device, MemoryType::Device);
        assert_eq!(MemoryType::NormalWBWA, MemoryType::NormalWBWA);
    }
}
