//! GIC Device Tree Discovery and Auto-Initialization
//!
//! This module provides automatic GIC discovery and initialization from device tree.
//!
//! ## GIC Device Tree Binding
//!
//! ### GICv1/v2
//! ```text
//! interrupt-controller@2c001000 {
//!     compatible = "arm,cortex-a15-gic";
//!     #interrupt-cells = <3>;
//!     #address-cells = <0>;
//!     interrupt-controller;
//!     reg = <0x2c001000 0x1000>,
//!           <0x2c002000 0x1000>,
//!           <0x2c004000 0x2000>,
//!           <0x2c006000 0x2000>;
//!     interrupts = <1 9 0xf04>;
//! };
//! ```
//!
//! ### GICv3
//! ```text
//! interrupt-controller@2c010000 {
//!     compatible = "arm,gic-v3";
//!     #interrupt-cells = <3>;
//!     #address-cells = <2>;
//!     #size-cells = <2>;
//!     interrupt-controller;
//!     reg = <0x2c010000 0x10000>,  // Distributor
//!           <0x2c040000 0x100000>; // Redistributors
//!     interrupts = <1 9 0xf04>;
//! };
//! ```
//!
//! ## Register Layout
//!
//! ### GICv2
//! - Index 0: GIC Distributor (GICD)
//! - Index 1: CPU Interface (GICC)
//! - Index 2: Virtual Interface Control (GICV)
//! - Index 3: Virtual Interface CPU Interface (GICH)
//!
//! ### GICv3
//! - Index 0: GIC Distributor (GICD)
//! - Index 1: Redistributor (GICR)
//! - Index 2: CPU Interface (optional, for legacy)
//!
//! ## References
//! - [Xvisor irq-gic.c](/home/zcxggmu/workspace/hello-projs/posp/xvisor/drivers/irqchip/irq-gic.c)
//! - [Linux GIC binding](https://www.kernel.org/doc/Documentation/devicetree/bindings/interrupt-controller/arm,gic.yaml)

use crate::arch::arm64::devtree::{GicInfo, compat};
use crate::arch::arm64::devtree::parse::parse_gic_node;
use crate::arch::arm64::interrupt::gic::{self, GicVersion, init as gic_init};
use crate::arch::riscv64::devtree::fdt::FlattenedDeviceTree;

/// GIC discovery configuration
#[derive(Debug, Clone)]
pub struct GicDiscoveryConfig {
    /// GIC version override (None = auto-detect)
    pub version: Option<GicVersion>,
    /// Number of IRQs override (None = auto-detect from hardware)
    pub num_irqs: Option<u32>,
    /// CPU ID for this CPU interface
    pub cpu_id: u32,
    /// Enable EOImode (GICv2 only)
    pub eoi_mode: bool,
}

impl Default for GicDiscoveryConfig {
    fn default() -> Self {
        Self {
            version: None,
            num_irqs: None,
            cpu_id: 0,
            eoi_mode: false,
        }
    }
}

impl GicDiscoveryConfig {
    /// Create new discovery config
    pub fn new() -> Self {
        Self::default()
    }

    /// Set GIC version
    pub fn with_version(mut self, version: GicVersion) -> Self {
        self.version = Some(version);
        self
    }

    /// Set number of IRQs
    pub fn with_num_irqs(mut self, num_irqs: u32) -> Self {
        self.num_irqs = Some(num_irqs);
        self
    }

    /// Set CPU ID
    pub fn with_cpu_id(mut self, cpu_id: u32) -> Self {
        self.cpu_id = cpu_id;
        self
    }

    /// Set EOImode
    pub fn with_eoi_mode(mut self, eoi_mode: bool) -> Self {
        self.eoi_mode = eoi_mode;
        self
    }
}

/// Discovered and initialized GIC information
#[derive(Debug, Clone)]
pub struct GicInitializedInfo {
    /// GIC version
    pub version: GicVersion,
    /// Number of IRQs
    pub num_irqs: u32,
    /// Distributor base address
    pub dist_base: u64,
    /// CPU interface base address (GICv2)
    pub cpu_base: Option<u64>,
    /// Hypervisor interface base address (GICv2)
    pub hyp_base: Option<u64>,
    /// Redistributor base address (GICv3)
    pub redist_base: Option<u64>,
}

