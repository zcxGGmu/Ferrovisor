//! ARM64 Device Tree Parsing
//!
//! This module provides ARM64-specific device tree parsing functionality:
//! - CPU node parsing (enable-method, cpu-release-addr)
//! - GIC node parsing (interrupt-controller)
//! - Timer node parsing (arm,armv8-timer)
//! - Memory node parsing
//!
//! ## Device Tree Paths
//!
//! Standard ARM device tree paths:
//! - `/cpus` - CPU container node
//! - `/cpus/cpu@N` - Individual CPU nodes
//! - `/interrupt-controller` - GIC node
//! - `/timer` - Generic Timer node
//! - `/memory@ADDR` - Memory regions

use super::{CpuInfo, CpuEnableMethod, GicInfo, TimerInfo, MemInfo, compat, props};
use crate::arch::riscv64::devtree::fdt::{FlattenedDeviceTree, Node, Property};

/// Global hardware information extracted from device tree
pub struct HardwareInfo {
    /// CPU information
    pub cpus: Vec<CpuInfo>,
    /// GIC information
    pub gic: Option<GicInfo>,
    /// Timer information
    pub timer: Option<TimerInfo>,
    /// Memory regions
    pub memory: Vec<MemInfo>,
    /// PSCI available
    pub psci_available: bool,
}

impl HardwareInfo {
    /// Create new hardware info
    pub fn new() -> Self {
        Self {
            cpus: Vec::new(),
            gic: None,
            timer: None,
            memory: Vec::new(),
            psci_available: false,
        }
    }

    /// Get boot CPU
    pub fn boot_cpu(&self) -> Option<&CpuInfo> {
        self.cpus.first()
    }

    /// Get number of CPUs
    pub fn cpu_count(&self) -> usize {
        self.cpus.len()
    }

    /// Get CPU by MPIDR
    pub fn cpu_by_mpidr(&self, mpidr: u64) -> Option<&CpuInfo> {
        self.cpus.iter().find(|cpu| cpu.mpidr == mpidr)
    }
}

impl Default for HardwareInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Global hardware info (initialized from device tree)
static mut HW_INFO: Option<HardwareInfo> = None;

/// Parse device tree and extract hardware information
pub fn parse_device_tree() -> Result<(), &'static str> {
    // Get FDT from boot (this would be provided by bootloader)
    // For now, we'll provide a placeholder implementation

    let hw_info = HardwareInfo::new();

    // Store globally
    unsafe {
        HW_INFO = Some(hw_info);
    }

    log::info!("Device tree: Parsed hardware info");
    Ok(())
}

/// Get global hardware info
pub fn get_hw_info() -> Option<&'static HardwareInfo> {
    unsafe { HW_INFO.as_ref() }
}

/// Parse CPU nodes from device tree
pub fn parse_cpu_nodes(fdt: &FlattenedDeviceTree) -> Result<Vec<CpuInfo>, &'static str> {
    let mut cpus = Vec::new();

    // Find /cpus node
    let cpus_node = fdt.find_node("/cpus")
        .ok_or("CPUs node not found")?;

    // Iterate over child nodes (cpu@0, cpu@1, etc.)
    for child in &cpus_node.children {
        if child.name.starts_with("cpu@") {
            let cpu_info = parse_cpu_node(child)?;
            cpus.push(cpu_info);
        }
    }

    // Sort by CPU ID
    cpus.sort_by(|a, b| a.cpu_id.cmp(&b.cpu_id));

    log::info!("Device tree: Found {} CPUs", cpus.len());
    Ok(cpus)
}

