//! RISC-V VirtIO Manager
//!
//! This module provides the main VirtIO management system that integrates
//! with the virtualization subsystem and manages the complete VirtIO device
//! lifecycle based on xvisor patterns.

use crate::arch::riscv64::virtualization::{VmId, VcpuId};
use crate::arch::riscv64::virtualization::vm::{VirtualDevice, VmDeviceConfig, VirtualMachine};
use crate::arch::riscv64::virtualization::virtio_framework::{
    VirtIODevice, VirtIODeviceConfig, VirtIODeviceType, VirtIODeviceFactory
};
use crate::arch::riscv64::virtualization::virtio_driver::{
    VirtIODriver, VirtIODriverRegistry, VirtIONetDriver, VirtIOBlockDriver,
    VirtIODeviceStats, VirtIODriverStats, VirtIONetConfig, VirtIOBlockConfig
};
use crate::arch::riscv64::virtualization::discovery::{
    VirtualDeviceDesc, DeviceDiscoveryManager, RiscvDeviceDiscoveryManager
};
use crate::arch::riscv64::virtualization::discovery_manager::DeviceDiscoveryManagerExt;
use crate::drivers::{DeviceId, DeviceType, DeviceStatus};
use crate::core::mm::{PhysAddr, VirtAddr};
use crate::core::sync::SpinLock;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// VirtIO manager - central coordination point for all VirtIO operations
pub struct VirtIOManager {
    /// Driver registry
    driver_registry: VirtIODriverRegistry,
    /// Active VirtIO devices
    devices: BTreeMap<DeviceId, Arc<SpinLock<VirtIODevice>>>,
    /// Device configurations
    device_configs: BTreeMap<DeviceId, VirtIODeviceConfig>,
    /// VM device assignments
    vm_devices: BTreeMap<VmId, Vec<DeviceId>>,
    /// Statistics
    stats: VirtIOManagerStats,
    /// Next device ID
    next_device_id: AtomicU32,
    /// Discovery manager reference
    discovery_manager: Option<*mut RiscvDeviceDiscoveryManager>,
}

/// VirtIO manager statistics
#[derive(Debug, Default)]
pub struct VirtIOManagerStats {
    /// Total devices created
    pub total_devices_created: AtomicU32,
    /// Total devices destroyed
    pub total_devices_destroyed: AtomicU32,
    /// Total MMIO operations
    pub total_mmio_operations: AtomicU64,
    /// Total interrupts handled
    pub total_interrupts_handled: AtomicU64,
    /// Total bytes transferred
    pub total_bytes_transferred: AtomicU64,
    /// Driver statistics
    pub driver_stats: VirtIODriverRegistryStats,
    /// Device type distribution
    pub device_types: BTreeMap<VirtIODeviceType, AtomicU32>,
}

/// VirtIO manager configuration
#[derive(Debug, Clone)]
pub struct VirtIOManagerConfig {
    /// Enable hotplug support
    pub enable_hotplug: bool,
    /// Maximum number of devices
    pub max_devices: u32,
    /// Default network configuration
    pub default_net_config: VirtIONetConfig,
    /// Default block configuration
    pub default_block_config: VirtIOBlockConfig,
    /// Enable device sharing between VMs
    pub enable_device_sharing: bool,
    /// Enable device statistics collection
    pub enable_statistics: bool,
}

impl Default for VirtIOManagerConfig {
    fn default() -> Self {
        Self {
            enable_hotplug: true,
            max_devices: 64,
            default_net_config: VirtIONetConfig {
                max_packet_size: 1518,
                rx_buffers: 256,
                tx_buffers: 256,
                promiscuous: false,
                multicast: true,
                checksum_offload: true,
                tso: true,
                ufo: true,
            },
            default_block_config: VirtIOBlockConfig {
                block_size: 512,
                max_segments: 128,
                flush: true,
                discard: true,
                write_zeroes: true,
            },
            enable_device_sharing: false,
            enable_statistics: true,
        }
    }
}

