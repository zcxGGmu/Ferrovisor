//! RISC-V Device Tree Parser
//!
//! This module provides high-level device tree parsing functionality including:
//! - Node path resolution
/// - Property value interpretation
/// - Standard property parsing
/// - Address and interrupt mapping

use crate::arch::riscv64::*;
use crate::arch::riscv64::devtree::fdt::*;
use bitflags::bitflags;

/// Parsed address information
#[derive(Debug, Clone)]
pub struct ParsedAddress {
    /// Physical address
    pub address: u64,
    /// Size
    pub size: u64,
}

/// Parsed interrupt information
#[derive(Debug, Clone)]
pub struct ParsedInterrupt {
    /// Interrupt specifier
    pub interrupt: u32,
    /// Interrupt type (e.g., 0 = high-level)
    pub interrupt_type: u32,
    /// Parent interrupt controller
    pub parent: Option<String>,
}

/// Parsed reg property (cells)
#[derive(Debug, Clone)]
pub struct ParsedReg {
    /// Physical address
    pub address: u64,
    /// Size
    pub size: u64,
}

/// Parsed Ranges property
#[derive(Debug, Clone)]
pub struct ParsedRange {
    /// Bus address
    pub bus_address: u64,
    /// CPU address
    pub cpu_address: u64,
    /// Size
    pub size: u64,
}

/// Device tree parser configuration
#[derive(Debug, Clone)]
pub struct ParserConfig {
    /// Default address cell size
    pub default_address_cells: u32,
    /// Default size cell size
    pub default_size_cells: u32,
    /// Default interrupt cell size
    pub default_interrupt_cells: u32,
}

impl Default for ParserConfig {
    fn default() -> Self {
        Self {
            default_address_cells: 2,
            default_size_cells: 1,
            default_interrupt_cells: 1,
        }
    }
}

/// Device tree parser
pub struct DeviceTreeParser {
    /// FDT reference
    fdt: FlattenedDeviceTree,
    /// Parser configuration
    config: ParserConfig,
}

impl DeviceTreeParser {
    /// Create new parser from FDT
    pub fn new(fdt: FlattenedDeviceTree, config: ParserConfig) -> Self {
        Self { fdt, config }
    }

    /// Create parser with default config
    pub fn new_default(fdt: FlattenedDeviceTree) -> Self {
        Self::new(fdt, ParserConfig::default())
    }

    /// Get root node
    pub fn get_root(&self) -> Option<&Node> {
        self.fdt.get_root()
    }

    /// Find node by path
    pub fn find_node(&self, path: &str) -> Option<&Node> {
        self.fdt.find_node(path)
    }

    /// Find node by compatible string
    pub fn find_node_by_compatible(&self, compatible: &str) -> Option<&Node> {
        self.find_node_by_compatible_recursive(self.get_root()?, compatible)
    }

    /// Recursive search for compatible string
    fn find_node_by_compatible_recursive(&self, node: &Node, compatible: &str) -> Option<&Node> {
        // Check current node
        if let Some(prop) = node.get_property("compatible") {
            if let Some(prop_str) = prop.as_string() {
                if prop_str.contains(compatible) {
                    return Some(node);
                }
            }

            // Check if compatible is an array of strings
            if prop.prop_type == PropertyType::ByteArray {
                // Parse as null-terminated string array
                let mut offset = 0;
                while offset < prop.len() {
                    // Find next null terminator
                    let end = prop[offset..].iter().position(|&b| *b == 0);
                    if let Some(len) = end {
                        if let Ok(s) = str::from_utf8(&prop[offset..offset + len]) {
                            if s.contains(compatible) {
                                return Some(node);
                            }
                        }
                        offset += len + 1;
                    } else {
                        break;
                    }
                }
            }
        }

        // Check children
        for child in &node.children {
            if let Some(found) = self.find_node_by_compatible_recursive(child, compatible) {
                return Some(found);
            }
        }

        None
    }

    /// Get address cells for a node
    pub fn get_address_cells(&self, node: &Node) -> u32 {
        node.get_prop_u32("#address-cells")
            .unwrap_or(self.config.default_address_cells)
    }

    /// Get size cells for a node
    pub fn get_size_cells(&self, node: &Node) -> u32 {
        node.get_prop_u32("#size-cells")
            .unwrap_or(self.config.default_size_cells)
    }

    /// Get interrupt cells for a node
    pub fn get_interrupt_cells(&self, node: &Node) -> u32 {
        node.get_prop_u32("#interrupt-cells")
            .unwrap_or(self.config.default_interrupt_cells)
    }

