//! ARM64 Device Tree (FDT) Support
//!
//! This module provides ARM64-specific device tree functionality including:
//! - ARM device tree parsing (CPU, GIC, Timer nodes)
//! - Virtual device tree generation for VMs
//! - ARM-specific property parsing
//!
//! ## ARM Device Tree Overview
//!
//! ARM device trees describe the hardware configuration of ARM-based systems:
//! - CPU nodes: Describe processor cores and enable methods
//! - GIC nodes: Generic Interrupt Controller configuration
//! - Timer nodes: ARM Generic Timer configuration
//! - Memory nodes: Physical memory layout
//!
//! ## Virtual Device Tree
//!
//! For guest VMs, we generate virtual device trees that present:
//! - Virtual GIC (VGIC) configuration
//! - Virtual Timer configuration
//! - Virtual CPU topology
//! - Emulated devices
//!
//! ## References
//! - [Device Tree Specification](https://www.devicetree.org/)
//! - [ARM Device Tree Documentation](https://www.kernel.org/doc/Documentation/devicetree/bindings/arm/)
//! - [Xvisor Device Tree](https://github.com/xvisor/xvisor)

pub mod parse;
pub mod vm_fdt;

// Re-export key types and functions
pub use parse::*;
pub use vm_fdt::*;

/// ARM device tree compatible strings
pub mod compat {
    /// ARM GICv1 interrupt controller
    pub const GIC_V1: &str = "arm,gic-400";
    /// ARM GICv2 interrupt controller
    pub const GIC_V2: &str = "arm,gic-400";
    /// ARM GICv3 interrupt controller
    pub const GIC_V3: &str = "arm,gic-v3";
    /// ARM GICv4 interrupt controller
    pub const GIC_V4: &str = "arm,gic-v4";

    /// ARM Generic Timer
    pub const ARM_TIMER: &str = "arm,armv8-timer";
    /// ARMv7 Generic Timer
    pub const ARM_TIMER_V7: &str = "arm,armv7-timer";

    /// ARM CPU
    pub const ARM_CPU: &str = "arm,armv8";

    /// ARM PL011 UART
    pub const PL011_UART: &str = "arm,pl011";

    /// VirtIO devices
    pub const VIRTIO_MMIO: &str = "virtio,mmio";
}

/// ARM device tree property names
pub mod props {
    /// CPU enable method
    pub const ENABLE_METHOD: &str = "enable-method";
    /// CPU release address (for spin-table)
    pub const CPU_RELEASE_ADDR: &str = "cpu-release-addr";
    /// CPU capacity
    pub const CAPACITY_DMHZ: &str = "capacity-dmips-mhz";

    /// GIC registers
    pub const REG: &str = "reg";
    /// Interrupt specifier
    pub const INTERRUPTS: &str = "interrupts";

    /// Clock frequency
    pub const CLOCK_FREQUENCY: &str = "clock-frequency";

    /// Device type
    pub const DEVICE_TYPE: &str = "device_type";
    /// Device type values
    pub const DEV_TYPE_CPU: &str = "cpu";
    pub const DEV_TYPE_MEMORY: &str = "memory";

    /// Memory reg property
    pub const MEMORY_REG: &str = "reg";

    /// Status
    pub const STATUS: &str = "status";
    /// Status values
    pub const STATUS_OK: &str = "okay";
    pub const STATUS_DISABLED: &str = "disabled";
    pub const STATUS_FAIL: &str = "fail";
}

/// ARM CPU enable methods
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuEnableMethod {
    /// Spin table method (secondary CPUs spin on a flag)
    SpinTable,
    /// PSCI (Power State Coordination Interface)
    Psci,
    /// ARM-specific method
    Arm,
    /// Unknown method
    Unknown,
}

impl CpuEnableMethod {
    /// Parse from string
    pub fn from_str(s: &str) -> Self {
        match s {
            "spin-table" => CpuEnableMethod::SpinTable,
            "psci" => CpuEnableMethod::Psci,
            "arm" => CpuEnableMethod::Arm,
            _ => CpuEnableMethod::Unknown,
        }
    }

