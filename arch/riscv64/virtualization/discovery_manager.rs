//! RISC-V Virtual Device Discovery Manager
//!
//! This module provides the main device discovery management interface
//! that integrates with the virtualization subsystem.

use crate::arch::riscv64::devtree::{FlattenedDeviceTree, DeviceTreeParser};
use crate::arch::riscv64::virtualization::discovery::{
    VirtualDeviceDiscovery, DeviceClass, DeviceCapability, VirtualDeviceDesc,
    DeviceResources, MemoryRegion, IrqResource, DiscoveryStats, HotplugEvent,
};
use crate::arch::riscv64::virtualization::VmConfig;
use crate::arch::riscv64::virtualization::{VmId, VcpuId};
use crate::core::mm::PhysAddr;
use crate::drivers::{DeviceId, DeviceType, DeviceStatus};
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};

/// Device discovery manager interface
pub trait DeviceDiscoveryManager {
    /// Discover all available devices
    fn discover_all_devices(&mut self) -> Result<Vec<DeviceId>, &'static str>;

    /// Discover devices for a specific VM
    fn discover_vm_devices(&mut self, vm_config: &VmConfig) -> Result<Vec<DeviceId>, &'static str>;

    /// Add a new device dynamically
    fn add_device(&mut self, device_desc: VirtualDeviceDesc) -> Result<DeviceId, &'static str>;

    /// Remove a device
    fn remove_device(&mut self, device_id: DeviceId) -> Result<(), &'static str>;

    /// Get device information
    fn get_device_info(&self, device_id: DeviceId) -> Option<&VirtualDeviceDesc>;

    /// Find compatible devices
    fn find_compatible_devices(&self, compatible: &str) -> Vec<&VirtualDeviceDesc>;

    /// Get discovery statistics
    fn get_stats(&self) -> &DiscoveryStats;

    /// Process hotplug events
    fn process_hotplug(&mut self) -> Result<Vec<DeviceId>, &'static str>;
}

/// Main device discovery manager implementation
pub struct RiscvDeviceDiscoveryManager {
    /// Core discovery engine
    discovery: VirtualDeviceDiscovery,
    /// VM device assignments
    vm_assignments: BTreeMap<VmId, Vec<DeviceId>>,
    /// Global device tree
    global_fdt: Option<FlattenedDeviceTree>,
    /// Next VM ID for assignments
    next_vm_id: AtomicU32,
    /// Device compatibility cache
    compat_cache: BTreeMap<String, Vec<DeviceId>>,
}

impl RiscvDeviceDiscoveryManager {
    /// Create new discovery manager
    pub fn new() -> Self {
        Self {
            discovery: VirtualDeviceDiscovery::new(),
            vm_assignments: BTreeMap::new(),
            global_fdt: None,
            next_vm_id: AtomicU32::new(1),
            compat_cache: BTreeMap::new(),
        }
    }

    /// Initialize with global device tree
    pub fn init_with_fdt(&mut self, fdt: FlattenedDeviceTree) -> Result<(), &'static str> {
        self.global_fdt = Some(fdt.clone());

        // Discover all devices from the device tree
        let discovered_devices = self.discovery.discover_from_device_tree(&fdt)?;

        // Update compatibility cache
        self.update_compat_cache();

        log::info!("Initialized device discovery manager with {} devices", discovered_devices.len());

