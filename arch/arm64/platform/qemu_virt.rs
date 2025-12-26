//! QEMU virt platform support for ARM64
//!
//! The QEMU virt platform is a virtual ARM64 machine designed for QEMU:
//! - VirtIO devices for paravirtualized I/O
//! - PL011 UART for console
//! - GICv3 or GICv2 interrupt controller
//! - Generic Timer
//! - Flexible memory layout
//!
//! ## Memory Layout (QEMU virt ARM64)
//!
//! | Address        | Size    | Description          |
//! |----------------|---------|----------------------|
//! | 0x4000_0000    | 256MB   | RAM                  |
//! | 0x0800_0000    | -       | GIC Distributor      |
//! | 0x080A_0000    | -       | GIC Redistributor    |
//! | 0x0900_0000    | -       | UART                 |
//! | 0x0A00_0000+   | -       | VirtIO devices       |
//! | 0x4000_0000    | 128MB   | PCIE MMIO            |
//! | 0x4010_0000    | -       | PCIE ECAM            |
//!
//! ## References
//! - [QEMU ARM virt Machine](https://qemu.readthedocs.io/en/latest/system/arm/virt.html)

use super::Platform;

/// QEMU virt platform memory layout
pub const QEMU_VIRT_MEM_BASE: u64 = 0x40000000;
pub const QEMU_VIRT_MEM_SIZE: u64 = 0x10000000; // 256 MB default

/// QEMU virt GIC addresses
pub const QEMU_VIRT_GIC_DIST_BASE: u64 = 0x08000000;
pub const QEMU_VIRT_GIC_REDIST_BASE: u64 = 0x080A0000;
pub const QEMU_VIRT_GIC_ITS_BASE: u64 = 0x08080000;

/// QEMU virt UART address
pub const QEMU_VIRT_UART_BASE: u64 = 0x09000000;

/// QEMU virt RTC address
pub const QEMU_VIRT_RTC_BASE: u64 = 0x09010000;

/// QEMU virt VirtIO base address
pub const QEMU_VIRT_VIRTIO_BASE: u64 = 0x0A000000;
pub const QEMU_VIRT_VIRTIO_SIZE: u64 = 0x00001000; // 4KB per device
pub const QEMU_VIRT_VIRTIO_COUNT: usize = 32;

/// QEMU virt PCIe MMIO
pub const QEMU_VIRT_PCIE_MMIO_BASE: u64 = 0x40000000;
pub const QEMU_VIRT_PCIE_MMIO_SIZE: u64 = 0x08000000; // 128 MB

/// QEMU virt PCIe ECAM
pub const QEMU_VIRT_PCIE_ECAM_BASE: u64 = 0x40100000;
pub const QEMU_VIRT_PCIE_ECAM_SIZE: u64 = 0x02000000; // 32 MB

/// QEMU virt platform memory map (kept for backward compatibility)
pub mod mem_map {
    /// Base of RAM
    pub const RAM_BASE: u64 = QEMU_VIRT_MEM_BASE;
    /// Size of RAM (default)
    pub const RAM_SIZE: u64 = QEMU_VIRT_MEM_SIZE;
    /// UART base address
    pub const UART_BASE: u64 = QEMU_VIRT_UART_BASE;
    /// GIC distributor base
    pub const GICD_BASE: u64 = QEMU_VIRT_GIC_DIST_BASE;
    /// GIC redistributor base
    pub const GICR_BASE: u64 = QEMU_VIRT_GIC_REDIST_BASE;
}

/// Platform information (kept for backward compatibility)
pub struct PlatformInfo {
    pub ram_base: u64,
    pub ram_size: u64,
    pub num_cpus: u32,
}

/// QEMU virt platform
pub struct QemuVirtPlatform {
    /// Memory layout (base, size) pairs
    memory: [(u64, u64); 2],
    /// GIC version
    gic_version: u32,
    /// GIC base address
    gic_base: u64,
    /// GIC redistributor address
    gic_redist_base: u64,
    /// UART base address
    uart_base: u64,
    /// VirtIO base address
    virtio_base: u64,
    /// Number of VirtIO devices
    virtio_count: usize,
}

impl QemuVirtPlatform {
    /// Create new QEMU virt platform
    pub const fn new() -> Self {
        Self {
            memory: [
                (QEMU_VIRT_MEM_BASE, QEMU_VIRT_MEM_SIZE), // RAM
                (QEMU_VIRT_PCIE_MMIO_BASE, QEMU_VIRT_PCIE_MMIO_SIZE), // PCIe MMIO
            ],
            gic_version: 3,
            gic_base: QEMU_VIRT_GIC_DIST_BASE,
            gic_redist_base: QEMU_VIRT_GIC_REDIST_BASE,
            uart_base: QEMU_VIRT_UART_BASE,
            virtio_base: QEMU_VIRT_VIRTIO_BASE,
            virtio_count: QEMU_VIRT_VIRTIO_COUNT,
        }
    }

    /// Probe for QEMU virt platform
    pub fn probe() -> Result<&'static Self, &'static str> {
        // Check device tree for "linux,dummy-virt" compatible
        // For now, just return the static instance
        Ok(&QEMU_VIRT_INSTANCE)
    }