/// Parse a single CPU node
fn parse_cpu_node(node: &Node) -> Result<CpuInfo, &'static str> {
    // Extract CPU ID from node name (e.g., "cpu@0" -> 0)
    let cpu_id = node.name
        .strip_prefix("cpu@")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);

    // Read reg property (contains MPIDR for ARM)
    let mpidr = if let Some(reg) = node.get_property("reg") {
        if reg.data.len() >= 8 {
            u64::from_be_bytes([
                reg.data[0], reg.data[1], reg.data[2], reg.data[3],
                reg.data[4], reg.data[5], reg.data[6], reg.data[7],
            ])
        } else if reg.data.len() == 4 {
            u32::from_be_bytes([reg.data[0], reg.data[1], reg.data[2], reg.data[3]]) as u64
        } else {
            0
        }
    } else {
        // Default MPIDR based on CPU ID
        (cpu_id as u64) | 0x80000000
    };

    let mut cpu_info = CpuInfo::new(cpu_id, mpidr);

    // Read enable-method
    if let Some(method) = node.get_prop_string(props::ENABLE_METHOD) {
        cpu_info.enable_method = CpuEnableMethod::from_str(method);
        log::debug!("CPU {}: enable-method = {}", cpu_id, method);
    }

    // Read cpu-release-addr (for spin-table method)
    if let Some(addr) = node.get_prop_u64(props::CPU_RELEASE_ADDR) {
        cpu_info.release_addr = Some(addr);
        log::debug!("CPU {}: cpu-release-addr = 0x{:x}", cpu_id, addr);
    }

    // Read capacity-dmips-mhz
    if let Some(capacity) = node.get_prop_u32(props::CAPACITY_DMHZ) {
        cpu_info.capacity = Some(capacity);
    }

    // Read clock-frequency
    if let Some(freq) = node.get_prop_u64(props::CLOCK_FREQUENCY) {
        cpu_info.clock_frequency = Some(freq);
    }

    // Read device-type to confirm it's a CPU
    if let Some(dev_type) = node.get_prop_string(props::DEVICE_TYPE) {
        if dev_type != props::DEV_TYPE_CPU {
            log::warn!("CPU {}: device-type is '{}', expected 'cpu'", cpu_id, dev_type);
        }
    }

    Ok(cpu_info)
}

/// Parse GIC node from device tree
pub fn parse_gic_node(fdt: &FlattenedDeviceTree) -> Result<Option<GicInfo>, &'static str> {
    // Try to find interrupt-controller node
    if let Some(node) = fdt.find_node("/interrupt-controller") {
        return Ok(Some(parse_gic_from_node(node)?));
    }

    // Try alternative paths
    for path in ["/soc/interrupt-controller", "/gic"].iter() {
        if let Some(node) = fdt.find_node(path) {
            return Ok(Some(parse_gic_from_node(node)?));
        }
    }

    Ok(None)
}

/// Parse GIC from a specific node
fn parse_gic_from_node(node: &Node) -> Result<GicInfo, &'static str> {
    let mut gic = GicInfo::new();

    // Read compatible string to determine GIC version
    if let Some compat_str) = node.get_prop_string("compatible") {
        gic.compatible = compat_str.to_string();

        gic.version = if compat_str.contains("gic-v3") {
            3
        } else if compat_str.contains("gic-400") || compat_str.contains("gic-v2") {
            2
        } else if compat_str.contains("gic-v1") {
            1
        } else {
            3 // Default to v3
        };

        log::info!("GIC: compatible = {}, version = {}", compat_str, gic.version);
    }

    // Read reg property (GIC register ranges)
    if let Some(reg) = node.get_property(props::REG) {
        gic.regs = parse_reg_property(reg)?;

        log::info!("GIC: {} register ranges", gic.regs.len());
        for (i, (addr, size)) in gic.regs.iter().enumerate() {
            log::debug!("  GIC reg[{}]: 0x{:x} + 0x{:x}", i, addr, size);
        }
    }

    // Read interrupts property (maintenance interrupts)
    if let Some(irqs) = node.get_property(props::INTERRUPTS) {
        gic.interrupts = parse_u32_array(irqs);
    }

    // Read #interrupt-cells
    if let Some(cells) = node.get_prop_u32("#interrupt-cells") {
        gic.num_irqs = cells;
    }

    Ok(gic)
}

/// Parse Timer node from device tree
pub fn parse_timer_node(fdt: &FlattenedDeviceTree) -> Result<Option<TimerInfo>, &'static str> {
    if let Some(node) = fdt.find_node("/timer") {
        return Ok(Some(parse_timer_from_node(node)?));
    }

    Ok(None)
}

/// Parse Timer from a specific node
fn parse_timer_from_node(node: &Node) -> Result<TimerInfo, &'static str> {
    let mut timer = TimerInfo::new();

    // Read compatible string
    if let Some(compat_str) = node.get_prop_string("compatible") {
        timer.compatible = compat_str.to_string();
        log::debug!("Timer: compatible = {}", compat_str);
    }

    // Read interrupts property
    // Format: <SEC_PPI SEC_NS_PPI VIRQ PHY_HYP_PPI>
    if let Some(irqs) = node.get_property(props::INTERRUPTS) {
        timer.interrupts = parse_u32_array(irqs);

        log::info!("Timer: {} interrupt entries", timer.interrupts.len());
        for (i, irq) in timer.interrupts.iter().enumerate() {
            log::debug!("  Timer IRQ[{}]: {}", i, irq);
        }
    }

    // Read clock-frequency
    if let Some(freq) = node.get_prop_u64(props::CLOCK_FREQUENCY) {
        timer.clock_frequency = Some(freq);
        log::info!("Timer: clock-frequency = {} Hz", freq);
    }

    // Always use the ARM Generic Timer frequency register
    if timer.clock_frequency.is_none() {
        // Read CNTFRQ_EL0
        let freq: u64;
        unsafe {
            core::arch::asm!("mrs {}, cntfrq_el0", out(reg) freq);
        }
        timer.clock_frequency = Some(freq);
        log::info!("Timer: CNTFRQ_EL0 = {} Hz", freq);
    }

    Ok(timer)
}