/// VirtIO device creation request
#[derive(Debug)]
pub struct VirtIODeviceRequest {
    /// Device type
    pub device_type: VirtIODeviceType,
    /// VM ID (if device is for specific VM)
    pub vm_id: Option<VmId>,
    /// Device name
    pub name: Option<String>,
    /// Device-specific configuration
    pub config: Option<Vec<u8>>,
    /// MMIO base address (None for auto-allocation)
    pub mmio_base: Option<PhysAddr>,
    /// MMIO size
    pub mmio_size: u64,
    /// Interrupt line (None for auto-allocation)
    pub interrupt: Option<u32>,
}

impl VirtIOManager {
    /// Create new VirtIO manager
    pub fn new(config: VirtIOManagerConfig) -> Self {
        let mut manager = Self {
            driver_registry: VirtIODriverRegistry::new(),
            devices: BTreeMap::new(),
            device_configs: BTreeMap::new(),
            vm_devices: BTreeMap::new(),
            stats: VirtIOManagerStats::default(),
            next_device_id: AtomicU32::new(1),
            discovery_manager: None,
        };

        // Initialize default drivers
        manager.init_default_drivers();

        manager
    }

    /// Initialize default drivers
    fn init_default_drivers(&mut self) {
        // Register network driver
        let net_driver = Arc::new(VirtIONetDriver::new());
        let _ = self.driver_registry.register_driver(net_driver);

        // Register block driver
        let block_driver = Arc::new(VirtIOBlockDriver::new());
        let _ = self.driver_registry.register_driver(block_driver);

        log::info!("Initialized default VirtIO drivers");
    }

    /// Set discovery manager
    pub fn set_discovery_manager(&mut self, discovery_manager: &mut RiscvDeviceDiscoveryManager) {
        self.discovery_manager = Some(discovery_manager);
    }

    /// Create VirtIO device from request
    pub fn create_device(&mut self, request: VirtIODeviceRequest) -> Result<DeviceId, &'static str> {
        // Check device limit
        if self.devices.len() >= 64 { // Configurable limit
            return Err("Maximum number of devices reached");
        }

        // Generate device ID
        let device_id = DeviceId::from(self.next_device_id.fetch_add(1, Ordering::Relaxed));

        // Create device description
        let device_desc = self.create_device_description(&request, device_id)?;

        // Create VirtIO device
        let mut device = VirtIODeviceFactory::create_device(&device_desc)?;

        // Assign to VM if specified
        if let Some(vm_id) = request.vm_id {
            device.vm_id = Some(vm_id);
            self.vm_devices.entry(vm_id).or_insert_with(Vec::new).push(device_id);
        }

        // Register with discovery manager
        if let Some(discovery_manager) = self.discovery_manager {
            unsafe {
                let dm = &mut *discovery_manager;
                let _ = dm.add_device(device_desc);
            }
        }

        // Probe and bind device with driver
        let _driver = self.driver_registry.probe_device(&mut device)?;

        // Store device
        self.devices.insert(device_id, Arc::new(SpinLock::new(device)));
        self.device_configs.insert(device_id, device.config.clone());

        // Update statistics
        self.stats.total_devices_created.fetch_add(1, Ordering::Relaxed);
        self.stats.device_types.entry(request.device_type)
            .or_insert_with(|| AtomicU32::new(0))
            .fetch_add(1, Ordering::Relaxed);

        log::info!("Created VirtIO {:?} device with ID {:?}", request.device_type, device_id);