    /// Convert to string
    pub fn as_str(&self) -> &'static str {
        match self {
            CpuEnableMethod::SpinTable => "spin-table",
            CpuEnableMethod::Psci => "psci",
            CpuEnableMethod::Arm => "arm",
            CpuEnableMethod::Unknown => "unknown",
        }
    }
}

/// ARM CPU information from device tree
#[derive(Debug, Clone)]
pub struct CpuInfo {
    /// CPU node name (e.g., "cpu@0")
    pub node_name: String,
    /// CPU number (logical ID)
    pub cpu_id: u32,
    /// CPU MPIDR (Multiprocessor ID Register)
    pub mpidr: u64,
    /// Enable method
    pub enable_method: CpuEnableMethod,
    /// CPU release address (for spin-table)
    pub release_addr: Option<u64>,
    /// CPU capacity (DMIPS per MHz)
    pub capacity: Option<u32>,
    /// Clock frequency
    pub clock_frequency: Option<u64>,
}

impl CpuInfo {
    /// Create new CPU info
    pub fn new(cpu_id: u32, mpidr: u64) -> Self {
        Self {
            node_name: format!("cpu@{}", cpu_id),
            cpu_id,
            mpidr,
            enable_method: CpuEnableMethod::Unknown,
            release_addr: None,
            capacity: None,
            clock_frequency: None,
        }
    }

    /// Get CPU node path
    pub fn path(&self) -> String {
        format!("/cpus/{}", self.node_name)
    }

    /// Check if this is the boot CPU
    pub fn is_boot_cpu(&self) -> bool {
        self.cpu_id == 0
    }
}

/// GIC information from device tree
#[derive(Debug, Clone)]
pub struct GicInfo {
    /// GIC node name
    pub node_name: String,
    /// GIC version
    pub version: u32,
    /// GIC compatible string
    pub compatible: String,
    /// GIC register ranges
    pub regs: Vec<(u64, u64)>,
    /// GIC interrupts (maintenance interrupts)
    pub interrupts: Vec<u32>,
    /// Number of interrupts
    pub num_irqs: u32,
}

impl GicInfo {
    /// Create new GIC info
    pub fn new() -> Self {
        Self {
            node_name: "interrupt-controller".to_string(),
            version: 3,
            compatible: compat::GIC_V3.to_string(),
            regs: Vec::new(),
            interrupts: Vec::new(),
            num_irqs: 0,
        }
    }

    /// Get GIC node path
    pub fn path(&self) -> String {
        format!("/{}", self.node_name)
    }

    /// Check if GICv3 or later
    pub fn is_v3_or_later(&self) -> bool {
        self.version >= 3
    }

    /// Get distributor address
    pub fn dist_addr(&self) -> Option<u64> {
        self.regs.first().map(|(addr, _)| *addr)
    }

    /// Get redistributor address (GICv3)
    pub fn redistributor_addr(&self) -> Option<u64> {
        if self.regs.len() > 1 {
            Some(self.regs[1].0)
        } else {
            None
        }
    }
}

/// ARM Generic Timer information from device tree
#[derive(Debug, Clone)]
pub struct TimerInfo {
    /// Timer node name
    pub node_name: String,
    /// Timer compatible string
    pub compatible: String,
    /// Timer interrupts (SECURE, NON_SECURE, VIRT, HYP)
    pub interrupts: Vec<u32>,
    /// Clock frequency
    pub clock_frequency: Option<u64>,
}

impl TimerInfo {
    /// Create new timer info
    pub fn new() -> Self {
        Self {
            node_name: "timer".to_string(),
            compatible: compat::ARM_TIMER.to_string(),
            interrupts: Vec::new(),
            clock_frequency: None,
        }
    }

    /// Get timer node path
    pub fn path(&self) -> String {
        format!("/{}", self.node_name)
    }

