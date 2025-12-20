//! RISC-V Device Tree Modifier
//!
//! This module provides device tree modification functionality including:
//! - Adding/removing nodes and properties
/// - Property value modification
/// - Device tree generation for guests
/// - FDT serialization

use crate::arch::riscv64::*;
use crate::arch::riscv64::devtree::fdt::*;
use bitflags::bitflags;

/// Modification flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ModifyFlags: u32 {
        /// Allow modifying read-only properties
        const READ_ONLY = 1 << 0;
        /// Allow adding new properties
        const ADD_PROPERTIES = 1 << 1;
        /// Allow removing properties
        const REMOVE_PROPERTIES = 1 << 2;
        /// Allow adding new nodes
        const ADD_NODES = 1 << 3;
        /// Allow removing nodes
        const REMOVE_NODES = 1 << 4;
    }
}

impl Default for ModifyFlags {
    fn default() -> Self {
        Self::all()
    }
}

/// Device tree modifier
pub struct DeviceTreeModifier {
    /// FDT to modify
    fdt: FlattenedDeviceTree,
    /// Modification flags
    flags: ModifyFlags,
    /// Current node path stack
    path_stack: Vec<String>,
    /// Modification log
    modifications: Vec<String>,
}

impl DeviceTreeModifier {
    /// Create new modifier
    pub fn new(fdt: FlattenedDeviceTree) -> Self {
        Self {
            fdt,
            flags: ModifyFlags::default(),
            path_stack: Vec::new(),
            modifications: Vec::new(),
        }
    }

    /// Create new modifier with flags
    pub fn new_with_flags(fdt: FlattenedDeviceTree, flags: ModifyFlags) -> Self {
        Self {
            fdt,
            flags,
            path_stack: Vec::new(),
            modifications: Vec::new(),
        }
    }