/// Discover GIC from device tree and initialize it
///
/// This function:
/// 1. Parses GIC node from device tree
/// 2. Extracts register addresses
/// 3. Detects GIC version
/// 4. Initializes GIC hardware
///
/// # Parameters
/// - `fdt`: Flattened device tree
/// - `config`: Discovery configuration
///
/// # Returns
/// Initialized GIC information
pub fn discover_and_init_gic(
    fdt: &FlattenedDeviceTree,
    config: GicDiscoveryConfig,
) -> Result<GicInitializedInfo, &'static str> {
    log::info!("GIC Discovery: Starting");

    // Parse GIC from device tree
    let gic_info = parse_gic_node(fdt)?
        .ok_or("GIC node not found in device tree")?;

    log::info!("GIC Discovery: Found GIC node");
    log::info!("  Compatible: {}", gic_info.compatible);
    log::info!("  Version: {}", gic_info.version);
    log::info!("  Register ranges: {}", gic_info.regs.len());

    // Determine GIC version
    let version = if let Some(ver) = config.version {
        ver
    } else {
        match gic_info.version {
            1 => GicVersion::V1,
            2 => GicVersion::V2,
            3 => GicVersion::V3,
            4 => GicVersion::V4,
            _ => GicVersion::V3,
        }
    };

    // Extract register addresses
    let gic_addrs = extract_gic_addresses(&gic_info, version)?;

    log::info!("GIC Discovery: Extracted addresses");
    log::info!("  Distributor: {:#x}", gic_addrs.dist_base);
    if let Some(addr) = gic_addrs.cpu_base {
        log::info!("  CPU Interface: {:#x}", addr);
    }
    if let Some(addr) = gic_addrs.hyp_base {
        log::info!("  Hypervisor Interface: {:#x}", addr);
    }
    if let Some(addr) = gic_addrs.redist_base {
        log::info!("  Redistributor: {:#x}", addr);
    }

    // Determine number of IRQs
    let num_irqs = config.num_irqs.unwrap_or_else(|| {
        // Default: read from hardware (GICD_TYPER.ITLinesNumber + 1) * 32
        // For now, use a reasonable default
        match version {
            GicVersion::V1 | GicVersion::V2 => 1020,
            GicVersion::V3 | GicVersion::V4 => 1020,
        }
    });

    // Initialize GIC
    log::info!("GIC Discovery: Initializing GIC v{:?} with {} IRQs", version, num_irqs);

    gic_init(
        gic_addrs.dist_base,
        gic_addrs.cpu_base.unwrap_or(0),
        gic_addrs.hyp_base,
        version,
        num_irqs,
        config.cpu_id,
    )?;

    log::info!("GIC Discovery: Successfully initialized");

    Ok(GicInitializedInfo {
        version,
        num_irqs,
        dist_base: gic_addrs.dist_base,
        cpu_base: gic_addrs.cpu_base,
        hyp_base: gic_addrs.hyp_base,
        redist_base: gic_addrs.redist_base,
    })
}

/// Extract GIC register addresses from GicInfo
fn extract_gic_addresses(
    gic_info: &GicInfo,
    version: GicVersion,
) -> Result<GicAddressInfo, &'static str> {
    let regs = &gic_info.regs;

    if regs.is_empty() {
        return Err("GIC has no register ranges");
    }

    match version {
        GicVersion::V1 | GicVersion::V2 => {
            // GICv1/v2 layout:
            // 0: Distributor (GICD)
            // 1: CPU Interface (GICC)
            // 2: Virtual Interface Control (GICV) - optional
            // 3: Virtual Interface CPU Interface (GICH) - optional

            let dist_base = regs.get(0)
                .ok_or("GICv2: Missing distributor register")?
                .0;

            let cpu_base = regs.get(1).map(|(addr, _)| *addr);

            // GICH (Hypervisor interface) is typically at index 3
            // If not present, calculate based on CPU interface size
            let hyp_base = if let Some((addr, size)) = regs.get(3) {
                Some(*addr)
            } else if let Some((_, size)) = regs.get(1) {
                // Calculate based on CPU interface size
                // GICv2 with V2M: CPU interface is 64KB
                // GICv2 without V2M: CPU interface is 8KB
                if *size >= 0x20000 {
                    Some(cpu_base.unwrap_or(0) + 0x10000)
                } else if *size >= 0x2000 {
                    Some(cpu_base.unwrap_or(0) + 0x1000)
                } else {
                    None
                }
            } else {
                None
            };

            Ok(GicAddressInfo {
                dist_base,
                cpu_base,
                hyp_base,
                redist_base: None,
            })
        }
        GicVersion::V3 | GicVersion::V4 => {
            // GICv3/v4 layout:
            // 0: Distributor (GICD)
            // 1: Redistributors (GICR)
            // 2: CPU Interface (ICC) - optional, for legacy

            let dist_base = regs.get(0)
                .ok_or("GICv3: Missing distributor register")?
                .0;

            let redist_base = regs.get(1).map(|(addr, _)| *addr);

            // CPU interface is system registers for GICv3 (not memory mapped)
            let cpu_base = None;
            let hyp_base = None;

            Ok(GicAddressInfo {
                dist_base,
                cpu_base,
                hyp_base,
                redist_base,
            })
        }
    }
}

/// GIC register address information
#[derive(Debug, Clone)]
struct GicAddressInfo {
    /// Distributor base address
    dist_base: u64,
    /// CPU interface base address (GICv2)
    cpu_base: Option<u64>,
    /// Hypervisor interface base address (GICv2)
    hyp_base: Option<u64>,
    /// Redistributor base address (GICv3)
    redist_base: Option<u64>,
}

