//! RISC-V Device Tree Module
//!
//! This module provides comprehensive device tree handling for RISC-V including:
//! - FDT (Flattened Device Tree) parsing and generation
//! - Device tree modification and manipulation
//! - Virtual device tree generation for VMs
//! - Hardware discovery and configuration
//! - Address space and interrupt mapping

pub mod fdt;
pub mod parser;
pub mod modifier;

use crate::arch::riscv64::*;
use fdt::FlattenedDeviceTree;
use parser::DeviceTreeParser;
use modifier::DeviceTreeModifier;

/// Global device tree instance
static mut BOOT_FDT: Option<FlattenedDeviceTree> = None;
static mut FDT_PARSER: Option<DeviceTreeParser> = None;

/// Initialize device tree handling
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing RISC-V device tree handling");

    // Device tree will be initialized when FDT is loaded
    log::info!("RISC-V device tree handling initialized");
    Ok(())
}

/// Load and parse boot device tree
pub fn load_boot_fdt(fdt_addr: usize) -> Result<(), &'static str> {
    log::info!("Loading boot device tree from address {:#x}", fdt_addr);

    // Validate FDT address
    if fdt_addr == 0 || !crate::arch::riscv64::mmu::is_valid_address(fdt_addr) {
        return Err("Invalid FDT address");
    }

    // Parse FDT from memory
    let fdt = FlattenedDeviceTree::from_memory(fdt_addr)
        .map_err(|_| "Failed to parse FDT")?;

    // Validate FDT
    fdt.validate()
        .map_err(|_| "Invalid FDT format")?;

    // Create parser
    let parser = DeviceTreeParser::new_default(fdt.clone());

    // Store globally
    unsafe {
        BOOT_FDT = Some(fdt);
        FDT_PARSER = Some(parser);
    }

    log::info!("Boot device tree loaded successfully");
    Ok(())
}

/// Get boot FDT
pub fn get_boot_fdt() -> Option<&'static FlattenedDeviceTree> {
    unsafe { BOOT_FDT.as_ref() }
}

/// Get FDT parser
pub fn get_fdt_parser() -> Option<&'static DeviceTreeParser> {
    unsafe { FDT_PARSER.as_ref() }
}

/// Create virtual device tree for VM
pub fn create_vm_fdt(vm_id: u32, config: &VmFdtConfig) -> Result<Vec<u8>, &'static str> {
    log::info!("Creating virtual device tree for VM {}", vm_id);

    // Get boot FDT as base
    let base_fdt = get_boot_fdt()
        .ok_or("No boot FDT loaded")?;

    // Create modifier
    let mut modifier = DeviceTreeModifier::new(base_fdt.clone());

    // Configure for VM
    modifier.configure_for_vm(vm_id, config)?;

    // Generate virtual FDT
    let vm_fdt = modifier.generate()
        .map_err(|_| "Failed to generate VM FDT")?;

    log::info!("Virtual device tree created for VM {}", vm_id);
    Ok(vm_fdt)
}

/// Find device node by path
pub fn find_node(path: &str) -> Option<&'static fdt::Node> {
    get_fdt_parser()?.find_node(path)
}

/// Find device node by compatible string
pub fn find_compatible(compatible: &str) -> Option<&'static fdt::Node> {
    get_fdt_parser()?.find_node_by_compatible(compatible)
}

/// Get CPU information from device tree
pub fn get_cpu_info() -> Vec<CpuInfo> {
    let mut cpus = Vec::new();
    let parser = get_fdt_parser();

    if let Some(cpus_node) = parser.and_then(|p| p.find_node("/cpus")) {
        for child in &cpus_node.children {
            if child.name.starts_with("cpu@") {
                if let Some(cpu_info) = parse_cpu_node(child) {
                    cpus.push(cpu_info);
                }
            }
        }
    }

    cpus
}

/// Get memory information from device tree
pub fn get_memory_info() -> Vec<MemoryRegion> {
    let mut memory = Vec::new();
    let parser = get_fdt_parser();

    if let Some(mem_node) = parser.and_then(|p| p.find_node("/memory")) {
        if let Some(regs) = parser.map(|p| p.parse_reg(mem_node)) {
            for reg in regs {
                memory.push(MemoryRegion {
                    address: reg.address,
                    size: reg.size,
                });
            }
        }
    }

    memory
}

/// Get interrupt controller information
pub fn get_interrupt_info() -> Option<InterruptController> {
    let parser = get_fdt_parser();

    // Try to find PLIC (Platform-Level Interrupt Controller)
    if let Some(plic_node) = parser.and_then(|p| p.find_compatible("riscv,plic0")) {
        return parse_interrupt_controller(plic_node, InterruptControllerType::PLIC);
    }

    // Try to find ACLINT (Core-Local Interrupt Controller)
    if let Some(aclint_node) = parser.and_then(|p| p.find_compatible("riscv,aclint")) {
        return parse_interrupt_controller(aclint_node, InterruptControllerType::ACLINT);
    }

    None
}

/// Get timer information
pub fn get_timer_info() -> Vec<TimerInfo> {
    let mut timers = Vec::new();
    let parser = get_fdt_parser();

    // Look for PLIC-based timers
    let compatible_strings = ["riscv,timer", "sifive,clint", "sifive,aclint"];

    for compatible in &compatible_strings {
        if let Some(timer_node) = parser.and_then(|p| p.find_compatible(compatible)) {
            if let Some(timer_info) = parse_timer_node(timer_node) {
                timers.push(timer_info);
            }
        }
    }

    timers
}

