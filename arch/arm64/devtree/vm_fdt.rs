//! Virtual Device Tree Generation for ARM64 VMs
//!
//! This module provides functionality to generate virtual device trees
//! for guest virtual machines:
//! - Virtual GIC device node
//! - Virtual Timer device node
//! - Virtual CPU topology
//! - Virtual memory layout
//! - Emulated devices
//!
//! ## Virtual Device Tree Structure
//!
//! The generated device tree for a VM includes:
//! - `/cpus` - Virtual CPU nodes
//! - `/interrupt-controller` - Virtual GIC
//! - `/timer` - Virtual Generic Timer
//! - `/memory` - Guest physical memory
//! - `/chosen` - Boot arguments
//! - `/virtio*` - VirtIO devices (optional)
//!
//! ## References
//! - [ARM Device Tree Specification](https://www.devicetree.org/)
//! - [Xvisor VM Device Tree](https://github.com/xvisor/xvisor)

use super::{CpuInfo, GicInfo, TimerInfo, MemInfo, compat, props};
use crate::arch::riscv64::devtree::fdt::{Node, Property, FlattenedDeviceTree};

/// Virtual device tree configuration
#[derive(Debug, Clone)]
pub struct VmFdtConfig {
    /// Number of VCPUs
    pub num_vcpus: usize,
    /// Guest physical memory base
    pub mem_base: u64,
    /// Guest physical memory size
    pub mem_size: u64,
    /// GIC version (2 or 3)
    pub gic_version: u32,
    /// GIC base address (guest physical)
    pub gic_base: u64,
    /// GIC redistributor address (for GICv3)
    pub gic_redist_base: Option<u64>,
    /// Boot arguments
    pub bootargs: Option<String>,
    /// Include VirtIO devices
    pub virtio_enabled: bool,
    /// Number of VirtIO devices
    pub num_virtio: usize,
    /// UART base address
    pub uart_base: Option<u64>,
}

impl Default for VmFdtConfig {
    fn default() -> Self {
        Self {
            num_vcpus: 1,
            mem_base: 0x40000000,
            mem_size: 0x20000000, // 512 MB
            gic_version: 3,
            gic_base: 0x08000000,
            gic_redist_base: Some(0x080A0000),
            bootargs: None,
            virtio_enabled: false,
            num_virtio: 0,
            uart_base: Some(0x09000000),
        }
    }
}

impl VmFdtConfig {
    /// Create new VM FDT config
    pub fn new(num_vcpus: usize, mem_base: u64, mem_size: u64) -> Self {
        Self {
            num_vcpus,
            mem_base,
            mem_size,
            ..Self::default()
        }
    }

    /// Set GIC version
    pub fn gic_version(mut self, version: u32) -> Self {
        self.gic_version = version;
        self
    }

    /// Set GIC base addresses
    pub fn gic_addrs(mut self, dist: u64, redist: Option<u64>) -> Self {
        self.gic_base = dist;
        self.gic_redist_base = redist;
        self
    }

    /// Set boot arguments
    pub fn bootargs(mut self, args: &str) -> Self {
        self.bootargs = Some(args.to_string());
        self
    }

    /// Enable VirtIO devices
    pub fn virtio(mut self, enabled: bool, count: usize) -> Self {
        self.virtio_enabled = enabled;
        self.num_virtio = count;
        self
    }

    /// Set UART address
    pub fn uart(mut self, addr: u64) -> Self {
        self.uart_base = Some(addr);
        self
    }
}