        Ok(device_id)
    }

    /// Create device description from request
    fn create_device_description(&self, request: &VirtIODeviceRequest, device_id: DeviceId) -> Result<VirtualDeviceDesc, &'static str> {
        // Determine device type
        let device_type = match request.device_type {
            VirtIODeviceType::Network => DeviceType::Network,
            VirtIODeviceType::Block => DeviceType::Block,
            VirtIODeviceType::Console => DeviceType::Console,
            VirtIODeviceType::Rng => DeviceType::Rng,
            VirtIODeviceType::GPU => DeviceType::Graphics,
            VirtIODeviceType::Input => DeviceType::Input,
            _ => DeviceType::Virtual,
        };

        // Create memory region
        use crate::arch::riscv64::virtualization::discovery::{MemoryRegion, MemoryType, MemoryFlags};
        let memory_regions = vec![MemoryRegion {
            name: "mmio".to_string(),
            base: request.mmio_base,
            size: request.mmio_size,
            alignment: 0x1000,
            mem_type: MemoryType::Device,
            flags: MemoryFlags {
                readable: true,
                writable: true,
                executable: false,
                cacheable: false,
                shareable: false,
            },
        }];

        // Create IRQ resource
        use crate::arch::riscv64::virtualization::discovery::{IrqResource, IrqType, IrqTrigger};
        let irqs = vec![IrqResource {
            name: "device".to_string(),
            irq_type: IrqType::Legacy,
            irq_num: request.interrupt,
            trigger: IrqTrigger::EdgeRising,
            priority: 1,
            affinity: u64::MAX,
        }];

        use crate::arch::riscv64::virtualization::discovery::DeviceClass;
        let device_desc = VirtualDeviceDesc {
            device_id,
            name: request.name.clone().unwrap_or_else(|| format!("virtio-{}", request.device_type.as_str())),
            class: DeviceClass {
                name: "virtio".to_string(),
                id: device_type as u32,
                parent_id: Some(0),
                required_caps: vec![],
                optional_caps: vec![],
                compatible: vec![format!("virtio,{}", request.device_type.as_str())],
            },
            device_type,
            vendor_id: 0x1AF4, // VirtIO vendor ID
            product_id: request.device_type as u32,
            capabilities: vec![],
            compatible: vec![format!("virtio,{}", request.device_type.as_str())],
            dt_path: None,
  use crate::arch::riscv64::virtualization::discovery::DeviceResources;
            resources: DeviceResources {
                memory_regions,
                irqs,
                ..Default::default()
            },
            status: DeviceStatus::Present,
        };

        Ok(device_desc)
    }

    /// Destroy VirtIO device
    pub fn destroy_device(&mut self, device_id: DeviceId) -> Result<(), &'static str> {
        let device = self.devices.remove(&device_id)
            .ok_or("Device not found")?;

        let mut device_guard = device.lock();

        // Unbind from driver
        let _driver = self.driver_registry.remove_device(&mut device_guard)?;

        // Remove from VM assignments
        if let Some(vm_id) = device_guard.vm_id {
            if let Some(devices) = self.vm_devices.get_mut(&vm_id) {
                devices.retain(|&id| id != device_id);
            }
        }

        // Remove from discovery manager
        if let Some(discovery_manager) = self.discovery_manager {
            unsafe {
                let dm = &mut *discovery_manager;
                let _ = dm.remove_device(device_id);
            }
        }

        // Clean up
        self.device_configs.remove(&device_id);

        // Update statistics
        self.stats.total_devices_destroyed.fetch_add(1, Ordering::Relaxed);

        log::info!("Destroyed VirtIO device {:?}", device_id);

        Ok(())
    }

    /// Handle MMIO access for VirtIO device
    pub fn handle_mmio(&mut self, device_id: DeviceId, gpa: usize, is_write: bool, value: u64) -> Result<u64, &'static str> {
        let device = self.devices.get(&device_id)
            .ok_or("Device not found")?;

        let driver = self.driver_registry.get_driver_for_device(device_id)
            .ok_or("No driver bound to device")?;

        // Update statistics
        self.stats.total_mmio_operations.fetch_add(1, Ordering::Relaxed);

        // Handle MMIO through driver
        let mut device_guard = device.lock();
        let result = match Arc::try_unwrap(driver) {
            Ok(mut driver) => {
                // This is a bit of a hack - in reality we'd need a different approach
                driver.handle_mmio(&mut device_guard, gpa, is_write, value)
            }
            Err(driver_arc) => {
                // Can't unwrap, need to use Arc references
                // This would require changing the trait to accept &Arc<dyn VirtIODriver>
                Err("Driver sharing not implemented")
            }
        };

        result
    }

    /// Handle interrupt for VirtIO device
    pub fn handle_interrupt(&mut self, device_id: DeviceId) -> Result<(), &'static str> {
        let device = self.devices.get(&device_id)
            .ok_or("Device not found")?;

        let driver = self.driver_registry.get_driver_for_device(device_id)
            .ok_or("No driver bound to device")?;

        // Update statistics
        self.stats.total_interrupts_handled.fetch_add(1, Ordering::Relaxed);

        // Handle interrupt through driver
        let mut device_guard = device.lock();
        let _result = driver.handle_interrupt(&mut device_guard)?;

        log::debug!("Handled interrupt for VirtIO device {:?}", device_id);

        Ok(())
    }

    /// Get VirtIO device
    pub fn get_device(&self, device_id: DeviceId) -> Option<Arc<SpinLock<VirtIODevice>>> {
        self.devices.get(&device_id).cloned()
    }

    /// Get devices for VM
    pub fn get_vm_devices(&self, vm_id: VmId) -> Vec<DeviceId> {
        self.vm_devices.get(&vm_id).cloned().unwrap_or_default()
    }

    /// Get all device IDs
    pub fn get_all_device_ids(&self) -> Vec<DeviceId> {
        self.devices.keys().copied().collect()
    }

    /// Get device statistics
    pub fn get_device_stats(&self, device_id: DeviceId) -> Option<&VirtIODeviceStats> {
        self.devices.get(&device_id).map(|device| {
            let device_guard = device.lock();
            device_guard.get_stats()
        })
    }

    /// Get manager statistics
    pub fn get_stats(&self) -> &VirtIOManagerStats {
        &self.stats
    }

    /// Reset device statistics
    pub fn reset_stats(&mut self) {
        self.stats = VirtIOManagerStats::default();
    }

    /// List all VirtIO devices
    pub fn list_devices(&self) -> Vec<VirtIODeviceInfo> {
        self.devices.iter().map(|(&id, device)| {
            let device_guard = device.lock();
            VirtIODeviceInfo {
                device_id: id,
                name: device_guard.config.device_type.as_str().to_string(),
                device_type: device_guard.config.device_type,
                status: device_guard.status,
                vm_id: device_guard.vm_id,
                mmio_base: device_guard.config.mmio_base,
                mmio_size: device_guard.config.mmio_size,
                interrupt: device_guard.config.interrupt,
                features: device_guard.driver_features,
            }
        }).collect()
    }

    /// Get driver registry reference
    pub fn get_driver_registry(&self) -> &VirtIODriverRegistry {
        &self.driver_registry
    }

    /// Get mutable driver registry reference
    pub fn get_driver_registry_mut(&mut self) -> &mut VirtIODriverRegistry {
        &mut self.driver_registry
    }

    /// Perform hotplug addition
    pub fn hotplug_add_device(&mut self, request: VirtIODeviceRequest) -> Result<DeviceId, &'static str> {
        log::info!("Hotplug adding VirtIO {:?}", request.device_type);
        self.create_device(request)
    }

    /// Perform hotplug removal
    pub fn hotplug_remove_device(&mut self, device_id: DeviceId) -> Result<(), &'static str> {
        log::info!("Hotplug removing VirtIO device {:?}", device_id);
        self.destroy_device(device_id)
    }

    /// Export device state
    pub fn export_device_state(&self, device_id: DeviceId) -> Result<Vec<u8>, &'static str> {
        let device = self.devices.get(&device_id)
            .ok_or("Device not found")?;

        let device_guard = device.lock();

        // Export device state (simplified)
        let state = VirtIODeviceState {
            device_id: device_guard.config.device_id,
            device_type: device_guard.config.device_type,
            status: device_guard.status,
            driver_features: device_guard.driver_features,
            // Add other state as needed
        };

        // Serialize state (simplified)
        let data = format!("device_id:{},type:{:?},status:{:?},features:0x{:x}",
                          state.device_id, state.device_type, state.status, state.driver_features);
        Ok(data.as_bytes().to_vec())
    }

    /// Import device state
    pub fn import_device_state(&mut self, device_id: DeviceId, data: &[u8]) -> Result<(), &'static str> {
        let device = self.devices.get_mut(&device_id)
            .ok_or("Device not found")?;

        let device_guard = device.lock();

        // Parse and restore state (simplified)
        let data_str = core::str::from_utf8(data).map_err(|_| "Invalid state data")?;
        log::info!("Importing state for device {:?}: {}", device_id, data_str);

        // Restore state fields (implementation would be more detailed)
        // device_guard.restore_from_state(parsed_state);

        Ok(())
    }
}