/// Parse CPU node
fn parse_cpu_node(node: &fdt::Node) -> Option<CpuInfo> {
    let parser = get_fdt_parser()?;

    let cpu_id = node.name.strip_prefix("cpu@")?
        .parse::<u32>()
        .ok()?;

    let compatible = parser.get_compatible(node)
        .first()?
        .clone();

    let clock_frequency = parser.get_clock_frequency(node);

    Some(CpuInfo {
        cpu_id,
        compatible,
        clock_frequency,
    })
}

/// Parse interrupt controller node
fn parse_interrupt_controller(node: &fdt::Node, ctrl_type: InterruptControllerType) -> Option<InterruptController> {
    let parser = get_fdt_parser()?;

    let regs = parser.parse_reg(node);
    let interrupt_cells = parser.get_interrupt_cells(node);

    Some(InterruptController {
        ctrl_type,
        regs,
        interrupt_cells,
    })
}

/// Parse timer node
fn parse_timer_node(node: &fdt::Node) -> Option<TimerInfo> {
    let parser = get_fdt_parser()?;

    let regs = parser.parse_reg(node);
    let compatible = parser.get_compatible(node)
        .first()?
        .clone();

    Some(TimerInfo {
        compatible,
        regs,
        frequency: parser.get_clock_frequency(node),
    })
}

/// CPU information
#[derive(Debug, Clone)]
pub struct CpuInfo {
    /// CPU ID
    pub cpu_id: u32,
    /// Compatible string
    pub compatible: String,
    /// Clock frequency in Hz
    pub clock_frequency: Option<u32>,
}

/// Memory region
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    /// Base address
    pub address: u64,
    /// Size
    pub size: u64,
}

/// Interrupt controller type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptControllerType {
    /// PLIC (Platform-Level Interrupt Controller)
    PLIC,
    /// ACLINT (Core-Local Interrupt Controller)
    ACLINT,
    /// Legacy CLINT
    CLINT,
    /// Unknown
    Unknown,
}

/// Interrupt controller information
#[derive(Debug, Clone)]
pub struct InterruptController {
    /// Controller type
    pub ctrl_type: InterruptControllerType,
    /// Register regions
    pub regs: Vec<parser::ParsedReg>,
    /// Number of interrupt cells
    pub interrupt_cells: u32,
}

/// Timer information
#[derive(Debug, Clone)]
pub struct TimerInfo {
    /// Compatible string
    pub compatible: String,
    /// Register regions
    pub regs: Vec<parser::ParsedReg>,
    /// Timer frequency in Hz
    pub frequency: Option<u32>,
}

/// VM FDT configuration
#[derive(Debug, Clone)]
pub struct VmFdtConfig {
    /// Number of vCPUs
    pub vcpu_count: u32,
    /// Memory size in bytes
    pub memory_size: u64,
    /// Memory layout
    pub memory_layout: VmMemoryLayout,
    /// Enable virtual devices
    pub enable_virtio: bool,
    /// Enable virtual console
    pub enable_console: bool,
    /// Virtual network interfaces
    pub virtio_net_count: u32,
    /// Virtual block devices
    pub virtio_blk_count: u32,
}

impl Default for VmFdtConfig {
    fn default() -> Self {
        Self {
            vcpu_count: 1,
            memory_size: 0x40000000, // 1GB
            memory_layout: VmMemoryLayout::default(),
            enable_virtio: true,
            enable_console: true,
            virtio_net_count: 0,
            virtio_blk_count: 1,
        }
    }
}

/// VM memory layout
#[derive(Debug, Clone)]
pub struct VmMemoryLayout {
    /// RAM base address
    pub ram_base: u64,
    /// RAM size
    pub ram_size: u64,
    /// Device tree address
    pub dtb_address: u64,
    /// Kernel load address
    pub kernel_address: u64,
}

impl Default for VmMemoryLayout {
    fn default() -> Self {
        Self {
            ram_base: 0x80000000,
            ram_size: 0x40000000,
            dtb_address: 0x10000000,
            kernel_address: 0x80200000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_fdt_config() {
        let config = VmFdtConfig::default();
        assert_eq!(config.vcpu_count, 1);
        assert_eq!(config.memory_size, 0x40000000);
        assert!(config.enable_virtio);
        assert!(config.enable_console);
        assert_eq!(config.virtio_net_count, 0);
        assert_eq!(config.virtio_blk_count, 1);
    }

    #[test]
    fn test_vm_memory_layout() {
        let layout = VmMemoryLayout::default();
        assert_eq!(layout.ram_base, 0x80000000);
        assert_eq!(layout.ram_size, 0x40000000);
        assert_eq!(layout.dtb_address, 0x10000000);
        assert_eq!(layout.kernel_address, 0x80200000);
    }

    #[test]
    fn test_cpu_info() {
        let cpu = CpuInfo {
            cpu_id: 0,
            compatible: "riscv,spike".to_string(),
            clock_frequency: Some(1000000),
        };
        assert_eq!(cpu.cpu_id, 0);
        assert_eq!(cpu.compatible, "riscv,spike");
        assert_eq!(cpu.clock_frequency, Some(1000000));
    }
}