    /// Parse reg property
    pub fn parse_reg(&self, node: &Node) -> Vec<ParsedReg> {
        let Some(prop) = node.get_property("reg") else {
            return Vec::new();
        };

        let addr_cells = self.get_address_cells(node);
        let size_cells = self.get_size_cells(node);
        let total_cells = addr_cells + size_cells;

        let mut regs = Vec::new();
        let mut offset = 0;

        while offset + (total_cells as usize) * 4 <= prop.len() {
            // Parse address
            let mut address = 0u64;
            for i in 0..addr_cells {
                let cell_offset = offset + (i as usize) * 4;
                if cell_offset + 4 <= prop.len() {
                    address = (address << 32) | u32::from_be_bytes([
                        prop[cell_offset],
                        prop[cell_offset + 1],
                        prop[cell_offset + 2],
                        prop[cell_offset + 3],
                    ]) as u64;
                }
            }

            // Parse size
            let mut size = 0u64;
            for i in 0..size_cells {
                let cell_offset = offset + (addr_cells + i) as usize * 4;
                if cell_offset + 4 <= prop.len() {
                    size = (size << 32) | u32::from_be_bytes([
                        prop[cell_offset],
                        prop[cell_offset + 1],
                        prop[cell_offset + 2],
                        prop[cell_offset + 3],
                    ]) as u64;
                }
            }

            regs.push(ParsedReg { address, size });
            offset += (total_cells * 4) as usize;
        }

        regs
    }

    /// Parse ranges property
    pub fn parse_ranges(&self, node: &Node) -> Vec<ParsedRange> {
        let Some(prop) = node.get_property("ranges") else {
            return Vec::new();
        };

        // Get parent's address and size cells
        let parent_addr_cells = self.get_parent_address_cells(node);
        let child_addr_cells = self.get_address_cells(node);
        let size_cells = self.get_size_cells(node);

        let total_cells = parent_addr_cells + child_addr_cells + size_cells;

        let mut ranges = Vec::new();
        let mut offset = 0;

        while offset + (total_cells as usize) * 4 <= prop.len() {
            // Parse bus address
            let mut bus_address = 0u64;
            for i in 0..child_addr_cells {
                let cell_offset = offset + (i as usize) * 4;
                if cell_offset + 4 <= prop.len() {
                    bus_address = (bus_address << 32) | u32::from_be_bytes([
                        prop[cell_offset],
                        prop[cell_offset + 1],
                        prop[cell_offset + 2],
                        prop[cell_offset + 3],
                    ]) as u64;
                }
            }

            // Parse CPU address
            let mut cpu_address = 0u64;
            for i in 0..parent_addr_cells {
                let cell_offset = offset + (child_addr_cells + i) as usize * 4;
                if cell_offset + 4 <= prop.len() {
                    cpu_address = (cpu_address << 32) | u32::from_be_bytes([
                        prop[cell_offset],
                        prop[cell_offset + 1],
                        prop[cell_offset + 2],
                        prop[cell_offset + 3],
                    ]) as u64;
                }
            }

            // Parse size
            let mut size = 0u64;
            for i in 0..size_cells {
                let cell_offset = offset + (child_addr_cells + parent_addr_cells + i) as usize * 4;
                if cell_offset + 4 <= prop.len() {
                    size = (size << 32) | u32::from_be_bytes([
                        prop[cell_offset],
                        prop[cell_offset + 1],
                        prop[cell_offset + 2],
                        prop[cell_offset + 3],
                    ]) as u64;
                }
            }

            ranges.push(ParsedRange {
                bus_address,
                cpu_address,
                size,
            });
            offset += (total_cells * 4) as usize;
        }

        ranges
    }