/// Generate virtual device tree for VM
pub fn generate_vm_fdt(config: &VmFdtConfig) -> Result<FlattenedDeviceTree, &'static str> {
    log::info!("Generating VM device tree: {} VCPUs, {} MB memory",
              config.num_vcpus, config.mem_size / (1024 * 1024));

    // Create root node
    let mut root = Node::new("", 0);
    root.add_property(Property::new("#address-cells", vec![0, 0, 0, 2]));
    root.add_property(Property::new("#size-cells", vec![0, 0, 0, 2]));
    root.add_property(Property::new("model", b"Ferrovisor ARM64 Virtual Machine\0"));

    // Add interrupt-parent property (point to GIC)
    let gic_phandle: u32 = 1;
    root.add_property(Property::new("interrupt-parent", gic_phandle.to_be_bytes().to_vec()));

    // Create /cpus node
    let cpus_node = create_cpus_node(config, gic_phandle)?;
    root.children.push(cpus_node);

    // Create /memory node
    let memory_node = create_memory_node(config)?;
    root.children.push(memory_node);

    // Create GIC node
    let gic_node = create_gic_node(config, gic_phandle)?;
    root.children.push(gic_node);

    // Create timer node
    let timer_node = create_timer_node(config)?;
    root.children.push(timer_node);

    // Create /chosen node
    let chosen_node = create_chosen_node(config)?;
    root.children.push(chosen_node);

    // Create UART node (if enabled)
    if let Some(uart_addr) = config.uart_base {
        let uart_node = create_uart_node(uart_addr)?;
        root.children.push(uart_node);
    }

    // Create VirtIO device nodes (if enabled)
    if config.virtio_enabled {
        for i in 0..config.num_virtio {
            let virtio_node = create_virtio_node(i)?;
            root.children.push(virtio_node);
        }
    }

    // Create FDT structure
    // Note: FDT header requires proper values, using placeholder
    use crate::arch::riscv64::devtree::fdt::FdtHeader;
    let fdt = FlattenedDeviceTree {
        data: Vec::new(), // Would need proper serialization
        header: FdtHeader {
            magic: 0xd00dfeed,
            totalsize: 0,
            off_dt_struct: 0,
            off_dt_strings: 0,
            off_mem_rsvmap: 0,
            version: 17,
            last_comp_version: 16,
            boot_cpuid_phys: 0,
            size_dt_strings: 0,
            size_dt_struct: 0,
        },
        root: Some(root),
        mem_reserve: Vec::new(),
    };

    log::info!("VM device tree generated successfully");
    Ok(fdt)
}

/// Create /cpus node for VM
fn create_cpus_node(config: &VmFdtConfig, gic_phandle: u32) -> Result<Node, &'static str> {
    let mut cpus = Node::new("cpus", 1);
    cpus.add_property(Property::new("#address-cells", vec![0, 0, 0, 1]));
    cpus.add_property(Property::new("#size-cells", vec![0, 0, 0, 0]));

    // Create CPU nodes
    for i in 0..config.num_vcpus {
        let mut cpu = Node::new(&format!("cpu@{}", i), 2);
        let cpu_id = i as u32;

        // CPU device type
        cpu.add_property(Property::new(props::DEVICE_TYPE, b"cpu\0"));
        cpu.add_property(Property::new("compatible", b"arm,armv8\0"));

        // CPU reg (MPIDR)
        let mpidr = if i == 0 {
            0x80000000u64  // Boot CPU
        } else {
            0x80000000u64 | (i as u64)  // Other CPUs
        };
        cpu.add_property(Property::new("reg", mpidr.to_be_bytes().to_vec()));

        // CPU enable-method (PSCI)
        cpu.add_property(Property::new(props::ENABLE_METHOD, b"psci\0"));

        // CPU next-level-cache (optional)
        // cpu.add_property(Property::new("next-level-cache", vec![0, 0, 0, 1]));

        // CPU interrupts (PPI)
        // Format: (type, hw_irq, flags)
        // type=1 (PPI), hw_irq=14 (timer IRQ), flags=8 (level-high)
        let mut irq_data: Vec<u8> = Vec::new();
        irq_data.extend_from_slice(&1u32.to_be_bytes()); // PPI
        irq_data.extend_from_slice(&14u32.to_be_bytes()); // Timer PPI
        irq_data.extend_from_slice(&8u32.to_be_bytes()); // Level-high
        cpu.add_property(Property::new("interrupts", irq_data));

        cpus.children.push(cpu);
    }

    // Create cpu-map node (for CPU topology)
    let mut cpu_map = Node::new("cpu-map", 2);

    // Create cluster node
    let mut cluster = Node::new("cluster0", 3);
    cluster.add_property(Property::new("core-map", vec![]));

    // Add core nodes
    for i in 0..config.num_vcpus {
        let mut core = Node::new(&format!("core{}", i), 4);
        let cpu_id = i as u32;
        core.add_property(Property::new("cpu", cpu_id.to_be_bytes().to_vec()));
        cluster.children.push(core);
    }

    cpu_map.children.push(cluster);
    cpus.children.push(cpu_map);

    Ok(cpus)
}