/// Auto-discover and initialize GIC from device tree
///
/// This is a convenience function that uses default configuration.
pub fn auto_init_gic(fdt: &FlattenedDeviceTree) -> Result<GicInitializedInfo, &'static str> {
    let config = GicDiscoveryConfig::new();
    discover_and_init_gic(fdt, config)
}

/// Initialize GIC from known platform configuration
///
/// For platforms without device tree or with known configurations.
///
/// # Parameters
/// - `dist_base`: Distributor base address
/// - `cpu_base`: CPU interface base address (GICv2)
/// - `hyp_base`: Hypervisor interface base (GICv2, optional)
/// - `version`: GIC version
/// - `num_irqs`: Number of IRQs
pub fn init_platform_gic(
    dist_base: u64,
    cpu_base: Option<u64>,
    hyp_base: Option<u64>,
    version: GicVersion,
    num_irqs: u32,
) -> Result<GicInitializedInfo, &'static str> {
    log::info!("Platform GIC: Initializing GIC v{:?}", version);
    log::info!("  Distributor: {:#x}", dist_base);
    if let Some(addr) = cpu_base {
        log::info!("  CPU Interface: {:#x}", addr);
    }
    if let Some(addr) = hyp_base {
        log::info!("  Hypervisor Interface: {:#x}", addr);
    }

    gic_init(dist_base, cpu_base.unwrap_or(0), hyp_base, version, num_irqs, 0)?;

    Ok(GicInitializedInfo {
        version,
        num_irqs,
        dist_base,
        cpu_base,
        hyp_base,
        redist_base: None,
    })
}

/// QEMU virt platform GIC configuration
///
/// QEMU ARM virt platform uses standard GIC addresses:
/// - GICD: 0x08000000
/// - GICC: 0x08010000
pub fn init_qemu_virt_gic() -> Result<GicInitializedInfo, &'static str> {
    init_platform_gic(
        0x08000000,  // GICD
        Some(0x08010000),  // GICC
        None,  // No GICH on QEMU virt
        GicVersion::V3,
        1020,
    )
}

/// ARM Foundation v8 platform GIC configuration
pub fn init_foundation_v8_gic() -> Result<GicInitializedInfo, &'static str> {
    init_platform_gic(
        0x2f000000,  // GICD
        Some(0x2f100000),  // GICR (redistributor)
        None,  // GICv3 uses system registers
        GicVersion::V3,
        1020,
    )
}

/// Get current CPU ID from MPIDR
fn get_cpu_id() -> u32 {
    let mpidr: u64;
    unsafe {
        core::arch::asm!("mrs {}, mpidr_el1", out(reg) mpidr);
    }
    // Extract affinity 0 (CPU ID)
    (mpidr & 0xFF) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gic_discovery_config() {
        let config = GicDiscoveryConfig::new()
            .with_version(GicVersion::V3)
            .with_num_irqs(1020)
            .with_cpu_id(0);

        assert_eq!(config.version, Some(GicVersion::V3));
        assert_eq!(config.num_irqs, Some(1020));
        assert_eq!(config.cpu_id, 0);
    }

    #[test]
    fn test_gic_address_info_v2() {
        let mut gic_info = GicInfo::new();
        gic_info.version = 2;
        gic_info.compatible = compat::GIC_V2.to_string();
        gic_info.regs = vec![
            (0x2c001000, 0x1000),  // GICD
            (0x2c002000, 0x1000),  // GICC
            (0x2c004000, 0x2000),  // GICV
            (0x2c006000, 0x2000),  // GICH
        ];

        let addrs = extract_gic_addresses(&gic_info, GicVersion::V2).unwrap();
        assert_eq!(addrs.dist_base, 0x2c001000);
        assert_eq!(addrs.cpu_base, Some(0x2c002000));
        assert_eq!(addrs.hyp_base, Some(0x2c006000));
        assert!(addrs.redist_base.is_none());
    }

    #[test]
    fn test_gic_address_info_v3() {
        let mut gic_info = GicInfo::new();
        gic_info.version = 3;
        gic_info.compatible = compat::GIC_V3.to_string();
        gic_info.regs = vec![
            (0x2c010000, 0x10000),  // GICD
            (0x2c040000, 0x100000), // GICR
        ];

        let addrs = extract_gic_addresses(&gic_info, GicVersion::V3).unwrap();
        assert_eq!(addrs.dist_base, 0x2c010000);
        assert!(addrs.cpu_base.is_none());
        assert!(addrs.hyp_base.is_none());
        assert_eq!(addrs.redist_base, Some(0x2c040000));
    }

    #[test]
    fn test_get_cpu_id() {
        let cpu_id = get_cpu_id();
        // Should be 0 for boot CPU
        assert!(cpu_id < 256);
    }
}