    /// Parse interrupts property
    pub fn parse_interrupts(&self, node: &Node) -> Vec<ParsedInterrupt> {
        let Some(prop) = node.get_property("interrupts") else {
            return Vec::new();
        };

        let interrupt_cells = self.get_interrupt_cells(node);
        let total_cells = interrupt_cells;

        let mut interrupts = Vec::new();
        let mut offset = 0;

        while offset + (total_cells as usize) * 4 <= prop.len() {
            let mut interrupt = 0u32;
            let mut interrupt_type = 0u32;
            let mut parent = None;

            // Parse interrupt cells
            match total_cells {
                1 => {
                    // Just interrupt number
                    interrupt = u32::from_be_bytes([
                        prop[offset],
                        prop[offset + 1],
                        prop[offset + 2],
                        prop[offset + 3],
                    ]);
                }
                2 => {
                    // Interrupt number and type
                    interrupt = u32::from_be_bytes([
                        prop[offset],
                        prop[offset + 1],
                        prop[offset + 2],
                        prop[offset + 3],
                    ]);
                    interrupt_type = u32::from_be_bytes([
                        prop[offset + 4],
                        prop[offset + 5],
                        prop[offset + 6],
                        prop[offset + 7],
                    ]);
                }
                _ => {
                    // Parse first two cells, ignore the rest for now
                    interrupt = u32::from_be_bytes([
                        prop[offset],
                        prop[offset + 1],
                        prop[offset + 2],
                        prop[offset + 3],
                    ]);
                    interrupt_type = u32::from_be_bytes([
                        prop[offset + 4],
                        prop[offset + 5],
                        prop[offset + 6],
                        prop[offset + 7],
                    ]);
                }
            }

            // Try to find parent interrupt controller
            if let Some(parent_prop) = node.get_property("interrupt-parent") {
                if let Some(phandle) = parent_prop.as_u32() {
                    parent = Some(format!("phandle:{}", phandle));
                }
            } else if let Some(interrupt_map) = node.get_property("interrupt-map") {
                    // Parse interrupt-map for parent
                    parent = Some("interrupt-map".to_string());
                }

            interrupts.push(ParsedInterrupt {
                interrupt,
                interrupt_type,
                parent,
            });
            offset += (total_cells * 4) as usize;
        }

        interrupts
    }

    /// Get parent node's address cells
    fn get_parent_address_cells(&self, node: &Node) -> u32 {
        // Find parent by walking up the tree
        if let Some(parent_path) = self.get_parent_path(node) {
            if let Some(parent) = self.find_node(&parent_path) {
                self.get_address_cells(parent)
            } else {
                self.config.default_address_cells
            }
        } else {
            self.config.default_address_cells
        }
    }

    /// Get parent node path
    fn get_parent_path(&self, node: &Node) -> Option<String> {
        // This would require maintaining parent references
        // For now, return None
        None
    }

    /// Parse status property
    pub fn get_status(&self, node: &Node) -> DeviceStatus {
        match node.get_prop_string("status") {
            Some("ok") | None => DeviceStatus::Okay,
            Some("disabled") => DeviceStatus::Disabled,
            Some("reserved") => DeviceStatus::Reserved,
            Some("fail") => DeviceStatus::Fail,
            _ => DeviceStatus::Unknown,
        }
    }

    /// Check if node is enabled
    pub fn is_enabled(&self, node: &Node) -> bool {
        matches!(self.get_status(node), DeviceStatus::Okay)
    }

    /// Get model property
    pub fn get_model(&self, node: &Node) -> Option<String> {
        node.get_prop_string("model").map(|s| s.to_string())
    }

    /// Get compatible strings
    pub fn get_compatible(&self, node: &Node) -> Vec<String> {
        let mut compatible_list = Vec::new();

        if let Some(prop) = node.get_property("compatible") {
            if prop.prop_type == PropertyType::String {
                if let Some(s) = prop.as_string() {
                    compatible_list.push(s.to_string());
                }
            } else if prop.prop_type == PropertyType::ByteArray {
                // Parse as null-terminated string array
                let mut offset = 0;
                while offset < prop.len() {
                    let end = prop[offset..].iter().position(|&b| *b == 0);
                    if let Some(len) = end {
                        if let Ok(s) = str::from_utf8(&prop[offset..offset + len]) {
                            compatible_list.push(s.to_string());
                        }
                        offset += len + 1;
                    } else {
                        break;
                    }
                }
            }
        }

        compatible_list
    }

    /// Parse clock-frequency property
    pub fn get_clock_frequency(&self, node: &Node) -> Option<u32> {
        node.get_prop_u32("clock-frequency")
    }

