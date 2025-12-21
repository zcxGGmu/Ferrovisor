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
    /// Cached node references for fast lookup
    node_cache: core::cell::RefCell<core::collections::HashMap<String, *const Node>>,
    /// Address translation cache
    addr_cache: core::cell::RefCell<core::collections::HashMap<String, AddrTranslation>>,
    /// Interrupt mapping cache
    int_cache: core::cell::RefCell<core::collections::HashMap<String, IntMapping>>,
}

/// Address translation information
#[derive(Debug, Clone)]
pub struct AddrTranslation {
    /// Bus address
    pub bus_addr: u64,
    /// Parent bus address
    pub parent_addr: u64,
    /// Size
    pub size: u64,
    /// Translation flags
    pub flags: u32,
}

/// Interrupt mapping information
#[derive(Debug, Clone)]
pub struct IntMapping {
    /// Interrupt specifier
    pub interrupt: u32,
    /// Interrupt type
    pub int_type: u32,
    /// Parent controller phandle
    pub parent_phandle: Option<u32>,
    /// Parent controller node path
    pub parent_path: Option<String>,
}

impl DeviceTreeParser {
    /// Create new parser from FDT
    pub fn new(fdt: FlattenedDeviceTree, config: ParserConfig) -> Self {
        let mut parser = Self {
            fdt,
            config,
            node_cache: core::cell::RefCell::new(core::collections::HashMap::new()),
            addr_cache: core::cell::RefCell::new(core::collections::HashMap::new()),
            int_cache: core::cell::RefCell::new(core::collections::HashMap::new()),
        };

        // Initialize caches
        parser.build_caches();
        parser
    }

    /// Create parser with default config
    pub fn new_default(fdt: FlattenedDeviceTree) -> Self {
        Self::new(fdt, ParserConfig::default())
    }

    /// Build caches for fast lookup
    fn build_caches(&mut self) {
        if let Some(root) = self.get_root() {
            self.build_node_cache_recursive(root, "");
        }
    }

    /// Build node cache recursively
    fn build_node_cache_recursive(&self, node: &Node, path: &str) {
        let current_path = if path.is_empty() {
            "/".to_string()
        } else if path == "/" {
            format!("/{}", node.name)
        } else {
            format!("{}/{}", path, node.name)
        };

        // Add to cache
        self.node_cache.borrow_mut().insert(current_path.clone(), node as *const Node);

        // Recursively process children
        for child in &node.children {
            self.build_node_cache_recursive(child, &current_path);
        }
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
        if node.depth == 0 {
            return None;
        }

        // Get parent from cached references
        let full_path = node.get_full_path();
        if let Some(pos) = full_path.rfind('/') {
            if pos > 0 {
                Some(full_path[..pos].to_string())
            } else {
                Some("/".to_string())
            }
        } else {
            None
        }
    }

    /// Translate bus address to CPU address
    pub fn translate_address(&self, node_path: &str, bus_addr: u64, size: u64) -> Result<u64, &'static str> {
        let cache_key = format!("{}:{:#x}:{:#x}", node_path, bus_addr, size);

        // Check cache first
        if let Some(translation) = self.addr_cache.borrow().get(&cache_key) {
            return Ok(translation.parent_addr + (bus_addr - translation.bus_addr));
        }

        // Walk up the tree to find address translation
        let mut current_path = node_path.to_string();
        let mut translated_addr = bus_addr;

        while !current_path.is_empty() && current_path != "/" {
            if let Some(parent_path) = self.get_parent_path_by_path(&current_path) {
                if let Some(parent_node) = self.find_node(&parent_path) {
                    if let Some(child_node) = self.find_node(&current_path) {
                        let ranges = self.parse_ranges(parent_node);

                        for range in &ranges {
                            if bus_addr >= range.bus_address &&
                               bus_addr < range.bus_address + range.size {
                                // Found matching range
                                let offset = bus_addr - range.bus_address;
                                translated_addr = range.cpu_address + offset;

                                // Cache the translation
                                let translation = AddrTranslation {
                                    bus_addr,
                                    parent_addr: range.cpu_address,
                                    size,
                                    flags: 0,
                                };
                                self.addr_cache.borrow_mut().insert(cache_key, translation);

                                return Ok(translated_addr);
                            }
                        }
                    }
                }
                current_path = parent_path;
            } else {
                break;
            }
        }