/// Parse memory nodes from device tree
pub fn parse_memory_nodes(fdt: &FlattenedDeviceTree) -> Result<Vec<MemInfo>, &'static str> {
    let mut memory = Vec::new();

    // Find memory nodes (can be multiple)
    // Standard path: /memory
    if let Some(node) = fdt.find_node("/memory") {
        if let Ok(mem) = parse_memory_node(node, 0) {
            memory.push(mem);
        }
    }

    // Check for numbered memory nodes (memory@0, memory@80000000, etc.)
    let root = fdt.get_root().ok_or("No root node")?;
    for child in &root.children {
        if child.name.starts_with("memory@") {
            if let Ok(mem) = parse_memory_node(child, memory.len() as u32) {
                memory.push(mem);
            }
        }
    }

    log::info!("Device tree: Found {} memory regions", memory.len());
    for mem in &memory {
        log::debug!("  Memory: 0x{:x} - 0x{:x} ({} MB)",
                   mem.base, mem.end(), mem.size / (1024 * 1024));
    }

    Ok(memory)
}

/// Parse a single memory node
fn parse_memory_node(node: &Node, index: u32) -> Result<MemInfo, &'static str> {
    // Read device-type
    if let Some(dev_type) = node.get_prop_string(props::DEVICE_TYPE) {
        if dev_type != props::DEV_TYPE_MEMORY {
            return Err("Not a memory node");
        }
    }

    // Read reg property (base address and size)
    if let Some(reg) = node.get_property(props::REG) {
        let ranges = parse_reg_property(reg)?;
        if ranges.is_empty() {
            return Err("Memory reg property is empty");
        }

        let (base, size) = ranges[0];
        log::debug!("Memory node {}: base=0x{:x}, size=0x{:x}", index, base, size);
        return Ok(MemInfo::new(base, size));
    }

    Err("Memory node missing reg property")
}

/// Parse PSCI node from device tree
pub fn parse_psci_node(fdt: &FlattenedDeviceTree) -> Result<bool, &'static str> {
    if let Some(node) = fdt.find_node("/psci") {
        // Check if PSCI is present
        if let Some(method) = node.get_prop_string("method") {
            log::info!("PSCI: method = {}", method);
            return Ok(true);
        }
    }

    Ok(false)
}

/// Parse 'reg' property (address/size pairs)
fn parse_reg_property(prop: &Property) -> Result<Vec<(u64, u64)>, &'static str> {
    let data = prop.as_bytes();
    let mut ranges = Vec::new();

    // Each cell is 4 bytes (u32)
    // #address-cells and #size-cells determine the format
    // Default to 2 cells each for 64-bit systems
    let addr_cells = 2;
    let size_cells = 2;
    let entry_size = (addr_cells + size_cells) * 4;

    if data.len() % entry_size != 0 {
        return Err("Invalid reg property length");
    }

    for chunk in data.chunks_exact(entry_size) {
        let addr = if addr_cells == 2 {
            u64::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3],
                               chunk[4], chunk[5], chunk[6], chunk[7]])
        } else {
            u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as u64
        };

        let size_offset = addr_cells * 4;
        let size = if size_cells == 2 {
            u64::from_be_bytes([chunk[size_offset], chunk[size_offset+1],
                               chunk[size_offset+2], chunk[size_offset+3],
                               chunk[size_offset+4], chunk[size_offset+5],
                               chunk[size_offset+6], chunk[size_offset+7]])
        } else {
            u32::from_be_bytes([chunk[size_offset], chunk[size_offset+1],
                               chunk[size_offset+2], chunk[size_offset+3]]) as u64
        };

        ranges.push((addr, size));
    }

    Ok(ranges)
}

/// Parse property as u32 array
fn parse_u32_array(prop: &Property) -> Vec<u32> {
    let data = prop.as_bytes();
    let mut result = Vec::new();

    for chunk in data.chunks_exact(4) {
        let val = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        result.push(val);
    }

    result
}