    /// Enter a node path
    pub fn enter_path(&mut self, path: &str) -> Result<(), &'static str> {
        if let Some(node) = self.fdt.find_node(path) {
            self.path_stack.push(path.to_string());
            self.modifications.push(format!("Enter path: {}", path));
            Ok(())
        } else {
            Err("Node not found")
        }
    }

    /// Exit current node
    pub fn exit_path(&mut self) -> Result<(), &'static str> {
        if self.path_stack.pop().is_some() {
            self.modifications.push("Exit current path".to_string());
            Ok(())
        } else {
            Err("No path to exit")
        }
    }

    /// Get current path
    pub fn get_current_path(&self) -> String {
        if let Some(last) = self.path_stack.last() {
            last.clone()
        } else {
            "/".to_string()
        }
    }

    /// Add a property to current node
    pub fn add_property(&mut self, name: &str, data: Vec<u8>) -> Result<(), &'static str> {
        if !self.flags.contains(ModifyFlags::ADD_PROPERTIES) {
            return Err("Adding properties not allowed");
        }

        if self.path_stack.is_empty() {
            return Err("No current node");
        }

        let current_path = self.get_current_path();
        if let Some(node) = self.fdt.find_node_mut(&current_path) {
            let prop = Property::new(name, data);
            node.add_property(prop);
            self.modifications.push(format!("Add property: {} ({} bytes)", name, data.len()));
            Ok(())
        } else {
            Err("Current node not found")
        }
    }

    /// Set a string property
    pub fn set_property_string(&mut self, name: &str, value: &str) -> Result<(), &'static str> {
        let mut data = value.as_bytes().to_vec();
        data.push(0); // Null terminate
        self.add_property(name, data)
    }

    /// Set a u32 property
    pub fn set_property_u32(&mut self, name: &str, value: u32) -> Result<(), &'static str> {
        let data = value.to_be_bytes().to_vec();
        self.add_property(name, data)
    }

    /// Set a u64 property
    pub fn set_property_u64(&mut self, name: &str, value: u64) -> Result<(), &'static str> {
        let data = value.to_be_bytes().to_vec();
        self.add_property(name, data)
    }

    /// Set a phandle property
    pub fn set_phandle(&mut self, phandle: u32) -> Result<(), &'static str> {
        self.set_property_u32("phandle", phandle)
    }

    /// Remove a property from current node
    pub fn remove_property(&mut self, name: &str) -> Result<(), &'static str> {
        if !self.flags.contains(ModifyFlags::REMOVE_PROPERTIES) {
            return Err("Removing properties not allowed");
        }

        if self.path_stack.is_empty() {
            return Err("No current node");
        }

        let current_path = self.get_current_path();
        if let Some(node) = self.fdt.find_node_mut(&current_path) {
            // Find and remove property
            let original_len = node.properties.len();
            node.properties.retain(|p| p.name != name);
            let removed = original_len != node.properties.len();

            if removed {
                self.modifications.push(format!("Remove property: {}", name));
                Ok(())
            } else {
                Err("Property not found")
            }
        } else {
            Err("Current node not found")
        }
    }

    /// Add a child node
    pub fn add_child_node(&mut self, name: &str) -> Result<(), &'static str> {
        if !self.flags.contains(ModifyFlags::ADD_NODES) {
            return Err("Adding nodes not allowed");
        }

        if self.path_stack.is_empty() {
            return Err("No current node");
        }

        let current_path = self.get_current_path();
        if let Some(parent) = self.fdt.find_node_mut(&current_path) {
            let depth = parent.depth + 1;
            let child = Node::new(name, depth);
            let child_index = parent.add_child(child);

            // Set parent reference for child
            if let Some(parent_children) = parent.children.get_mut(child_index) {
                parent_children.parent = Some(child_index);
            }

            self.modifications.push(format!("Add child node: {}", name));
            Ok(())
        } else {
            Err("Current node not found")
        }
    }

    /// Remove a child node
    pub fn remove_child_node(&mut self, name: &str) -> Result<(), &'static str> {
        if !self.flags.contains(ModifyFlags::REMOVE_NODES) {
            return Err("Removing nodes not allowed");
        }

        if self.path_stack.is_empty() {
            return Err("No current node");
        }

        let current_path = self.get_current_path();
        if let Some(parent) = self.fdt.find_node_mut(&current_path) {
            let original_len = parent.children.len();
            parent.children.retain(|c| c.name != name);
            let removed = original_len != parent.children.len();

            if removed {
                self.modifications.push(format!("Remove child node: {}", name));
                Ok(())
            } else {
                Err("Child node not found")
            }
        } else {
            Err("Current node not found")
        }
    }

    /// Get modification log
    pub fn get_modifications(&self) -> &[String] {
        &self.modifications
    }

    /// Get modified FDT
    pub fn get_fdt(&self) -> &FlattenedDeviceTree {
        &self.fdt
    }

    /// Get mutable FDT
    pub fn get_fdt_mut(&mut self) -> &mut FlattenedDeviceTree {
        &mut self.fdt
    }

    /// Serialize modifications back to FDT format
    pub fn serialize(&mut self) -> Result<Vec<u8>, &'static str> {
        // This would serialize the modified tree structure back to FDT format
        // For now, just return the original data
        Ok(self.fdt.serialize()?)
    }

    /// Clear all modifications
    pub fn clear_modifications(&mut self) {
        self.modifications.clear();
    }

    /// Check if modifications were made
    pub fn has_modifications(&self) -> bool {
        !self.modifications.is_empty()
    }

    /// Revert to original FDT
    pub fn revert(&mut self, original: FlattenedDeviceTree) {
        self.fdt = original;
        self.clear_modifications();
        self.path_stack.clear();
    }
}

/// Utility functions for common device tree modifications
pub mod utils {
    use super::*;