/// Create /memory node for VM
fn create_memory_node(config: &VmFdtConfig) -> Result<Node, &'static str> {
    let mut memory = Node::new(&format!("memory@{:x}", config.mem_base), 1);
    memory.add_property(Property::new(props::DEVICE_TYPE, b"memory\0"));

    // Memory reg property (base + size)
    let mut reg_data: Vec<u8> = Vec::new();
    reg_data.extend_from_slice(&config.mem_base.to_be_bytes());
    reg_data.extend_from_slice(&config.mem_size.to_be_bytes());
    memory.add_property(Property::new(props::REG, reg_data));

    Ok(memory)
}

/// Create GIC node for VM
fn create_gic_node(config: &VmFdtConfig, phandle: u32) -> Result<Node, &'static str> {
    let mut gic = Node::new("interrupt-controller", 1);
    let gic_phandle: u32 = 1;

    // GIC compatible string
    let compatible = if config.gic_version >= 3 {
        compat::GIC_V3
    } else {
        compat::GIC_V2
    };
    gic.add_property(Property::new("compatible", compatible.as_bytes()));

    // Interrupt controller
    gic.add_property(Property::new("interrupt-controller", vec![]));

    // #interrupt-cells
    gic.add_property(Property::new("#interrupt-cells", vec![0, 0, 0, 3]));

    // phandle
    gic.add_property(Property::new("phandle", gic_phandle.to_be_bytes().to_vec()));

    // GIC reg property
    let mut reg_data: Vec<u8> = Vec::new();

    if config.gic_version >= 3 {
        // GICv3 registers:
        // - Distributor: 64KB
        // - Redistributor: 2MB per CPU
        reg_data.extend_from_slice(&config.gic_base.to_be_bytes());
        reg_data.extend_from_slice(&0x10000u64.to_be_bytes()); // 64KB distributor

        if let Some(redist_base) = config.gic_redist_base {
            reg_data.extend_from_slice(&redist_base.to_be_bytes());
            let redist_size = 0x20000u64 * config.num_vcpus as u64;
            reg_data.extend_from_slice(&redist_size.to_be_bytes());
        }
    } else {
        // GICv2 registers:
        // - Distributor + CPU interface
        reg_data.extend_from_slice(&config.gic_base.to_be_bytes());
        reg_data.extend_from_slice(&0x10000u64.to_be_bytes()); // 64KB distributor

        reg_data.extend_from_slice(&(config.gic_base + 0x10000).to_be_bytes());
        reg_data.extend_from_slice(&0x10000u64.to_be_bytes()); // 64KB CPU interface
    }

    gic.add_property(Property::new(props::REG, reg_data));

    // GIC interrupts (maintenance interrupts)
    // GICv3: uses system registers for CPU interface
    if config.gic_version < 3 {
        let mut irq_data: Vec<u8> = Vec::new();
        irq_data.extend_from_slice(&1u32.to_be_bytes()); // PPI
        irq_data.extend_from_slice(&9u32.to_be_bytes());  // IRQ 9
        irq_data.extend_from_slice(&4u32.to_be_bytes());  // Edge-triggered
        gic.add_property(Property::new(props::INTERRUPTS, irq_data));
    }

    Ok(gic)
}