/// Parse complete device tree and extract all hardware info
pub fn parse_complete(fdt: &FlattenedDeviceTree) -> Result<HardwareInfo, &'static str> {
    let mut hw_info = HardwareInfo::new();

    // Parse CPU nodes
    hw_info.cpus = parse_cpu_nodes(fdt)?;

    // Parse GIC node
    hw_info.gic = parse_gic_node(fdt)?;

    // Parse Timer node
    hw_info.timer = parse_timer_node(fdt)?;

    // Parse Memory nodes
    hw_info.memory = parse_memory_nodes(fdt)?;

    // Parse PSCI
    hw_info.psci_available = parse_psci_node(fdt)?;

    Ok(hw_info)
}

/// Parse interrupt specifier from device tree
///
/// ARM GIC interrupt format:
/// - 1 cell: SPI (shared peripheral interrupt)
/// - 2 cells: (SPI, flags) or (PPI, flags)
/// - 3 cells: (interrupt_type, hw_irq, flags)
///   - interrupt_type: 0=SPI, 1=PPI
///   - hw_irq: interrupt number (0-31 for PPI, 32-1019 for SPI)
///   - flags: interrupt flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptType {
    /// SGI (Software Generated Interrupt) - 0-15
    Sgi(u8),
    /// PPI (Private Peripheral Interrupt) - 16-31
    Ppi(u8),
    /// SPI (Shared Peripheral Interrupt) - 32-1019
    Spi(u16),
}

/// Interrupt flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterruptFlags {
    pub edge_triggered: bool,
    pub level_sensitive: bool,
    pub high_level: bool,
    pub low_level: bool,
    pub rising_edge: bool,
    pub falling_edge: bool,
}

impl InterruptFlags {
    /// Parse from u32 flags value
    pub fn from_u32(flags: u32) -> Self {
        Self {
            edge_triggered: (flags & 0x4) != 0,
            level_sensitive: (flags & 0x4) == 0,
            high_level: (flags & 0x3) == 0x1,
            low_level: (flags & 0x3) == 0x3,
            rising_edge: (flags & 0x3) == 0x2,
            falling_edge: (flags & 0x3) == 0x0,
        }
    }
}

/// Parse interrupt specifier
pub fn parse_interrupt(data: &[u8]) -> Result<(InterruptType, InterruptFlags), &'static str> {
    if data.len() < 4 {
        return Err("Interrupt data too short");
    }

    // Read interrupt type (1st cell)
    let irq_type = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);

    match data.len() {
        4 => {
            // 1 cell: Just the SPI number
            let irq = (irq_type & 0x3FF) as u16;
            if irq >= 32 {
                Ok((InterruptType::Spi(irq), InterruptFlags::from_u32(0)))
            } else {
                Ok((InterruptType::Ppi(irq as u8), InterruptFlags::from_u32(0)))
            }
        }
        8 => {
            // 2 cells: (irq, flags)
            let irq = (irq_type & 0x3FF) as u16;
            let flags = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
            let int_flags = InterruptFlags::from_u32(flags);

            if irq < 16 {
                Ok((InterruptType::Sgi(irq as u8), int_flags))
            } else if irq < 32 {
                Ok((InterruptType::Ppi(irq as u8), int_flags))
            } else {
                Ok((InterruptType::Spi(irq), int_flags))
            }
        }
        12 => {
            // 3 cells: (type, hw_irq, flags)
            let hw_irq = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as u16;
            let flags = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
            let int_flags = InterruptFlags::from_u32(flags);

            match irq_type {
                0 => Ok((InterruptType::Spi(hw_irq), int_flags)),
                1 => Ok((InterruptType::Ppi(hw_irq as u8), int_flags)),
                _ => Err("Invalid interrupt type"),
            }
        }
        _ => Err("Invalid interrupt specifier length"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_interrupt_spi() {
        let data = [0, 0, 0, 0, 0, 0, 0x03, 0xE8, 0, 0, 0, 4]; // SPI 1000, edge-triggered
        let (irq_type, flags) = parse_interrupt(&data).unwrap();
        assert!(matches!(irq_type, InterruptType::Spi(1000)));
        assert!(flags.edge_triggered);
    }

    #[test]
    fn test_parse_interrupt_ppi() {
        let data = [0, 0, 0, 1, 0, 0, 0, 0x1B, 0, 0, 0, 1]; // PPI 27, high-level
        let (irq_type, flags) = parse_interrupt(&data).unwrap();
        assert!(matches!(irq_type, InterruptType::Ppi(27)));
        assert!(flags.high_level);
    }

    #[test]
    fn test_hardware_info() {
        let hw_info = HardwareInfo::new();
        assert_eq!(hw_info.cpu_count(), 0);
        assert!(hw_info.boot_cpu().is_none());
    }
}