    /// Set boot arguments
    pub fn set_boot_args(
        fdt: &mut FlattenedDeviceTree,
        bootargs: &str,
    ) -> Result<(), &'static str> {
        if let Some(chosen) = fdt.find_node_mut("/chosen") {
            chosen.add_property(Property::new("bootargs", {
                let mut data = bootargs.as_bytes().to_vec();
                data.push(0); // Null terminate
                data
            }));
            Ok(())
        } else {
            Err("/chosen node not found")
        }
    }

    /// Set initrd start and end
    pub fn set_initrd(
        fdt: &mut FlattenedDeviceTree,
        start: u64,
        end: u64,
    ) -> Result<(), &'static str> {
        if let Some(chosen) = fdt.find_node_mut("/chosen") {
            chosen.add_property(Property::new("linux,initrd-start", start.to_be_bytes().to_vec()));
            chosen.add_property(Property::new("linux,initrd-end", end.to_be_bytes().to_vec()));
            Ok(())
        } else {
            Err("/chosen node not found")
        }
    }

    /// Set kernel command line
    pub fn set_kernel_cmdline(
        fdt: &mut FlattenedDeviceTree,
        cmdline: &str,
    ) -> Result<(), &'static str> {
        if let Some(chosen) = fdt.find_node_mut("/chosen") {
            chosen.add_property(Property::new("bootargs", {
                let mut data = cmdline.as_bytes().to_vec();
                data.push(0); // Null terminate
                data
            }));
            Ok(())
        } else {
            Err("/chosen node not found")
        }
    }

    /// Disable a device by setting status to "disabled"
    pub fn disable_device(
        fdt: &mut FlattenedDeviceTree,
        path: &str,
    ) -> Result<(), &'static str> {
        if let Some(node) = fdt.find_node_mut(path) {
            // Remove existing status property if any
            node.properties.retain(|p| p.name != "status");
            // Add disabled status
            node.add_property(Property::new("status", b"disabled\0"));
            Ok(())
        } else {
            Err("Node not found")
        }
    }

    /// Enable a device by setting status to "okay"
    pub fn enable_device(
        fdt: &mut FlattenedDeviceTree,
        path: &str,
    ) -> Result<(), &'static str> {
        if let Some(node) = fdt.find_node_mut(path) {
            // Remove existing status property if any
            node.properties.retain(|p| p.name != "status");
            // Add okay status
            node.add_property(Property::new("status", b"okay\0"));
            Ok(())
        } else {
            Err("Node not found")
        }
    }

    /// Set CPU clock frequency
    pub fn set_cpu_clock(
        fdt: &mut FlattenedDeviceTree,
        cpu_id: u32,
        frequency: u32,
    ) -> Result<(), &'static str> {
        let path = format!("/cpus/cpu@{:x}", cpu_id);
        if let Some(cpu_node) = fdt.find_node_mut(&path) {
            cpu_node.add_property(Property::new("clock-frequency", frequency.to_be_bytes().to_vec()));
            Ok(())
        } else {
            Err("CPU node not found")
        }
    }

    /// Set memory size
    pub fn set_memory_size(
        fdt: &mut FlattenedDeviceTree,
        address: u64,
        size: u64,
    ) -> Result<(), &'static str> {
        if let Some(memory_node) = fdt.find_node_mut("/memory") {
            // Remove existing reg property
            memory_node.properties.retain(|p| p.name != "reg");
            // Add new reg property (address, size)
            let mut data = Vec::new();
            data.extend_from_slice(&address.to_be_bytes());
            data.extend_from_slice(&size.to_be_bytes());
            memory_node.add_property(Property::new("reg", data));
            Ok(())
        } else {
            Err("/memory node not found")
        }
    }

    /// Create a simple device tree
    pub fn create_simple_fdt(
        bootargs: &str,
        initrd_start: Option<u64>,
        initrd_end: Option<u64>,
        memory_base: u64,
        memory_size: u64,
        cpu_count: u32,
        cpu_frequency: u32,
    ) -> Result<Vec<u8>, &'static str> {
        // Start with minimal header
        let mut data = Vec::new();

        // Header (will be filled later)
        for _ in 0..core::mem::size_of::<FdtHeader>() {
            data.push(0);
        }

        // Memory reserve map (just end marker)
        for _ in 0..core::mem::size_of::<MemReserveEntry>() {
            data.push(0);
        }

        let struct_block_offset = data.len();

        // Begin root node
        data.extend_from_slice(&[0x00, 00, 00, 01]); // FDT_BEGIN_NODE

        // Add chosen node
        data.extend_from_slice(&[0x00, 00, 00, 01]); // FDT_BEGIN_NODE
        let chosen_name_offset = data.len() - struct_block_offset;
        data.extend_from_slice("chosen".as_bytes());
        data.push(0); // Null terminator
        data.push(0);
        data.push(0);
        data.push(0); // Alignment

        // bootargs property
        let bootargs_data = format!("bootargs\0{}", bootargs);
        data.extend_from_slice(&[0x00, 00, 00, 03]); // FDT_PROP
        let bootargs_name_offset = data.len() - struct_block_offset;
        data.extend_from_slice(&bootargs_data.len().to_be_bytes());
        data.extend_from_slice(bootargs_data.as_bytes());

        // initrd-start property (if provided)
        if let Some(start) = initrd_start {
            data.extend_from_slice(&[0x00, 00, 00, 03]); // FDT_PROP
            let initrd_start_name_offset = data.len() - struct_block_offset;
            data.extend_from_slice(&20u32.to_be_bytes()); // Property length
            data.extend_from_slice(&start.to_be_bytes());
        }

        // initrd-end property (if provided)
        if let Some(end) = initrd_end {
            data.extend_from_slice(&[0x00, 00, 00, 03]); // FDT_PROP
            let initrd_end_name_offset = data.len() - struct_block_offset;
            data.extend_from_slice(&18u32.to_be_bytes()); // Property length
            data.extend_from_slice(&end.to_be_bytes());
        }

        data.extend_from_slice(&[0x00, 00, 00, 02]); // FDT_END_NODE

        // Add memory node
        data.extend_from_slice(&[0x00, 00, 00, 01]); // FDT_BEGIN_NODE
        let memory_name_offset = data.len() - struct_block_offset;
        data.extend_from_slice("memory".as_bytes());
        data.push(0); // Null terminator
        data.push(0);
        data.push(0);
        data.push(0); // Alignment

        // device_type property
        data.extend_from_slice(&[0x00, 00, 00, 03]); // FDT_PROP
        let device_type_name_offset = data.len() - struct_block_offset;
        data.extend_from_slice(&19u32.to_be_bytes()); // Property length
        data.extend_from_slice("memory\0".as_bytes());

        // reg property
        data.extend_from_slice(&[0x00, 00, 00, 03]); // FDT_PROP
        let reg_name_offset = data.len() - struct_block_offset;
        data.extend_from_slice(&16u32.to_be_bytes()); // Property length
        data.extend_from_slice(&memory_base.to_be_bytes());
        data.extend_from_slice(&memory_size.to_be_bytes());

        data.extend_from_slice(&[0x00, 00, 00, 02]); // FDT_END_NODE

        // Add cpus node
        data.extend_from_slice(&[0x00, 00, 00, 01]); // FDT_BEGIN_NODE
        let cpus_name_offset = data.len() - struct_block_offset;
        data.extend_from_slice("cpus".as_bytes());
        data.push(0); // Null terminator
        data.push(0);
        data.push(0);
        data.push(0); // Alignment

        // #address-cells
        data.extend_from_slice(&[0x00, 00, 00, 03]); // FDT_PROP
        let addr_cells_name_offset = data.len() - struct_block_offset;
        data.extend_from_slice(&16u32.to_be_bytes()); // Property length
        data.extend_from_slice(&2u32.to_be_bytes());

        // #size-cells
        data.extend_from_slice(&[0x00, 00, 00, 03]); // FDT_PROP
        let size_cells_name_offset = data.len() - struct_block_offset;
        data.extend_from_slice(&16u32.to_be_bytes()); // Property length
        data.extend_from_slice(&2u32.to_be_bytes());

        // Add CPU nodes
        for i in 0..cpu_count {
            data.extend_from_slice(&[0x00, 00, 00, 01]); // FDT_BEGIN_NODE
            let cpu_name_offset = data.len() - struct_block_offset;
            data.extend_from_slice(format!("cpu@{:x}", i).as_bytes());
            data.push(0); // Null terminator
            data.extend_from_slice(&[0, 0, 0, 0]); // Alignment to 4

            // device_type
            data.extend_from_slice(&[0x00, 00, 00, 03]); // FDT_PROP
            let cpu_device_type_offset = data.len() - struct_block_offset;
            data.extend_from_slice(&18u32.to_be_bytes());
            data.extend_from_slice("cpu\0".as_bytes());

            // clock-frequency
            data.extend_from_slice(&[0x00, 00, 00, 03]); // FDT
            let cpu_clock_offset = data.len() - struct_block_offset;
            data.extend_from_slice(&20u32.to_be_bytes());
            data.extend_from_slice(&cpu_frequency.to_be_bytes());

            // status
            data.extend_from_slice(&[0x00, 00, 00, 03]); // FDT
            let cpu_status_offset = data.len() - struct_block_offset;
            data.extend_from_slice(&12u32.to_be_bytes());
            data.extend_from_slice("okay\0".as_bytes());

            data.extend_from_slice(&[0x00, 00, 00, 02]); // FDT_END_NODE
        }

        data.extend_from_slice(&[0x00, 00, 00, 02]); // FDT_END_NODE (cpus)

        // End of structure block
        data.extend_from_slice(&[0x00, 00, 00, 09]); // FDT_END

        // Strings block
        let strings_offset = data.len();

        // Add strings
        data.extend_from_slice("chosen\0");
        data.extend_from_slice("device_type\0");
        data.extend_from_slice("memory\0");
        data.extend_from_slice("reg\0");
        data.extend_from_slice("cpus\0");
        data.extend_from_slice("#address-cells\0");
        data.extend_from_slice("#size-cells\0");
        data.extend_from_slice("cpu\0");
        data.extend_from_slice("clock-frequency\0");
        data.extend_from_slice("status\0");

        // Calculate offsets and fill header
        let totalsize = data.len();
        let off_dt_struct = struct_block_offset;
        let off_dt_strings = strings_offset;
        let off_mem_rsvmap = core::mem::size_of::<FdtHeader>();

        // Update header
        {
            let header = unsafe { &mut *(data.as_mut_ptr() as *mut FdtHeader) };
            header.magic = 0xd00dfeed;
            header.totalsize = totalsize as u32;
            header.off_dt_struct = off_dt_struct as u32;
            header.off_dt_strings = off_dt_strings as u32;
            header.off_mem_rsvmap = off_mem_rsvmap as u32;
            header.version = 17;
            header.last_comp_version = 16;
            header.boot_cpuid_phys = 0;
            header.size_dt_strings = (totalsize - strings_offset) as u32;
            header.size_dt_struct = (off_dt_strings - struct_block_offset) as u32;
        }

        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modifier_flags() {
        let flags = ModifyFlags::default();
        assert!(flags.contains(ModifyFlags::READ_ONLY));
        assert!(flags.contains(ModifyFlags::ADD_PROPERTIES));
        assert!(flags.contains(ModifyFlags::REMOVE_PROPERTIES));
        assert!(flags.contains(ModifyFlags::ADD_NODES));
        assert!(flags.contains(ModifyFlags::REMOVE_NODES));
    }

    #[test]
    fn test_modifier() {
        // Create a simple FDT for testing
        let fdt_data = utils::create_simple_fdt(
            "console=ttyS0",
            Some(0x80000000),
            Some(0x80100000),
            0x40000000,
            0x10000000,
            1,
            1000000,
        ).unwrap();

        let mut fdt = FlattenedDeviceTree::from_bytes(fdt_data).unwrap();
        let mut modifier = DeviceTreeModifier::new(fdt);

        // Test navigation
        assert!(modifier.enter_path("/chosen").is_ok());
        assert_eq!(modifier.get_current_path(), "/chosen");

        // Test adding properties
        assert!(modifier.set_property_u32("test-value", 0x12345678).is_ok());
        assert!(modifier.add_property("string-prop", b"test\0").is_ok());

        // Test removing properties
        assert!(modifier.remove_property("test-value").is_ok());

        // Test adding child nodes
        assert!(modifier.exit_path().is_ok());
        assert!(modifier.enter_path("/").is_ok());
        assert!(modifier.add_child_node("test-node").is_ok());

        // Check modifications
        assert!(modifier.has_modifications());
        let mods = modifier.get_modifications();
        assert!(!mods.is_empty());
    }
}