/// Create Timer node for VM
fn create_timer_node(config: &VmFdtConfig) -> Result<Node, &'static str> {
    let mut timer = Node::new("timer", 1);

    // ARM Generic Timer compatible
    timer.add_property(Property::new("compatible", compat::ARM_TIMER.as_bytes()));

    // Timer interrupts
    // Format: <SEC_PPI SEC_NS_PPI VIRT_PPI HYP_PPI>
    let mut irq_data: Vec<u8> = Vec::new();

    // Secure timer IRQ (13)
    irq_data.extend_from_slice(&1u32.to_be_bytes()); // PPI
    irq_data.extend_from_slice(&13u32.to_be_bytes());
    irq_data.extend_from_slice(&8u32.to_be_bytes()); // Level-high

    // Non-secure timer IRQ (14)
    irq_data.extend_from_slice(&1u32.to_be_bytes()); // PPI
    irq_data.extend_from_slice(&14u32.to_be_bytes());
    irq_data.extend_from_slice(&8u32.to_be_bytes()); // Level-high

    // Virtual timer IRQ (11)
    irq_data.extend_from_slice(&1u32.to_be_bytes()); // PPI
    irq_data.extend_from_slice(&11u32.to_be_bytes());
    irq_data.extend_from_slice(&8u32.to_be_bytes()); // Level-high

    // Hypervisor timer IRQ (10)
    irq_data.extend_from_slice(&1u32.to_be_bytes()); // PPI
    irq_data.extend_from_slice(&10u32.to_be_bytes());
    irq_data.extend_from_slice(&8u32.to_be_bytes()); // Level-high

    timer.add_property(Property::new(props::INTERRUPTS, irq_data));

    // Clock frequency (optional - use CNTFRQ_EL0)
    // timer.add_property(Property::new(props::CLOCK_FREQUENCY, ...));

    // Always-on property
    timer.add_property(Property::new("always-on", vec![]));

    Ok(timer)
}

/// Create /chosen node for VM
fn create_chosen_node(config: &VmFdtConfig) -> Result<Node, &'static str> {
    let mut chosen = Node::new("chosen", 1);

    // Boot arguments
    if let Some(ref bootargs) = config.bootargs {
        chosen.add_property(Property::new("bootargs", bootargs.as_bytes()));
    } else {
        // Default bootargs
        let default_bootargs = format!(
            "console=ttyAMA0 earlycon=pl011,0x{:x} root=/dev/vda rw",
            config.uart_base.unwrap_or(0x09000000)
        );
        chosen.add_property(Property::new("bootargs", default_bootargs.as_bytes()));
    }

    Ok(chosen)
}

/// Create UART node for VM
fn create_uart_node(base_addr: u64) -> Result<Node, &'static str> {
    let mut uart = Node::new(&format!("uart@{:x}", base_addr), 1);

    // PL011 UART compatible
    uart.add_property(Property::new("compatible", compat::PL011_UART.as_bytes()));

    // UART registers
    let mut reg_data: Vec<u8> = Vec::new();
    reg_data.extend_from_slice(&base_addr.to_be_bytes());
    reg_data.extend_from_slice(&0x1000u64.to_be_bytes()); // 4KB register space
    uart.add_property(Property::new(props::REG, reg_data));

    // UART interrupts
    let mut irq_data: Vec<u8> = Vec::new();
    irq_data.extend_from_slice(&1u32.to_be_bytes()); // PPI
    irq_data.extend_from_slice(&1u32.to_be_bytes());  // UART IRQ 1
    irq_data.extend_from_slice(&8u32.to_be_bytes()); // Level-high
    uart.add_property(Property::new(props::INTERRUPTS, irq_data));

    // Clock properties
    uart.add_property(Property::new("clocks", vec![0, 0, 0, 2]));
    uart.add_property(Property::new("clock-names", b"uartclk\0"));

    // Status
    uart.add_property(Property::new(props::STATUS, props::STATUS_OK.as_bytes()));

    Ok(uart)
}

/// Create VirtIO MMIO device node
fn create_virtio_node(index: usize) -> Result<Node, &'static str> {
    let base_addr = 0x0A000000 + (index as u64 * 0x1000);
    let mut virtio = Node::new(&format!("virtio@{:x}", base_addr), 1);

    // VirtIO MMIO compatible
    virtio.add_property(Property::new("compatible", compat::VIRTIO_MMIO.as_bytes()));

    // Registers
    let mut reg_data: Vec<u8> = Vec::new();
    reg_data.extend_from_slice(&base_addr.to_be_bytes());
    reg_data.extend_from_slice(&0x1000u64.to_be_bytes());
    virtio.add_property(Property::new(props::REG, reg_data));

    // Interrupts
    let mut irq_data: Vec<u8> = Vec::new();
    irq_data.extend_from_slice(&0u32.to_be_bytes()); // SPI
    irq_data.extend_from_slice(&(32 + index as u32).to_be_bytes()); // SPI starting from 32
    irq_data.extend_from_slice(&1u32.to_be_bytes()); // Edge-triggered
    virtio.add_property(Property::new(props::INTERRUPTS, irq_data));

    // Status
    virtio.add_property(Property::new(props::STATUS, props::STATUS_OK.as_bytes()));

    Ok(virtio)
}