        Ok(())
    }

    /// Create VM-specific device tree
    pub fn create_vm_device_tree(&self, vm_config: &VmConfig) -> Result<FlattenedDeviceTree, &'static str> {
        let fdt = self.global_fdt.as_ref()
            .ok_or("No global device tree available")?;

        // Create VM-specific device tree
        let mut vm_fdt = fdt.clone();

        // Filter and modify devices for this VM
        self.filter_devices_for_vm(&mut vm_fdt, vm_config)?;

        Ok(vm_fdt)
    }

    /// Filter devices for VM based on configuration
    fn filter_devices_for_vm(&self, fdt: &mut FlattenedDeviceTree, vm_config: &VmConfig) -> Result<(), &'static str> {
        // Remove devices that are not assigned to this VM
        let assigned_devices = self.vm_assignments.get(&vm_config.vm_id);

        // For each device node, check if it's assigned to this VM
        if let Some(root) = fdt.get_root_mut() {
            self.filter_node_recursively(root, assigned_devices, vm_config)?;
        }

        Ok(())
    }

    /// Recursively filter device tree nodes
    fn filter_node_recursively(
        &self,
        node: &mut crate::arch::riscv64::devtree::fdt::Node,
        assigned_devices: Option<&Vec<DeviceId>>,
        vm_config: &VmConfig,
    ) -> Result<bool, &'static str> {
        let mut keep_node = true;

        // Check if node should be kept based on VM configuration
        if let Some(status_prop) = node.get_property("status") {
            let status = core::str::from_utf8(status_prop.data).unwrap_or("");
            if status == "disabled" {
                keep_node = false;
            }
        }

        // Filter children
        node.children.retain(|child| {
            matches!(self.filter_node_recursively(child, assigned_devices, vm_config), Ok(true))
        });

        Ok(keep_node)
    }

    /// Update compatibility cache
    fn update_compat_cache(&mut self) {
        self.compat_cache.clear();

        // Build cache of compatible strings to device IDs
        for (device_id, device_desc) in self.discovery.devices.iter() {
            for compatible_str in &device_desc.compatible {
                self.compat_cache
                    .entry(compatible_str.clone())
                    .or_insert_with(Vec::new)
                    .push(*device_id);
            }
        }
    }

    /// Assign device to VM
    pub fn assign_device_to_vm(&mut self, device_id: DeviceId, vm_id: VmId) -> Result<(), &'static str> {
        // Check if device exists
        if !self.discovery.devices.contains_key(&device_id) {
            return Err("Device not found");
        }

        // Check if device is already assigned to another VM
        for (existing_vm_id, devices) in &self.vm_assignments {
            if *existing_vm_id != vm_id && devices.contains(&device_id) {
                return Err("Device already assigned to another VM");
            }
        }

        // Add device to VM assignment
        self.vm_assignments
            .entry(vm_id)
            .or_insert_with(Vec::new)
            .push(device_id);

        log::info!("Assigned device {:?} to VM {:?}", device_id, vm_id);

        Ok(())
    }

    /// Unassign device from VM
    pub fn unassign_device_from_vm(&mut self, device_id: DeviceId, vm_id: VmId) -> Result<(), &'static str> {
        if let Some(devices) = self.vm_assignments.get_mut(&vm_id) {
            if let Some(pos) = devices.iter().position(|&id| id == device_id) {
                devices.remove(pos);
                log::info!("Unassigned device {:?} from VM {:?}", device_id, vm_id);
                Ok(())
            } else {
                Err("Device not assigned to this VM")
            }
        } else {
            Err("VM not found")
        }
    }

    /// Get devices assigned to VM
    pub fn get_vm_devices(&self, vm_id: VmId) -> Vec<&VirtualDeviceDesc> {
        if let Some(device_ids) = self.vm_assignments.get(&vm_id) {
            device_ids.iter()
                .filter_map(|&device_id| self.discovery.get_device(device_id))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Check device compatibility
    pub fn check_device_compatibility(
        &self,
        device_id: DeviceId,
        required_caps: &[String],
    ) -> bool {
        if let Some(device_desc) = self.discovery.get_device(device_id) {
            // Check if device has all required capabilities
            required_caps.iter().all(|required_cap| {
                device_desc.capabilities.iter()
                    .any(|cap| cap.name == *required_cap)
            })
        } else {
            false
        }
    }

    /// Get device capabilities
    pub fn get_device_capabilities(&self, device_id: DeviceId) -> Option<&[DeviceCapability]> {
        self.discovery.get_device(device_id)
            .map(|device_desc| device_desc.capabilities.as_slice())
    }

    /// Get device resources
    pub fn get_device_resources(&self, device_id: DeviceId) -> Option<&DeviceResources> {
        self.discovery.get_device(device_id)
            .map(|device_desc| &device_desc.resources)
    }

    /// Perform device enumeration
    pub fn enumerate_devices(&self) -> DeviceEnumeration {
        let mut enumeration = DeviceEnumeration::default();

        for (device_id, device_desc) in &self.discovery.devices {
            // Count by device type
            *enumeration.by_type.entry(device_desc.device_type).or_insert(0) += 1;

            // Count by class
            *enumeration.by_class.entry(device_desc.class.id).or_insert(0) += 1;

            // Count by status
            *enumeration.by_status.entry(device_desc.status).or_insert(0) += 1;

            enumeration.total_devices += 1;
        }

        enumeration
    }

    /// Generate device inventory report
    pub fn generate_inventory_report(&self) -> DeviceInventoryReport {
        let mut report = DeviceInventoryReport::default();

        for (device_id, device_desc) in &self.discovery.devices {
            let device_info = DeviceInfo {
                device_id: *device_id,
                name: device_desc.name.clone(),
                device_type: device_desc.device_type,
                class_name: device_desc.class.name.clone(),
                status: device_desc.status,
                vendor_id: device_desc.vendor_id,
                product_id: device_desc.product_id,
                capabilities: device_desc.capabilities.len(),
                memory_regions: device_desc.resources.memory_regions.len(),
                irqs: device_desc.resources.irqs.len(),
            };

            report.devices.push(device_info);

            // Update statistics
            report.total_memory += device_desc.resources.memory_regions.iter()
                .map(|region| region.size)
                .sum::<u64>();

            report.total_irqs += device_desc.resources.irqs.len() as u32;
        }

        report.total_devices = report.devices.len() as u32;

        // Sort devices by name
        report.devices.sort_by(|a, b| a.name.cmp(&b.name));

        report
    }
}

impl DeviceDiscoveryManager for RiscvDeviceDiscoveryManager {
    fn discover_all_devices(&mut self) -> Result<Vec<DeviceId>, &'static str> {
        if let Some(fdt) = &self.global_fdt {
            let discovered_devices = self.discovery.discover_from_device_tree(fdt)?;
            self.update_compat_cache();
            Ok(discovered_devices)
        } else {
            Err("No device tree available for discovery")
        }
    }

    fn discover_vm_devices(&mut self, vm_config: &VmConfig) -> Result<Vec<DeviceId>, &'static str> {
        // Discover all devices first
        let all_devices = self.discover_all_devices()?;

        // Filter devices for this VM based on configuration
        let vm_devices: Vec<DeviceId> = all_devices.into_iter()
            .filter(|device_id| {
                // Check if device is compatible with VM requirements
                self.check_device_compatibility(*device_id, &vm_config.required_capabilities)
            })
            .collect();

        // Assign discovered devices to VM
        for &device_id in &vm_devices {
            let _ = self.assign_device_to_vm(device_id, vm_config.vm_id);
        }

        Ok(vm_devices)
    }

    fn add_device(&mut self, device_desc: VirtualDeviceDesc) -> Result<DeviceId, &'static str> {
        let device_id = device_desc.device_id;
        self.discovery.devices.insert(device_id, device_desc);
        self.update_compat_cache();

        // Trigger hotplug event
        self.discovery.add_hotplug_event(HotplugEvent::DeviceAdd(device_desc));

        Ok(device_id)
    }

    fn remove_device(&mut self, device_id: DeviceId) -> Result<(), &'static str> {
        // Remove from all VM assignments
        for (_, devices) in self.vm_assignments.iter_mut() {
            devices.retain(|&id| id != device_id);
        }

        // Remove from discovery
        self.discovery.devices.remove(&device_id);

        // Update cache
        self.update_compat_cache();

        // Trigger hotplug event
        self.discovery.add_hotplug_event(HotplugEvent::DeviceRemove(device_id));

        Ok(())
    }

    fn get_device_info(&self, device_id: DeviceId) -> Option<&VirtualDeviceDesc> {
        self.discovery.get_device(device_id)
    }

    fn find_compatible_devices(&self, compatible: &str) -> Vec<&VirtualDeviceDesc> {
        // Use cache first
        if let Some(device_ids) = self.compat_cache.get(compatible) {
            device_ids.iter()
                .filter_map(|&device_id| self.discovery.get_device(device_id))
                .collect()
        } else {
            // Fallback to linear search
            self.discovery.find_devices_by_compatible(compatible)
        }
    }

    fn get_stats(&self) -> &DiscoveryStats {
        self.discovery.get_stats()
    }

    fn process_hotplug(&mut self) -> Result<Vec<DeviceId>, &'static str> {
        let new_devices = self.discovery.process_hotplug_events()?;
        self.update_compat_cache();
        Ok(new_devices)
    }
}

