//! QEMU virt platform support
//!
//! Provides QEMU ARM virt platform initialization.

/// QEMU virt memory map
pub mod mem_map {
    /// Base of RAM
    pub const RAM_BASE: u64 = 0x40000000;
    /// Size of RAM (default)
    pub const RAM_SIZE: u64 = 0x40000000; // 1GB
    /// UART base address
    pub const UART_BASE: u64 = 0x09000000;
    /// GIC distributor base
    pub const GICD_BASE: u64 = 0x08000000;
    /// GIC redistributor base
    pub const GICR_BASE: u64 = 0x080A0000;
}

/// Platform information
pub struct PlatformInfo {
    pub ram_base: u64,
    pub ram_size: u64,
    pub num_cpus: u32,
}

/// Initialize QEMU virt platform
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing QEMU virt platform");

    let info = PlatformInfo {
        ram_base: mem_map::RAM_BASE,
        ram_size: mem_map::RAM_SIZE,
        num_cpus: 4, // Default QEMU virt has 4 CPUs
    };

    log::info!("QEMU virt platform: RAM {:#x}-{:#x}, {} CPUs",
              info.ram_base,
              info.ram_base + info.ram_size,
              info.num_cpus);

    log::info!("QEMU virt platform initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mem_map() {
        assert_eq!(mem_map::RAM_BASE, 0x40000000);
        assert_eq!(mem_map::UART_BASE, 0x09000000);
        assert_eq!(mem_map::GICD_BASE, 0x08000000);
    }

    #[test]
    fn test_platform_info() {
        let info = PlatformInfo {
            ram_base: mem_map::RAM_BASE,
            ram_size: mem_map::RAM_SIZE,
            num_cpus: 4,
        };
        assert_eq!(info.num_cpus, 4);
    }
}