    /// Parse clock-frequency-range property
    pub fn get_clock_frequency_range(&self, node: &Node) -> Option<(u32, u32)> {
        if let Some(prop) = node.get_property("clock-frequency-range") {
            if prop.len() >= 8 {
                let min = u32::from_be_bytes([prop[0], prop[1], prop[2], prop[3]]);
                let max = u32::from_be_bytes([prop[4], prop[5], prop[6], prop[7]]);
                Some((min, max))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Get CPU count
    pub fn get_cpu_count(&self) -> usize {
        let mut count = 0;

        // Look for "cpus" node
        if let Some(cpus_node) = self.find_node("/cpus") {
            // Count child nodes starting with "cpu@"
            for child in &cpus_node.children {
                if child.name.starts_with("cpu@") {
                    // Check if CPU is enabled
                    if self.is_enabled(child) {
                        count += 1;
                    }
                }
            }
        }

        count
    }

    /// Get memory size
    pub fn get_memory_size(&self) -> u64 {
        let mut total_size = 0u64;

        if let Some(memory_node) = self.find_node("/memory") {
            for reg in self.parse_reg(memory_node) {
                total_size += reg.size;
            }
        }

        total_size
    }

    /// Iterate over all nodes
    pub fn iter_nodes(&self) -> NodeIterator {
        NodeIterator::new(self.get_root()?)
    }

    /// Iterate over all properties
    pub fn iter_properties(&self) -> PropertyIterator {
        PropertyIterator::new(self.get_root()?)
    }
}

/// Device status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceStatus {
    /// Device is okay
    Okay,
    /// Device is disabled
    Disabled,
    /// Device is reserved
    Reserved,
    /// Device failed
    Fail,
    /// Unknown status
    Unknown,
}

/// Standard properties
pub mod standard_props {
    /// CPU properties
    pub const CPU_RELEASE: &str = "cpu-release";
    pub const CPU_IDLE: &str = "cpu-idle";
    pub const CPU_OPERATING_POINTS: &str = "operating-points";
    pub const CPU_POWER_DOMAINS: &str = "power-domains";

    /// Memory properties
    pub const MEMORY_DEVICE_TYPE: &str = "memory-device-type";
    pub const MEMORY_REG: &str = "memory-region";

    /// Interrupt controller properties
    pub const INTERRUPT_CONTROLLER: &str = "interrupt-controller";
    pub const INTERRUPT_PARENT: &str = "interrupt-parent";
    pub const INTERRUPTS: &str = "interrupts";
    pub const INTERRUPT_MAP: &str = "interrupt-map";
    pub const INTERRUPT_MAP_MASK: &str = "interrupt-map-mask";

    /// Clock properties
    pub const CLOCKS: &str = "clocks";
    pub const CLOCK_NAMES: &str = "clock-names";
    pub const CLOCK_OUTPUT_NAMES: &str = "clock-output-names";
    pub const ASSIGNED_CLOCKS: &str = "assigned-clocks";
    pub const CLOCK_RATES: &str = "clock-rates";

    /// Timer properties
    pub const TIMER: &str = "timer";

    /// Serial properties
    pub const SERIAL: &str = "serial";

    /// Ethernet properties
    pub const ETHERNET: &str = "ethernet";
    pub const LOCAL_MAC_ADDRESS: &str = "local-mac-address";
    pub const PHY_HANDLE: &str = "phy-handle";
    pub const PHY_MODE: &str = "phy-mode";

    /// PCI properties
    pub const PCI: &str = "pci";
    pub const VENDOR_ID: &str = "vendor-id";
    pub const DEVICE_ID: str = "device-id";
    pub const SUBSYSTEM_VENDOR_ID: &str = "subsystem-vendor-id";
    pub const SUBSYSTEM_DEVICE_ID: &str = "subsystem-device-id";
    pub const CLASS_CODE: &str = "class-code";
    pub const REVISION_ID: &str = "revision-id";
    pub const NUMERIC_REVISION: &str = "#numeric-revision";

    /// GPIO properties
    pub const GPIO: &str = "gpio";
    pub const GPIO_CONTROLLER: &str = "gpio-controller";
    pub const GPIO_RANGES: &str = "gpio-ranges";
    pub const NGPIO: &str = "ngpios";

    /// I2C properties
    pub const I2C: &str = "i2c";
    pub const I2C_BUS: &str = "i2c-bus";
    pub const #ADDRESS_CELLS: &str = "#address-cells";
    pub const #SIZE_CELLS: &str = "#size-cells";
    pub const #INTERRUPT_CELLS: &str = "#interrupt-cells";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_config() {
        let config = ParserConfig::default();
        assert_eq!(config.default_address_cells, 2);
        assert_eq!(config.default_size_cells, 1);
        assert_eq!(config.default_interrupt_cells, 1);
    }

    #[test]
    fn test_parsed_address() {
        let addr = ParsedAddress {
            address: 0x80000000,
            size: 0x10000,
        };
        assert_eq!(addr.address, 0x80000000);
        assert_eq!(addr.size, 0x10000);
    }

    #[test]
    fn test_parsed_interrupt() {
        let intr = ParsedInterrupt {
            interrupt: 25,
            interrupt_type: 4,
            parent: Some("gic".to_string()),
        };
        assert_eq!(intr.interrupt, 25);
        assert_eq!(intr.interrupt_type, 4);
        assert_eq!(intr.parent, Some("gic".to_string()));
    }

    #[test]
    fn test_device_status() {
        assert_eq!(DeviceStatus::Okay, DeviceStatus::Okay);
        assert_eq!(DeviceStatus::Disabled, DeviceStatus::Disabled);
    }
}