/// Device enumeration results
#[derive(Debug, Default)]
pub struct DeviceEnumeration {
    /// Total number of devices
    pub total_devices: usize,
    /// Devices by type
    pub by_type: BTreeMap<DeviceType, usize>,
    /// Devices by class ID
    pub by_class: BTreeMap<u32, usize>,
    /// Devices by status
    pub by_status: BTreeMap<DeviceStatus, usize>,
}

/// Device inventory report
#[derive(Debug, Default)]
pub struct DeviceInventoryReport {
    /// Total number of devices
    pub total_devices: u32,
    /// Total memory regions
    pub total_memory: u64,
    /// Total IRQs
    pub total_irqs: u32,
    /// Device information
    pub devices: Vec<DeviceInfo>,
}

/// Device information for inventory
#[derive(Debug)]
pub struct DeviceInfo {
    /// Device ID
    pub device_id: DeviceId,
    /// Device name
    pub name: String,
    /// Device type
    pub device_type: DeviceType,
    /// Class name
    pub class_name: String,
    /// Device status
    pub status: DeviceStatus,
    /// Vendor ID
    pub vendor_id: u32,
    /// Product ID
    pub product_id: u32,
    /// Number of capabilities
    pub capabilities: usize,
    /// Number of memory regions
    pub memory_regions: usize,
    /// Number of IRQs
    pub irqs: usize,
}

