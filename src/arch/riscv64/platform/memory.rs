//! RISC-V Platform Memory Configuration
//!
//! This module provides platform-specific memory configuration including:
//! - Memory regions and layout
//! - Memory attributes and permissions
//! - Memory initialization
//! - Platform-specific memory maps

use crate::arch::riscv64::*;

/// Memory region type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryType {
    /// Regular RAM
    Normal,
    /// Device memory (MMIO)
    Device,
    /// Uncacheable memory
    Uncacheable,
    /// Write-combining memory
    WriteCombining,
    /// Reserved memory
    Reserved,
}

/// Memory permissions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPermissions {
    /// No access
    None,
    /// Read only
    ReadOnly,
    /// Read/Write
    ReadWrite,
    /// Read/Execute
    ReadExecute,
    /// Read/Write/Execute
    ReadWriteExecute,
}

/// Memory region
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    /// Region name
    pub name: String,
    /// Base address
    pub base: u64,
    /// Size
    pub size: u64,
    /// Memory type
    pub mem_type: MemoryType,
    /// Permissions
    pub permissions: MemoryPermissions,
    /// Is cacheable
    pub cacheable: bool,
    /// Is shared
    pub shared: bool,
}

impl MemoryRegion {
    /// Create new memory region
    pub fn new(
        name: &str,
        base: u64,
        size: u64,
        mem_type: MemoryType,
        permissions: MemoryPermissions,
    ) -> Self {
        Self {
            name: name.to_string(),
            base,
            size,
            mem_type,
            permissions,
            cacheable: mem_type == MemoryType::Normal,
            shared: matches!(mem_type, MemoryType::Device | MemoryType::Uncacheable),
        }
    }

    /// Get end address (exclusive)
    pub fn end(&self) -> u64 {
        self.base + self.size
    }

    /// Check if address is within this region
    pub fn contains(&self, addr: u64) -> bool {
        addr >= self.base && addr < self.end()
    }

    /// Check if range overlaps with this region
    pub fn overlaps(&self, base: u64, size: u64) -> bool {
        let end = base + size;
        !(end <= self.base || base >= self.end())
    }

    /// Get page table attributes for this region
    pub fn get_pt_attributes(&self) -> (u64, u64) {
        let mut pte_flags = 0u64;
        let mut pte_attributes = 0u64;

        // Set permissions
        match self.permissions {
            MemoryPermissions::None => {
                pte_flags |= 0; // No access
            }
            MemoryPermissions::ReadOnly => {
                pte_flags |= PTE_R;
            }
            MemoryPermissions::ReadWrite => {
                pte_flags |= PTE_R | PTE_W;
            }
            MemoryPermissions::ReadExecute => {
                pte_flags |= PTE_R | PTE_X;
            }
            MemoryPermissions::ReadWriteExecute => {
                pte_flags |= PTE_R | PTE_W | PTE_X;
            }
        }

        // Set memory type attributes
        match self.mem_type {
            MemoryType::Normal => {
                pte_attributes |= PMA_CACHEABLE;
            }
            MemoryType::Device => {
                pte_attributes |= PMA_DEVICE;
            }
            MemoryType::Uncacheable => {
                pte_attributes |= 0; // No caching
            }
            MemoryType::WriteCombining => {
                pte_attributes |= PMA_WRITE_COMBINE;
            }
            MemoryType::Reserved => {
                pte_attributes |= PMA_RESERVED;
            }
        }

        // Set shared flag
        if self.shared {
            pte_attributes |= PMA_SHARED;
        }

        (pte_flags, pte_attributes)
    }
}

/// Memory configuration
#[derive(Debug, Clone)]
pub struct MemoryConfig {
    /// Memory regions
    pub regions: Vec<MemoryRegion>,
    /// Page size
    pub page_size: u64,
    /// Enable huge pages
    pub enable_huge_pages: bool,
    /// Memory attributes
    pub default_attributes: u64,
}

impl MemoryConfig {
    /// Create new memory configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Get memory regions
    pub fn get_regions(&self) -> Vec<MemoryRegion> {
        self.regions.clone()
    }

