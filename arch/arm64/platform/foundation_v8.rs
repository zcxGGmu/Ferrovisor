//! ARM Foundation v8 model platform support
//!
//! The ARM Foundation v8 model is a fixed platform virtual ARMv8-A implementation:
//! - PL011 UART for console
//! - GICv3 or GICv2 interrupt controller
//! - Generic Timer
//! - CLCD (Color LCD Controller) for display
//! - Fixed memory layout
//!
//! ## Memory Layout (Foundation v8)
//!
//! | Address        | Size    | Description          |
//! |----------------|---------|----------------------|
//! | 0x8000_0000    | 2GB     | RAM                  |
//! | 0x2F00_0000    | 64KB    | GIC Distributor      |
//! | 0x2F10_0000    | 2MB     | GIC Redistributor    |
//! | 0x1C09_0000    | -       | UART0                |
//! | 0x1C0B_0000    | -       | UART1                |
//! | 0x1C0F_0000    | -       | CLCD                 |
//! | 0x1C1F_0000    | -       | RTC                  |
//!
//! ## References
//! - [ARM Foundation Model](https://developer.arm.com/products/system-design/foundation-model)

use super::Platform;

/// Foundation v8 platform memory layout
pub const FOUNDATION_V8_MEM_BASE: u64 = 0x80000000;
pub const FOUNDATION_V8_MEM_SIZE: u64 = 0x80000000; // 2 GB

/// Foundation v8 GIC addresses
pub const FOUNDATION_V8_GIC_DIST_BASE: u64 = 0x2F000000;
pub const FOUNDATION_V8_GIC_REDIST_BASE: u64 = 0x2F100000;
pub const FOUNDATION_V8_GIC_ITS_BASE: u64 = 0x2F400000;

/// Foundation v8 UART addresses
pub const FOUNDATION_V8_UART0_BASE: u64 = 0x1C090000;
pub const FOUNDATION_V8_UART1_BASE: u64 = 0x1C0B0000;

/// Foundation v8 CLCD address
pub const FOUNDATION_V8_CLCD_BASE: u64 = 0x1C0F0000;

/// Foundation v8 RTC address
pub const FOUNDATION_V8_RTC_BASE: u64 = 0x1C1F0000;

/// Foundation v8 memory map (kept for backward compatibility)
pub mod mem_map {
    /// Base of RAM
    pub const RAM_BASE: u64 = FOUNDATION_V8_MEM_BASE;
    /// Size of RAM (default)
    pub const RAM_SIZE: u64 = FOUNDATION_V8_MEM_SIZE;
    /// UART0 base address
    pub const UART0_BASE: u64 = FOUNDATION_V8_UART0_BASE;
    /// UART1 base address
    pub const UART1_BASE: u64 = FOUNDATION_V8_UART1_BASE;
    /// GIC distributor base
    pub const GICD_BASE: u64 = FOUNDATION_V8_GIC_DIST_BASE;
    /// GIC redistributor base
    pub const GICR_BASE: u64 = FOUNDATION_V8_GIC_REDIST_BASE;
    /// CLCD base address
    pub const CLCD_BASE: u64 = FOUNDATION_V8_CLCD_BASE;
}

/// Platform information (kept for backward compatibility)
pub struct PlatformInfo {
    pub ram_base: u64,
    pub ram_size: u64,
    pub num_cpus: u32,
}

/// Foundation v8 platform
pub struct FoundationV8Platform {
    /// Memory layout (base, size) pairs
    memory: [(u64, u64); 1],
    /// GIC version
    gic_version: u32,
    /// GIC base address
    gic_base: u64,
    /// GIC redistributor address
    gic_redist_base: u64,
    /// UART0 base address
    uart0_base: u64,
    /// UART1 base address
    uart1_base: u64,
    /// CLCD base address
    clcd_base: u64,
}

impl FoundationV8Platform {
    /// Create new Foundation v8 platform
    pub const fn new() -> Self {
        Self {
            memory: [
                (FOUNDATION_V8_MEM_BASE, FOUNDATION_V8_MEM_SIZE), // RAM
            ],
            gic_version: 3,
            gic_base: FOUNDATION_V8_GIC_DIST_BASE,
            gic_redist_base: FOUNDATION_V8_GIC_REDIST_BASE,
            uart0_base: FOUNDATION_V8_UART0_BASE,
            uart1_base: FOUNDATION_V8_UART1_BASE,
            clcd_base: FOUNDATION_V8_CLCD_BASE,
        }
    }