impl core::fmt::Display for DeviceEnumeration {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "Device Enumeration ({} total devices)", self.total_devices)?;
        writeln!(f, "  By type:")?;
        for (device_type, count) in &self.by_type {
            writeln!(f, "    {}: {}", device_type.to_string(), count)?;
        }
        writeln!(f, "  By class:")?;
        for (class_id, count) in &self.by_class {
            writeln!(f, "    Class {}: {}", class_id, count)?;
        }
        writeln!(f, "  By status:")?;
        for (status, count) in &self.by_status {
            writeln!(f, "    {:?}: {}", status, count)?;
        }
        Ok(())
    }
}

impl core::fmt::Display for DeviceInventoryReport {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "Device Inventory Report")?;
        writeln!(f, "======================")?;
        writeln!(f, "Total devices: {}", self.total_devices)?;
        writeln!(f, "Total memory: {} MB", self.total_memory / (1024 * 1024))?;
        writeln!(f, "Total IRQs: {}", self.total_irqs)?;
        writeln!(f, "")?;
        writeln!(f, "Device List:")?;
        for device in &self.devices {
            writeln!(f, "  {:?}: {} ({}, {})",
                    device.device_id,
                    device.name,
                    device.device_type.to_string(),
                    device.class_name)?;
        }
        Ok(())
    }
}