    /// Add memory region
    pub fn add_region(&mut self, region: MemoryRegion) -> Result<(), String> {
        // Check for overlaps
        for existing in &self.regions {
            if existing.overlaps(region.base, region.size) {
                return Err(format!(
                    "Memory region overlaps with existing region '{}': {:#x}-{:#x}",
                    existing.name,
                    existing.base,
                    existing.end()
                ));
            }
        }

        self.regions.push(region);
        Ok(())
    }

    /// Find region containing address
    pub fn find_region(&self, addr: u64) -> Option<&MemoryRegion> {
        self.regions.iter().find(|r| r.contains(addr))
    }

    /// Get platform-specific memory map
    pub fn get_platform_memory_map(platform_type: super::PlatformType) -> Self {
        match platform_type {
            super::PlatformType::QemuVirt => Self::qemu_virt_memory_map(),
            super::PlatformType::SiFiveUnleashed => Self::sifive_unleashed_memory_map(),
            super::PlatformType::AllwinnerD1 => Self::allwinner_d1_memory_map(),
            super::PlatformType::Custom => Self::default(),
        }
    }

    /// QEMU Virt memory map
    pub fn qemu_virt_memory_map() -> Self {
        let mut config = Self {
            regions: Vec::new(),
            page_size: PAGE_SIZE as u64,
            enable_huge_pages: true,
            default_attributes: PMA_CACHEABLE,
        };

        // Flash memory (readonly)
        config.regions.push(MemoryRegion::new(
            "flash",
            0x20000000,
            0x4000000, // 64MB
            MemoryType::Normal,
            MemoryPermissions::ReadExecute,
        ));

        // ROM
        config.regions.push(MemoryRegion::new(
            "rom",
            0x1000,
            0x11000,
            MemoryType::Normal,
            MemoryPermissions::ReadOnly,
        ));

        // Main RAM
        config.regions.push(MemoryRegion::new(
            "ram",
            0x80000000,
            0x80000000, // 2GB
            MemoryType::Normal,
            MemoryPermissions::ReadWriteExecute,
        ));

        // UART (device)
        config.regions.push(MemoryRegion::new(
            "uart",
            0x10000000,
            0x1000,
            MemoryType::Device,
            MemoryPermissions::ReadWrite,
        ));

        // VirtIO devices
        config.regions.push(MemoryRegion::new(
            "virtio",
            0x10001000,
            0x1000,
            MemoryType::Device,
            MemoryPermissions::ReadWrite,
        ));

        // PLIC (interrupt controller)
        config.regions.push(MemoryRegion::new(
            "plic",
            0x0c000000,
            0x4000000, // 64MB
            MemoryType::Device,
            MemoryPermissions::ReadWrite,
        ));

        // CLINT (core-local interruptor)
        config.regions.push(MemoryRegion::new(
            "clint",
            0x02000000,
            0x10000,
            MemoryType::Device,
            MemoryPermissions::ReadWrite,
        ));

        // Test device (for QEMU)
        config.regions.push(MemoryRegion::new(
            "test",
            0x100000,
            0x1000,
            MemoryType::Device,
            MemoryPermissions::ReadWrite,
        ));

        config
    }

    /// SiFive HiFive Unleashed memory map
    pub fn sifive_unleashed_memory_map() -> Self {
        let mut config = Self {
            regions: Vec::new(),
            page_size: PAGE_SIZE as u64,
            enable_huge_pages: true,
            default_attributes: PMA_CACHEABLE,
        };

        // Main RAM
        config.regions.push(MemoryRegion::new(
            "ram",
            0x80000000,
            0x80000000, // 2GB
            MemoryType::Normal,
            MemoryPermissions::ReadWriteExecute,
        ));

        // UART0
        config.regions.push(MemoryRegion::new(
            "uart0",
            0x10010000,
            0x1000,
            MemoryType::Device,
            MemoryPermissions::ReadWrite,
        ));

        // UART1
        config.regions.push(MemoryRegion::new(
            "uart1",
            0x10011000,
            0x1000,
            MemoryType::Device,
            MemoryPermissions::ReadWrite,
        ));

        // GPIO
        config.regions.push(MemoryRegion::new(
            "gpio",
            0x10020000,
            0x1000,
            MemoryType::Device,
            MemoryPermissions::ReadWrite,
        ));

        // PLIC
        config.regions.push(MemoryRegion::new(
            "plic",
            0x0c000000,
            0x4000000, // 64MB
            MemoryType::Device,
            MemoryPermissions::ReadWrite,
        ));

        // CLINT
        config.regions.push(MemoryRegion::new(
            "clint",
            0x02000000,
            0x10000,
            MemoryType::Device,
            MemoryPermissions::ReadWrite,
        ));

        config
    }