/// Create PSCI node (if PSCI is enabled)
pub fn create_psci_node() -> Result<Node, &'static str> {
    let mut psci = Node::new("psci", 1);

    // PSCI version
    psci.add_property(Property::new("compatible", b"arm,psci-1.0\0"));
    psci.add_property(Property::new("method", b"smc\0"));

    // PSCI function IDs (for SMC calling convention)
    // These are standard PSCI v1.0 function IDs
    psci.add_property(Property::new("cpu_suspend", u32::to_be_bytes(0xC4000001).to_vec()));
    psci.add_property(Property::new("cpu_off", u32::to_be_bytes(0x84000002).to_vec()));
    psci.add_property(Property::new("cpu_on", u32::to_be_bytes(0xC4000003).to_vec()));
    psci.add_property(Property::new("migrate", u32::to_be_bytes(0xC4000005).to_vec()));

    Ok(psci)
}

/// Serialize device tree to FDT format
///
/// This function serializes a Node tree to the flattened device tree format.
/// Note: This is a simplified implementation - a complete implementation would
/// need proper FDT structure generation.
pub fn serialize_fdt(root: &Node) -> Result<Vec<u8>, &'static str> {
    // This would properly serialize the tree to FDT format
    // For now, return a placeholder
    let mut fdt_data = Vec::new();

    // FDT header (magic number)
    fdt_data.extend_from_slice(&0xd00dfeedu32.to_be_bytes());

    // This is a simplified placeholder
    // A complete implementation would:
    // 1. Write FDT header with proper offsets
    // 2. Write memory reserve map
    // 3. Write structure block (nodes and properties)
    // 4. Write strings block

    log::warn!("FDT serialization not fully implemented");
    Ok(fdt_data)
}

/// Calculate required FDT size for configuration
pub fn calculate_fdt_size(config: &VmFdtConfig) -> usize {
    // Base size + nodes
    let mut size = 4096; // Base FDT size

    // CPU nodes
    size += config.num_vcpus * 256;

    // Memory nodes
    size += 256;

    // GIC node
    size += 512;

    // Timer node
    size += 256;

    // Chosen node
    size += 256;

    // UART node (if enabled)
    if config.uart_base.is_some() {
        size += 256;
    }

    // VirtIO nodes
    if config.virtio_enabled {
        size += config.num_virtio * 256;
    }

    // Add safety margin
    size * 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_fdt_config_default() {
        let config = VmFdtConfig::default();
        assert_eq!(config.num_vcpus, 1);
        assert_eq!(config.mem_base, 0x40000000);
        assert_eq!(config.mem_size, 0x20000000);
        assert_eq!(config.gic_version, 3);
    }

    #[test]
    fn test_vm_fdt_config_builder() {
        let config = VmFdtConfig::new(4, 0x80000000, 0x40000000)
            .gic_version(2)
            .gic_addrs(0x2f000000, None)
            .bootargs("console=ttyAMA0")
            .virtio(true, 4);

        assert_eq!(config.num_vcpus, 4);
        assert_eq!(config.mem_size, 0x40000000);
        assert_eq!(config.gic_version, 2);
        assert!(config.virtio_enabled);
        assert_eq!(config.num_virtio, 4);
    }

    #[test]
    fn test_calculate_fdt_size() {
        let config = VmFdtConfig::new(2, 0x40000000, 0x20000000)
            .virtio(true, 4);

        let size = calculate_fdt_size(&config);
        assert!(size > 4096);
    }

    #[test]
    fn test_create_timer_node() {
        let config = VmFdtConfig::default();
        let timer = create_timer_node(&config).unwrap();
        assert_eq!(timer.name, "timer");
        assert!(timer.get_property("compatible").is_some());
        assert!(timer.get_property("interrupts").is_some());
    }

    #[test]
    fn test_create_memory_node() {
        let config = VmFdtConfig::new(1, 0x80000000, 0x10000000);
        let memory = create_memory_node(&config).unwrap();
        assert_eq!(memory.name, "memory@80000000");
        assert!(memory.get_property("reg").is_some());
    }
}