    /// Get secure timer IRQ
    pub fn secure_irq(&self) -> Option<u32> {
        self.interrupts.get(0).copied()
    }

    /// Get non-secure timer IRQ
    pub fn ns_irq(&self) -> Option<u32> {
        self.interrupts.get(1).copied()
    }

    /// Get virtual timer IRQ
    pub fn virt_irq(&self) -> Option<u32> {
        self.interrupts.get(2).copied()
    }

    /// Get hypervisor timer IRQ
    pub fn hyp_irq(&self) -> Option<u32> {
        self.interrupts.get(3).copied()
    }
}

/// ARM memory information from device tree
#[derive(Debug, Clone)]
pub struct MemInfo {
    /// Memory node name
    pub node_name: String,
    /// Memory address
    pub base: u64,
    /// Memory size
    pub size: u64,
}

impl MemInfo {
    /// Create new memory info
    pub fn new(base: u64, size: u64) -> Self {
        Self {
            node_name: format!("memory@{:x}", base),
            base,
            size,
        }
    }

    /// Get memory node path
    pub fn path(&self) -> String {
        format!("/{}", self.node_name)
    }

    /// Get end address (exclusive)
    pub fn end(&self) -> u64 {
        self.base + self.size
    }

    /// Check if address is within this memory region
    pub fn contains(&self, addr: u64) -> bool {
        addr >= self.base && addr < self.end()
    }
}

/// Initialize ARM64 device tree support
pub fn init() -> Result<(), &'static str> {
    log::info!("ARM64 Device Tree: Initializing");

    // Parse device tree and extract hardware info
    if let Err(e) = parse::parse_device_tree() {
        log::warn!("ARM64 Device Tree: Failed to parse: {}", e);
        // Don't fail - device tree may not be available in all environments
        return Ok(());
    }

    log::info!("ARM64 Device Tree: Initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_enable_method() {
        assert_eq!(CpuEnableMethod::from_str("spin-table"), CpuEnableMethod::SpinTable);
        assert_eq!(CpuEnableMethod::from_str("psci"), CpuEnableMethod::Psci);
        assert_eq!(CpuEnableMethod::from_str("arm"), CpuEnableMethod::Arm);
        assert_eq!(CpuEnableMethod::from_str("unknown"), CpuEnableMethod::Unknown);
    }

    #[test]
    fn test_cpu_info() {
        let cpu = CpuInfo::new(0, 0x80000000);
        assert_eq!(cpu.cpu_id, 0);
        assert_eq!(cpu.mpidr, 0x80000000);
        assert!(cpu.is_boot_cpu());
        assert_eq!(cpu.path(), "/cpus/cpu@0");
    }

    #[test]
    fn test_gic_info() {
        let mut gic = GicInfo::new();
        gic.regs.push((0x2f000000, 0x10000));
        gic.regs.push((0x2f100000, 0x200000));
        assert_eq!(gic.dist_addr(), Some(0x2f000000));
        assert_eq!(gic.redistributor_addr(), Some(0x2f100000));
        assert!(gic.is_v3_or_later());
    }

    #[test]
    fn test_timer_info() {
        let mut timer = TimerInfo::new();
        timer.interrupts.push(13);  // Secure
        timer.interrupts.push(14);  // Non-secure
        timer.interrupts.push(11);  // Virtual
        timer.interrupts.push(10);  // Hyp
        assert_eq!(timer.secure_irq(), Some(13));
        assert_eq!(timer.virt_irq(), Some(11));
        assert_eq!(timer.hyp_irq(), Some(10));
    }

    #[test]
    fn test_mem_info() {
        let mem = MemInfo::new(0x80000000, 0x10000000);
        assert_eq!(mem.base, 0x80000000);
        assert_eq!(mem.size, 0x10000000);
        assert_eq!(mem.end(), 0x90000000);
        assert!(mem.contains(0x80000000));
        assert!(mem.contains(0x8fffffff));
        assert!(!mem.contains(0x90000000));
    }
}