    /// Allwinner D1 memory map
    pub fn allwinner_d1_memory_map() -> Self {
        let mut config = Self {
            regions: Vec::new(),
            page_size: PAGE_SIZE as u64,
            enable_huge_pages: false, // D1 doesn't support huge pages
            default_attributes: PMA_CACHEABLE,
        };

        // Main RAM
        config.regions.push(MemoryRegion::new(
            "ram",
            0x40000000,
            0x40000000, // 1GB
            MemoryType::Normal,
            MemoryPermissions::ReadWriteExecute,
        ));

        // SRAM A1
        config.regions.push(MemoryRegion::new(
            "sram_a1",
            0x00020000,
            0x8000, // 32KB
            MemoryType::Normal,
            MemoryPermissions::ReadWriteExecute,
        ));

        // SRAM C
        config.regions.push(MemoryRegion::new(
            "sram_c",
            0x00080000,
            0x10000, // 64KB
            MemoryType::Normal,
            MemoryPermissions::ReadWriteExecute,
        ));

        // UART0
        config.regions.push(MemoryRegion::new(
            "uart0",
            0x02500000,
            0x1000,
            MemoryType::Device,
            MemoryPermissions::ReadWrite,
        ));

        // GPIO
        config.regions.push(MemoryRegion::new(
            "gpio",
            0x02000000,
            0x1000,
            MemoryType::Device,
            MemoryPermissions::ReadWrite,
        ));

        // Timer
        config.regions.push(MemoryRegion::new(
            "timer",
            0x02050000,
            0x1000,
            MemoryType::Device,
            MemoryPermissions::ReadWrite,
        ));

        config
    }

    /// Initialize memory regions
    pub fn init(&self) -> Result<(), &'static str> {
        log::debug!("Initializing platform memory regions");

        for region in &self.regions {
            log::debug!("Memory region: {} {:#x}-{:#x} ({:?})",
                       region.name, region.base, region.end(), region.mem_type);

            // Initialize region based on type
            match region.mem_type {
                MemoryType::Normal => {
                    // Zero out BSS sections if this is RAM and writable
                    if region.permissions != MemoryPermissions::ReadOnly {
                        // TODO: Clear memory region
                    }
                }
                MemoryType::Device => {
                    // Device regions don't need initialization
                }
                MemoryType::Uncacheable | MemoryType::WriteCombining => {
                    // Special memory regions
                }
                MemoryType::Reserved => {
                    // Reserved regions should not be accessed
                }
            }
        }

        log::debug!("Platform memory regions initialized");
        Ok(())
    }

    /// Validate memory configuration
    pub fn validate(&self) -> Result<(), String> {
        // Check for overlapping regions
        for (i, region1) in self.regions.iter().enumerate() {
            for region2 in self.regions.iter().skip(i + 1) {
                if region1.overlaps(region2.base, region2.size) {
                    return Err(format!(
                        "Memory regions overlap: '{}' {:#x}-{:#x} and '{}' {:#x}-{:#x}",
                        region1.name, region1.base, region1.end(),
                        region2.name, region2.base, region2.end()
                    ));
                }
            }
        }

        // Check alignment
        for region in &self.regions {
            if (region.base & (self.page_size - 1)) != 0 {
                return Err(format!(
                    "Memory region '{}' base address {:#x} is not page-aligned",
                    region.name, region.base
                ));
            }

            if (region.size & (self.page_size - 1)) != 0 {
                return Err(format!(
                    "Memory region '{}' size {:#x} is not page-aligned",
                    region.name, region.size
                ));
            }
        }

        Ok(())
    }
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self::qemu_virt_memory_map()
    }
}

// Page table entry flags
const PTE_V: u64 = 1 << 0;       // Valid
const PTE_R: u64 = 1 << 1;       // Read
const PTE_W: u64 = 1 << 2;       // Write
const PTE_X: u64 = 1 << 3;       // Execute
const PTE_U: u64 = 1 << 4;       // User
const PTE_G: u64 = 1 << 5;       // Global
const PTE_A: u64 = 1 << 6;       // Accessed
const PTE_D: u64 = 1 << 7;       // Dirty