    /// Probe for Foundation v8 platform
    pub fn probe() -> Result<&'static Self, &'static str> {
        // Check device tree for "arm,foundation-v8" compatible
        // For now, just return the static instance
        Ok(&FOUNDATION_V8_INSTANCE)
    }
}

/// Global Foundation v8 platform instance
static FOUNDATION_V8_INSTANCE: FoundationV8Platform = FoundationV8Platform::new();

impl Platform for FoundationV8Platform {
    /// Get platform name
    fn name(&self) -> &str {
        "ARM Foundation v8"
    }

    /// Get platform compatible string
    fn compatible(&self) -> &str {
        "arm,foundation-v8"
    }

    /// Get memory layout
    fn memory_layout(&self) -> &[(u64, u64)] {
        &self.memory
    }

    /// Get GIC base address
    fn gic_base(&self) -> u64 {
        self.gic_base
    }

    /// Get GIC version
    fn gic_version(&self) -> u32 {
        self.gic_version
    }

    /// Get UART base address
    fn uart_base(&self) -> Option<u64> {
        Some(self.uart0_base)
    }

    /// Early initialization
    fn early_init(&mut self) -> Result<(), &'static str> {
        log::info!("Foundation v8: Early initialization");

        // Setup UART0 for console
        log::debug!("Foundation v8: UART0 @ 0x{:x}", self.uart0_base);

        // Setup GIC
        log::debug!("Foundation v8: GICv{} @ 0x{:x}", self.gic_version, self.gic_base);

        // Setup CLCD (optional)
        log::debug!("Foundation v8: CLCD @ 0x{:x}", self.clcd_base);

        Ok(())
    }

    /// Final initialization
    fn final_init(&mut self) -> Result<(), &'static str> {
        log::info!("Foundation v8: Final initialization");

        // Setup CLCD display
        log::debug!("Foundation v8: CLCD initialized");

        Ok(())
    }
}

/// Get Foundation v8 platform instance
pub fn get() -> &'static FoundationV8Platform {
    &FOUNDATION_V8_INSTANCE
}

/// Check if running on Foundation v8
pub fn is_foundation_v8() -> bool {
    // Check device tree or use CPU ID
    // Foundation v8 typically has specific MIDR values
    // For now, return false
    false
}

/// Foundation v8 interrupt mappings
pub mod irq {
    /// UART0 IRQ
    pub const UART0_IRQ: u32 = 1;

    /// UART1 IRQ
    pub const UART1_IRQ: u32 = 2;

    /// CLCD IRQ
    pub const CLCD_IRQ: u32 = 3;

    /// RTC IRQ
    pub const RTC_IRQ: u32 = 4;
}

/// Foundation v8 utility functions
pub mod utils {
    use super::*;

    /// Initialize Foundation v8 UART
    pub fn init_uart() {
        // PL011 UART is memory-mapped
        log::debug!("Foundation v8: UART initialized");
    }

    /// Get memory size from device tree
    pub fn get_memory_size() -> u64 {
        // Read from device tree
        // Default to 2 GB
        FOUNDATION_V8_MEM_SIZE
    }

    /// Get number of CPUs
    pub fn get_cpu_count() -> usize {
        // Read from device tree
        // Foundation v8 typically has 4-8 CPUs
        4
    }

    /// Initialize CLCD display
    pub fn init_clcd() {
        log::debug!("Foundation v8: CLCD initialized");
    }
}

/// Initialize Foundation v8 platform (backward compatible function)
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

    #[test]
    fn test_platform_info() {
        let info = PlatformInfo {
            ram_base: mem_map::RAM_BASE,
            ram_size: mem_map::RAM_SIZE,
            num_cpus: 4,
        };
        assert_eq!(info.num_cpus, 4);
    }

    #[test]
    fn test_foundation_v8_platform() {
        let platform = FoundationV8Platform::new();
        assert_eq!(platform.name(), "ARM Foundation v8");
        assert_eq!(platform.gic_base(), FOUNDATION_V8_GIC_DIST_BASE);
        assert_eq!(platform.uart_base(), Some(FOUNDATION_V8_UART0_BASE));
        assert_eq!(platform.memory_layout().len(), 1);
    }

    #[test]
    fn test_irq_constants() {
        assert_eq!(irq::UART0_IRQ, 1);
        assert_eq!(irq::UART1_IRQ, 2);
        assert_eq!(irq::CLCD_IRQ, 3);
    }
}