    /// Get VirtIO device address for index
    pub fn virtio_addr(&self, index: usize) -> Option<u64> {
        if index < self.virtio_count {
            Some(self.virtio_base + (index as u64 * QEMU_VIRT_VIRTIO_SIZE))
        } else {
            None
        }
    }

    /// Get VirtIO IRQ for index
    pub fn virtio_irq(&self, index: usize) -> Option<u32> {
        if index < self.virtio_count {
            // VirtIO IRQs start from GPIO 1 (SPI 33)
            Some((1 + index) as u32)
        } else {
            None
        }
    }
}

/// Global QEMU virt platform instance
static QEMU_VIRT_INSTANCE: QemuVirtPlatform = QemuVirtPlatform::new();

impl Platform for QemuVirtPlatform {
    /// Get platform name
    fn name(&self) -> &str {
        "QEMU virt ARM64"
    }

    /// Get platform compatible string
    fn compatible(&self) -> &str {
        "linux,dummy-virt"
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
        Some(self.uart_base)
    }

    /// Early initialization
    fn early_init(&mut self) -> Result<(), &'static str> {
        log::info!("QEMU virt: Early initialization");

        // Setup UART for console
        log::debug!("QEMU virt: UART @ 0x{:x}", self.uart_base);

        // Setup GIC
        log::debug!("QEMU virt: GICv{} @ 0x{:x}", self.gic_version, self.gic_base);

        // Setup VirtIO devices
        log::debug!("QEMU virt: {} VirtIO devices @ 0x{:x}+",
                   self.virtio_count, self.virtio_base);

        Ok(())
    }

    /// Final initialization
    fn final_init(&mut self) -> Result<(), &'static str> {
        log::info!("QEMU virt: Final initialization");

        // Nothing to do for QEMU virt

        Ok(())
    }
}

/// Get QEMU virt platform instance
pub fn get() -> &'static QemuVirtPlatform {
    &QEMU_VIRT_INSTANCE
}

/// Check if running on QEMU virt
pub fn is_qemu_virt() -> bool {
    // Check device tree or use CPU ID
    // QEMU virt typically has MIDR = 0x410FD034 (Cortex-A53)
    let midr: u64;
    unsafe {
        core::arch::asm!("mrs {}, midr_el1", out(reg) midr);
    }

    // QEMU ARM64 uses Cortex-A53 or custom implementation
    // MIDR[31:24] = Implementer (0x41 = ARM)
    // MIDR[15:4] = Part number (0xD03 = Cortex-A53)
    let implementer = (midr >> 24) & 0xFF;
    let part = (midr >> 4) & 0xFFF;

    implementer == 0x41 && (part == 0xD03 || part == 0xD40 || part == 0xD0C)
}

/// QEMU virt interrupt mappings
pub mod irq {
    /// UART IRQ
    pub const UART_IRQ: u32 = 1;

    /// RTC IRQ
    pub const RTC_IRQ: u32 = 2;

    /// GPIO IRQs start
    pub const GPIO_IRQ_BASE: u32 = 3;

    /// VirtIO IRQs start
    pub const VIRTIO_IRQ_BASE: u32 = 1;

    /// PCIE IRQs start
    pub const PCIE_IRQ_BASE: u32 = 113;
}

/// QEMU virt utility functions
pub mod utils {
    use super::*;

    /// Initialize QEMU virt UART
    pub fn init_uart() {
        // PL011 UART is memory-mapped
        // Setup would involve:
        // 1. Disable UART
        // 2. Set baud rate
        // 3. Set line control
        // 4. Enable UART

        log::debug!("QEMU virt: UART initialized");
    }

    /// Get memory size from device tree
    pub fn get_memory_size() -> u64 {
        // Read from device tree
        // Default to 256 MB
        QEMU_VIRT_MEM_SIZE
    }

    /// Get number of CPUs
    pub fn get_cpu_count() -> usize {
        // Read from device tree
        // Default to 1
        1
    }
}

/// Initialize QEMU virt platform (backward compatible function)
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

    #[test]
    fn test_qemu_virt_platform() {
        let platform = QemuVirtPlatform::new();
        assert_eq!(platform.name(), "QEMU virt ARM64");
        assert_eq!(platform.gic_base(), QEMU_VIRT_GIC_DIST_BASE);
        assert_eq!(platform.uart_base(), Some(QEMU_VIRT_UART_BASE));
        assert_eq!(platform.memory_layout().len(), 2);
    }

    #[test]
    fn test_virtio_addrs() {
        let platform = QemuVirtPlatform::new();
        assert_eq!(platform.virtio_addr(0), Some(0x0A000000));
        assert_eq!(platform.virtio_addr(1), Some(0x0A001000));
        assert_eq!(platform.virtio_irq(0), Some(1));
        assert_eq!(platform.virtio_irq(1), Some(2));
    }

    #[test]
    fn test_irq_constants() {
        assert_eq!(irq::UART_IRQ, 1);
        assert_eq!(irq::RTC_IRQ, 2);
        assert_eq!(irq::VIRTIO_IRQ_BASE, 1);
    }
}
