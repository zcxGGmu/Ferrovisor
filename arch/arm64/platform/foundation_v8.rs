//! ARM Foundation v8 model platform support
//!
//! Provides ARM Foundation v8 model platform initialization.

/// Foundation v8 memory map
pub mod mem_map {
    /// Base of RAM
    pub const RAM_BASE: u64 = 0x80000000;
    /// Size of RAM (default)
    pub const RAM_SIZE: u64 = 0x80000000; // 2GB
    /// UART0 base address
    pub const UART0_BASE: u64 = 0x1C090000;
    /// GIC distributor base
    pub const GICD_BASE: u64 = 0x2F000000;
    /// GIC redistributor base
    pub const GICR_BASE: u64 = 0x2F100000;
}

/// Platform information
pub struct PlatformInfo {
    pub ram_base: u64,
    pub ram_size: u64,
    pub num_cpus: u32,
}

/// Initialize Foundation v8 platform
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing Foundation v8 platform");

    let info = PlatformInfo {
        ram_base: mem_map::RAM_BASE,
        ram_size: mem_map::RAM_SIZE,
        num_cpus: 4, // Foundation v8 typically has 4-8 CPUs
    };

    log::info!("Foundation v8 platform: RAM {:#x}-{:#x}, {} CPUs",
              info.ram_base,
              info.ram_base + info.ram_size,
              info.num_cpus);

    log::info!("Foundation v8 platform initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mem_map() {
        assert_eq!(mem_map::RAM_BASE, 0x80000000);
        assert_eq!(mem_map::UART0_BASE, 0x1C090000);
        assert_eq!(mem_map::GICD_BASE, 0x2F000000);
    }
}