// Physical memory attributes
const PMA_CACHEABLE: u64 = 1 << 0;
const PMA_DEVICE: u64 = 1 << 1;
const PMA_WRITE_COMBINE: u64 = 1 << 2;
const PMA_SHARED: u64 = 1 << 3;
const PMA_RESERVED: u64 = 1 << 4;

/// Initialize platform memory
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing platform memory");

    // Get memory configuration from platform
    let config = if let Some(platform_config) = super::get_platform_configurations() {
        platform_config.memory.clone()
    } else {
        MemoryConfig::default()
    };

    // Validate configuration
    config.validate()
        .map_err(|e| {
            log::error!("Invalid memory configuration: {}", e);
            "Invalid memory configuration"
        })?;

    // Initialize memory regions
    config.init()?;

    log::info!("Platform memory initialized successfully");
    Ok(())
}

/// Get current memory configuration
pub fn get_config() -> Option<MemoryConfig> {
    super::get_platform_configurations().map(|c| c.memory.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_region() {
        let region = MemoryRegion::new(
            "test",
            0x80000000,
            0x100000,
            MemoryType::Normal,
            MemoryPermissions::ReadWrite,
        );

        assert_eq!(region.name, "test");
        assert_eq!(region.base, 0x80000000);
        assert_eq!(region.size, 0x100000);
        assert_eq!(region.end(), 0x80100000);
        assert!(region.contains(0x80000000));
        assert!(region.contains(0x800FFFFF));
        assert!(!region.contains(0x80100000));
        assert!(region.overlaps(0x80050000, 0x1000));
        assert!(!region.overlaps(0x80100000, 0x1000));
    }

    #[test]
    fn test_memory_config() {
        let config = MemoryConfig::default();
        assert!(!config.regions.is_empty());
        assert_eq!(config.page_size, PAGE_SIZE as u64);
        assert!(config.enable_huge_pages);
    }

    #[test]
    fn test_qemu_virt_memory_map() {
        let config = MemoryConfig::qemu_virt_memory_map();

        // Check for key regions
        assert!(config.find_region(0x80000000).is_some()); // RAM
        assert!(config.find_region(0x10000000).is_some()); // UART
        assert!(config.find_region(0x0c000000).is_some()); // PLIC
        assert!(config.find_region(0x02000000).is_some()); // CLINT
    }

    #[test]
    fn test_memory_permissions() {
        let region_rw = MemoryRegion::new(
            "test",
            0x80000000,
            0x1000,
            MemoryType::Normal,
            MemoryPermissions::ReadWrite,
        );
        let (flags, _attr) = region_rw.get_pt_attributes();
        assert_eq!(flags & (PTE_R | PTE_W), PTE_R | PTE_W);
        assert_eq!(flags & PTE_X, 0);

        let region_rx = MemoryRegion::new(
            "test",
            0x80000000,
            0x1000,
            MemoryType::Normal,
            MemoryPermissions::ReadExecute,
        );
        let (flags, _attr) = region_rx.get_pt_attributes();
        assert_eq!(flags & (PTE_R | PTE_X), PTE_R | PTE_X);
        assert_eq!(flags & PTE_W, 0);
    }

    #[test]
    fn test_memory_attributes() {
        let region_normal = MemoryRegion::new(
            "test",
            0x80000000,
            0x1000,
            MemoryType::Normal,
            MemoryPermissions::ReadWrite,
        );
        assert!(region_normal.cacheable);
        assert!(!region_normal.shared);

        let region_device = MemoryRegion::new(
            "test",
            0x10000000,
            0x1000,
            MemoryType::Device,
            MemoryPermissions::ReadWrite,
        );
        assert!(!region_device.cacheable);
        assert!(region_device.shared);
    }

    #[test]
    fn test_config_validation() {
        let mut config = MemoryConfig::default();

        // Valid configuration should pass
        assert!(config.validate().is_ok());

        // Add overlapping region should fail
        let overlapping = MemoryRegion::new(
            "overlap",
            0x80000000,
            0x1000,
            MemoryType::Normal,
            MemoryPermissions::ReadWrite,
        );
        assert!(config.add_region(overlapping).is_err());
    }
}