/// VirtIO device information
#[derive(Debug, Clone)]
pub struct VirtIODeviceInfo {
    /// Device ID
    pub device_id: DeviceId,
    /// Device name
    pub name: String,
    /// Device type
    pub device_type: VirtIODeviceType,
    /// Device status
    pub status: DeviceStatus,
    /// VM ID (if assigned)
    pub vm_id: Option<VmId>,
    /// MMIO base address
    pub mmio_base: PhysAddr,
    /// MMIO size
    pub mmio_size: u64,
    /// Interrupt line
    pub interrupt: u32,
    /// Negotiated features
    pub features: u64,
}

/// VirtIO device state for export/import
#[derive(Debug, Clone)]
struct VirtIODeviceState {
    device_id: u32,
    device_type: VirtIODeviceType,
    status: DeviceStatus,
    driver_features: u64,
}

/// Extension trait for device discovery manager
trait DeviceDiscoveryManagerExt {
    /// Add device to discovery manager
    fn add_device(&mut self, device_desc: VirtualDeviceDesc) -> Result<DeviceId, &'static str>;

    /// Remove device from discovery manager
    fn remove_device(&mut self, device_id: DeviceId) -> Result<(), &'static str>;
}

impl Default for VirtIOManager {
    fn default() -> Self {
        Self::new(VirtIOManagerConfig::default())
    }
}