        // No translation needed or found
        Ok(translated_addr)
    }

    /// Get parent path by node path
    fn get_parent_path_by_path(&self, node_path: &str) -> Option<String> {
        if node_path.is_empty() || node_path == "/" {
            return None;
        }

        if let Some(pos) = node_path.rfind('/') {
            if pos > 0 {
                Some(node_path[..pos].to_string())
            } else {
                Some("/".to_string())
            }
        } else {
            Some("/")
        }
    }

    /// Find interrupt controller for a node
    pub fn find_interrupt_controller(&self, node_path: &str) -> Option<(String, u32)> {
        if let Some(node) = self.find_node(node_path) {
            // Check for direct interrupt-parent
            if let Some(parent_prop) = node.get_property("interrupt-parent") {
                if let Some(phandle) = parent_prop.as_u32() {
                    if let Some(controller_path) = self.find_node_by_phandle(phandle) {
                        return Some((controller_path, self.get_interrupt_cells(node)));
                    }
                }
            }

            // Walk up tree to find interrupt controller
            let mut current_path = node_path.to_string();
            while !current_path.is_empty() && current_path != "/" {
                if let Some(parent_path) = self.get_parent_path_by_path(&current_path) {
                    if let Some(parent_node) = self.find_node(&parent_path) {
                        if parent_node.get_property("interrupt-controller").is_some() {
                            return Some((parent_path, self.get_interrupt_cells(node)));
                        }
                    }
                    current_path = parent_path;
                } else {
                    break;
                }
            }
        }

        None
    }

    /// Find node by phandle
    pub fn find_node_by_phandle(&self, phandle: u32) -> Option<String> {
        for node_path in self.node_cache.borrow().keys() {
            if let Some(node) = self.find_node(node_path) {
                if let Some(node_phandle) = node.get_prop_u32("phandle") {
                    if node_phandle == phandle {
                        return Some(node_path.clone());
                    }
                }
            }
        }
        None
    }

    /// Parse interrupt-map property
    pub fn parse_interrupt_map(&self, node_path: &str) -> Vec<((u32, u32), (String, u32))> {
        let Some(node) = self.find_node(node_path) else {
            return Vec::new();
        };

        let Some(prop) = node.get_property("interrupt-map") else {
            return Vec::new();
        };

        let Some(interrupt_map_mask) = node.get_property("interrupt-map-mask") else {
            return Vec::new();
        };

        // Parse the interrupt-map and interrupt-map-mask properties
        // This is a simplified implementation - real parsing would be more complex
        let mut mappings = Vec::new();

        // For now, return empty - this would need full implementation
        mappings
    }

    /// Validate FDT structure
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // Check root node
        if self.get_root().is_none() {
            errors.push("No root node found".to_string());
        }

        // Check required nodes
        if self.find_node("/cpus").is_none() {
            errors.push("No /cpus node found".to_string());
        }

        if self.find_node("/memory").is_none() {
            errors.push("No /memory node found".to_string());
        }

        // Validate CPU nodes
        if let Some(cpus_node) = self.find_node("/cpus") {
            for child in &cpus_node.children {
                if child.name.starts_with("cpu@") {
                    if child.get_property("compatible").is_none() {
                        errors.push(format!("CPU node {} missing compatible property", child.name));
                    }
                    if child.get_property("reg").is_none() {
                        errors.push(format!("CPU node {} missing reg property", child.name));
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Create virtual device tree for VM
    pub fn create_vm_fdt(&self, vm_config: &VmDeviceTreeConfig) -> Result<FlattenedDeviceTree, &'static str> {
        // Create a copy of the FDT
        let mut vm_fdt = self.fdt.clone();

        // Modify for VM
        if let Some(root) = vm_fdt.get_root_mut() {
            // Remove or modify hardware-specific nodes
            self.vmify_hardware_nodes(root, vm_config);

            // Add VM-specific nodes
            self.add_vm_specific_nodes(root, vm_config);

            // Update memory node
            self.update_memory_node(root, vm_config);

            // Update CPU node
            self.update_cpu_node(root, vm_config);
        }

        // Serialize back to bytes
        let vm_fdt_bytes = vm_fdt.serialize()
            .map_err(|_| "Failed to serialize VM FDT")?;

        // Create new FDT from bytes
        FlattenedDeviceTree::from_bytes(vm_fdt_bytes)
    }

    /// Modify hardware nodes for VM
    fn vmify_hardware_nodes(&self, root: &mut Node, vm_config: &VmDeviceTreeConfig) {
        // Disable direct hardware access
        let nodes_to_disable = [
            "/pci",
            "/soc/ethernet",
            "/soc/serial",
            "/soc/i2c",
            "/soc/spi",
        ];

        for node_path in &nodes_to_disable {
            if let Some(node) = root.find_path_mut(node_path) {
                // Add status = "disabled" property
                let disabled_prop = Property::new("status", b"disabled\0");
                node.add_property(disabled_prop);
            }
        }
    }

    /// Add VM-specific nodes
    fn add_vm_specific_nodes(&self, root: &mut Node, vm_config: &VmDeviceTreeConfig) {
        // Add virtio nodes
        if vm_config.enable_virtio {
            self.add_virtio_node(root, "virtio_mmio", vm_config);
            self.add_virtio_console_node(root);
            if vm_config.virtio_blk_count > 0 {
                self.add_virtio_block_nodes(root, vm_config.virtio_blk_count);
            }
            if vm_config.virtio_net_count > 0 {
                self.add_virtio_net_nodes(root, vm_config.virtio_net_count);
            }
        }

        // Add hypervisor node
        self.add_hypervisor_node(root);
    }

    /// Add virtio MMIO node
    fn add_virtio_node(&self, root: &mut Node, name: &str, vm_config: &VmDeviceTreeConfig) {
        let mut virtio_node = Node::new(name, 1);

        // Add compatible property
        let compatible_prop = Property::new("compatible", b"virtio,mmio\0");
        virtio_node.add_property(compatible_prop);

        // Add reg property
        let reg_data = 0x10000000u64.to_be_bytes().to_vec(); // Example address
        let reg_size = 0x1000u64.to_be_bytes().to_vec(); // Example size
        let mut reg_prop_data = Vec::new();
        reg_prop_data.extend_from_slice(&reg_data);
        reg_prop_data.extend_from_slice(&reg_size);
        let reg_prop = Property::new("reg", reg_prop_data);
        virtio_node.add_property(reg_prop);

        // Add interrupts property
        let interrupt_data = 1u32.to_be_bytes().to_vec();
        let interrupt_prop = Property::new("interrupts", interrupt_data);
        virtio_node.add_property(interrupt_prop);

        // Add status = "okay"
        let status_prop = Property::new("status", b"okay\0");
        virtio_node.add_property(status_prop);

        root.add_child(virtio_node);
    }

    /// Add virtio console node
    fn add_virtio_console_node(&self, root: &mut Node) {
        let mut console_node = Node::new("virtio_console", 1);

        let compatible_prop = Property::new("compatible", b"virtio,mmio\0");
        console_node.add_property(compatible_prop);

        let reg_data = 0x10001000u64.to_be_bytes().to_vec();
        let reg_size = 0x1000u64.to_be_bytes().to_vec();
        let mut reg_prop_data = Vec::new();
        reg_prop_data.extend_from_slice(&reg_data);
        reg_prop_data.extend_from_slice(&reg_size);
        let reg_prop = Property::new("reg", reg_prop_data);
        console_node.add_property(reg_prop);

        root.add_child(console_node);
    }

    /// Add virtio block nodes
    fn add_virtio_block_nodes(&self, root: &mut Node, count: u32) {
        for i in 0..count {
            let mut block_node = Node::new(&format!("virtio_block{}", i), 1);

            let compatible_prop = Property::new("compatible", b"virtio,mmio\0");
            block_node.add_property(compatible_prop);

            let base_addr = 0x10002000 + (i as u64 * 0x1000);
            let reg_data = base_addr.to_be_bytes().to_vec();
            let reg_size = 0x1000u64.to_be_bytes().to_vec();
            let mut reg_prop_data = Vec::new();
            reg_prop_data.extend_from_slice(&reg_data);
            reg_prop_data.extend_from_slice(&reg_size);
            let reg_prop = Property::new("reg", reg_prop_data);
            block_node.add_property(reg_prop);

            root.add_child(block_node);
        }
    }

    /// Add virtio network nodes
    fn add_virtio_net_nodes(&self, root: &mut Node, count: u32) {
        for i in 0..count {
            let mut net_node = Node::new(&format!("virtio_net{}", i), 1);

            let compatible_prop = Property::new("compatible", b"virtio,mmio\0");
            net_node.add_property(compatible_prop);

            let base_addr = 0x10004000 + (i as u64 * 0x1000);
            let reg_data = base_addr.to_be_bytes().to_vec();
            let reg_size = 0x1000u64.to_be_bytes().to_vec();
            let mut reg_prop_data = Vec::new();
            reg_prop_data.extend_from_slice(&reg_data);
            reg_prop_data.extend_from_slice(&reg_size);
            let reg_prop = Property::new("reg", reg_prop_data);
            net_node.add_property(reg_prop);

            root.add_child(net_node);
        }
    }

    /// Add hypervisor node
    fn add_hypervisor_node(&self, root: &mut Node) {
        let mut hv_node = Node::new("hypervisor", 1);

        let compatible_prop = Property::new("compatible", b"ferrovisor,hypervisor\0");
        hv_node.add_property(compatible_prop);

        let version_prop = Property::new("version", b"1.0\0");
        hv_node.add_property(version_prop);

        root.add_child(hv_node);
    }

    /// Update memory node for VM
    fn update_memory_node(&self, root: &mut Node, vm_config: &VmDeviceTreeConfig) {
        if let Some(mem_node) = root.find_child_mut("memory") {
            // Remove existing reg property
            mem_node.properties.retain(|p| p.name != "reg");

            // Add new reg property for VM memory
            let addr_data = vm_config.memory_base.to_be_bytes().to_vec();
            let size_data = vm_config.memory_size.to_be_bytes().to_vec();
            let mut reg_data = Vec::new();
            reg_data.extend_from_slice(&addr_data);
            reg_data.extend_from_slice(&size_data);
            let reg_prop = Property::new("reg", reg_data);
            mem_node.add_property(reg_prop);
        }
    }

    /// Update CPU node for VM
    fn update_cpu_node(&self, root: &mut Node, vm_config: &VmDeviceTreeConfig) {
        if let Some(cpus_node) = root.find_child_mut("cpus") {
            // Remove all existing CPU nodes
            cpus_node.children.clear();

            // Add VM's CPUs
            for i in 0..vm_config.vcpu_count {
                let mut cpu_node = Node::new(&format!("cpu@{}", i), 2);

                let compatible_prop = Property::new("compatible", b"riscv,cpu\0");
                cpu_node.add_property(compatible_prop);

                let reg_prop = Property::new("reg", (i as u64).to_be_bytes().to_vec());
                cpu_node.add_property(reg_prop);

                let status_prop = Property::new("status", b"okay\0");
                cpu_node.add_property(status_prop);

                cpus_node.add_child(cpu_node);
            }

            // Update #address-cells and #size-cells for cpus node
            let addr_cells_prop = Property::new("#address-cells", 1u32.to_be_bytes().to_vec());
            cpus_node.add_property(addr_cells_prop);

            let size_cells_prop = Property::new("#size-cells", 0u32.to_be_bytes().to_vec());
            cpus_node.add_property(size_cells_prop);
        }
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

/// VM device tree configuration
#[derive(Debug, Clone)]
pub struct VmDeviceTreeConfig {
    /// Number of virtual CPUs
    pub vcpu_count: u32,
    /// Memory base address
    pub memory_base: u64,
    /// Memory size
    pub memory_size: u64,
    /// Enable VirtIO devices
    pub enable_virtio: bool,
    /// Number of VirtIO block devices
    pub virtio_blk_count: u32,
    /// Number of VirtIO network devices
    pub virtio_net_count: u32,
    /// Enable virtual console
    pub enable_console: bool,
    /// Hypervisor features
    pub hypervisor_features: u32,
}

impl Default for VmDeviceTreeConfig {
    fn default() -> Self {
        Self {
            vcpu_count: 1,
            memory_base: 0x80000000,
            memory_size: 0x40000000, // 1GB
            enable_virtio: true,
            virtio_blk_count: 1,
            virtio_net_count: 0,
            enable_console: true,
            hypervisor_features: 0xFFFFFFFF, // All features enabled
        }
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