impl core::fmt::Display for VirtIODeviceInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "VirtIODevice {{")?;
        writeln!(f, "  device_id: {:?}", self.device_id)?;
        writeln!(f, "  name: {}", self.name)?;
        writeln!(f, "  device_type: {:?}", self.device_type)?;
        writeln!(f, "  status: {:?}", self.status)?;
        if let Some(vm_id) = self.vm_id {
            writeln!(f, "  vm_id: {:?}", vm_id)?;
        }
        writeln!(f, "  mmio_base: {:#x}", self.mmio_base.as_u64())?;
        writeln!(f, "  mmio_size: {}", self.mmio_size)?;
        writeln!(f, "  interrupt: {}", self.interrupt)?;
        writeln!(f, "  features: 0x{:x}", self.features)?;
        write!(f, "}}")
    }
}

/// Global VirtIO manager instance
static mut VIRTIO_MANAGER: Option<VirtIOManager> = None;

/// Initialize global VirtIO manager
pub fn init_virtio_manager(config: VirtIOManagerConfig) -> Result<(), &'static str> {
    let manager = VirtIOManager::new(config);

    unsafe {
        VIRTIO_MANAGER = Some(manager);
    }

    log::info!("Global VirtIO manager initialized");

    Ok(())
}

/// Get global VirtIO manager
pub fn get_virtio_manager() -> Option<&'static VirtIOManager> {
    unsafe { VIRTIO_MANAGER.as_ref() }
}

/// Get mutable global VirtIO manager
pub fn get_virtio_manager_mut() -> Option<&'static mut VirtIOManager> {
    unsafe { VIRTIO_MANAGER.as_mut() }
}