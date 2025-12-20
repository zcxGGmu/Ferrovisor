//! Base device driver framework
//!
//! This module provides the foundational framework for device drivers,
//! including common interfaces, utilities, and base classes.

use crate::{Result, Error};
use crate::core::sync::SpinLock;
use crate::drivers::{DeviceType, DeviceInfo, DeviceStatus, DeviceResource, ResourceType};
use crate::drivers::DeviceOps;
use crate::core::mm::{PhysAddr, VirtAddr};
use core::sync::atomic::{AtomicU64, AtomicU32, Ordering};

pub mod console;
pub mod serial;
pub mod timer;
pub mod interrupt;
pub mod pci;

/// Base device implementation
pub struct BaseDevice {
    /// Device ID
    device_id: u32,
    /// Device type
    device_type: DeviceType,
    /// Device name
    name: &'static str,
    /// Device status
    status: SpinLock<DeviceStatus>,
    /// Device resources
    resources: SpinLock<Vec<DeviceResource>>,
    /// IRQ number
    irq: SpinLock<Option<u32>>,
    /// Statistics
    stats: SpinLock<BaseDeviceStats>,
}

/// Base device statistics
#[derive(Debug, Default, Clone, Copy)]
pub struct BaseDeviceStats {
    /// Number of interrupts handled
    pub interrupts_handled: u64,
    /// Number of I/O operations
    pub io_operations: u64,
    /// Number of errors
    pub errors: u64,
    /// Last activity timestamp
    pub last_activity: u64,
}

impl BaseDevice {
    /// Create a new base device
    pub fn new(
        device_id: u32,
        device_type: DeviceType,
        name: &'static str,
        resources: Vec<DeviceResource>,
    ) -> Self {
        Self {
            device_id,
            device_type,
            name,
            status: SpinLock::new(DeviceStatus::Present),
            resources: SpinLock::new(resources),
            irq: SpinLock::new(None),
            stats: SpinLock::new(BaseDeviceStats::default()),
        }
    }

    /// Get device ID
    pub fn device_id(&self) -> u32 {
        self.device_id
    }

    /// Get device type
    pub fn device_type(&self) -> DeviceType {
        self.device_type
    }

    /// Set IRQ number
    pub fn set_irq(&self, irq: u32) {
        let mut irq_lock = self.irq.lock();
        *irq_lock = Some(irq);
    }

    /// Get IRQ number
    pub fn get_irq(&self) -> Option<u32> {
        *self.irq.lock()
    }

    /// Add a resource
    pub fn add_resource(&self, resource: DeviceResource) -> Result<()> {
        let mut resources = self.resources.lock();
        resources.push(resource);
        Ok(())
    }

    /// Find a resource by type and name
    pub fn find_resource(&self, resource_type: ResourceType, name: &str) -> Option<DeviceResource> {
        let resources = self.resources.lock();
        resources.iter().find(|r| {
            r.resource_type == resource_type && r.name == name
        }).cloned()
    }

    /// Find MMIO resource by name
    pub fn find_mmio_resource(&self, name: &str) -> Option<(VirtAddr, usize)> {
        let resource = self.find_resource(ResourceType::Mmio, name)?;
        Some((
            VirtAddr::new(resource.start),
            resource.size() as usize
        ))
    }

    /// Find IRQ resource by name
    pub fn find_irq_resource(&self, name: &str) -> Option<u32> {
        let resource = self.find_resource(ResourceType::Irq, name)?;
        Some(resource.start as u32)
    }

    /// Update statistics
    pub fn update_stats<F>(&self, update_fn: F) where F: FnOnce(&mut BaseDeviceStats) {
        let mut stats = self.stats.lock();
        update_fn(&mut *stats);
        stats.last_activity = crate::utils::get_timestamp();
    }

    /// Get statistics
    pub fn get_stats(&self) -> BaseDeviceStats {
        *self.stats.lock()
    }
}

impl DeviceOps for BaseDevice {
    fn init(&mut self) -> Result<()> {
        *self.status.lock() = DeviceStatus::Initializing;

        // Initialize resources
        let resources = self.resources.lock();
        for resource in resources.iter() {
            crate::debug!("Initializing resource: {} ({})", resource.name, resource.start);
        }

        *self.status.lock() = DeviceStatus::Ready;
        crate::info!("Initialized base device: {}", self.name);

        Ok(())
    }

    fn probe(&mut self) -> Result<bool> {
        // Check if device is present by probing resources
        let resources = self.resources.lock();
        if resources.is_empty() {
            return Ok(false);
        }

        // For now, assume device is present if it has resources
        *self.status.lock() = DeviceStatus::Present;
        Ok(true)
    }

    fn remove(&mut self) -> Result<()> {
        *self.status.lock() = DeviceStatus::Removing;

        // Cleanup resources
        self.resources.lock().clear();
        *self.irq.lock() = None;

        *self.status.lock() = DeviceStatus::NotPresent;
        crate::info!("Removed base device: {}", self.name);

        Ok(())
    }

    fn suspend(&mut self) -> Result<()> {
        if *self.status.lock() != DeviceStatus::Ready {
            return Err(Error::InvalidState);
        }

        *self.status.lock() = DeviceStatus::Suspended;
        crate::info!("Suspended base device: {}", self.name);

        Ok(())
    }

    fn resume(&mut self) -> Result<()> {
        if *self.status.lock() != DeviceStatus::Suspended {
            return Err(Error::InvalidState);
        }

        *self.status.lock() = DeviceStatus::Ready;
        crate::info!("Resumed base device: {}", self.name);

        Ok(())
    }

    fn device_type(&self) -> DeviceType {
        self.device_type
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn status(&self) -> DeviceStatus {
        *self.status.lock()
    }

    fn get_info(&self) -> DeviceInfo {
        DeviceInfo {
            device_type: self.device_type,
            name: self.name.to_string(),
            vendor_id: None,
            device_id: None,
            class_id: None,
            revision: None,
            data: None,
        }
    }

    fn handle_interrupt(&mut self, irq: u32) -> Result<()> {
        self.update_stats(|stats| {
            stats.interrupts_handled += 1;
        });

        crate::debug!("Base device '{}' handled interrupt {}", self.name, irq);
        Ok(())
    }

    fn ioctl(&mut self, cmd: u32, arg: u64) -> Result<u64> {
        self.update_stats(|stats| {
            stats.io_operations += 1;
        });

        match cmd {
            // Get device ID
            0x1000 => Ok(self.device_id as u64),
            // Get device status
            0x1001 => Ok(self.status() as u64),
            // Get statistics
            0x1002 => {
                let stats = self.get_stats();
                // Pack stats into u64 (simplified)
                Ok(stats.interrupts_handled)
            }
            _ => Err(Error::NotSupported),
        }
    }
}

/// Generic device driver
pub struct GenericDriver {
    /// Driver name
    name: &'static str,
    /// Supported device types
    supported_types: &'static [DeviceType],
    /// Bound devices
    devices: SpinLock<Vec<Box<dyn DeviceOps>>>,
    /// Driver-specific data
    driver_data: SpinLock<Option<*mut u8>>,
}

impl GenericDriver {
    /// Create a new generic driver
    pub const fn new(
        name: &'static str,
        supported_types: &'static [DeviceType],
    ) -> Self {
        Self {
            name,
            supported_types,
            devices: SpinLock::new(Vec::new()),
            driver_data: SpinLock::new(None),
        }
    }

    /// Get number of bound devices
    pub fn device_count(&self) -> usize {
        self.devices.lock().len()
    }

    /// Get bound device by index
    pub fn get_device(&self, index: usize) -> Option<Box<dyn DeviceOps>> {
        // Note: This is a simplified approach
        // In real implementation, we'd need proper cloning or references
        None
    }
}

unsafe impl Send for GenericDriver {}
unsafe impl Sync for GenericDriver {}

impl crate::drivers::Driver for GenericDriver {
    fn name(&self) -> &'static str {
        self.name
    }

    fn supported_types(&self) -> &[DeviceType] {
        self.supported_types
    }

    fn probe(&self, device: &dyn DeviceOps) -> Result<bool> {
        // Check if device type is supported
        for supported_type in self.supported_types.iter() {
            if device.device_type() == *supported_type {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn bind(&mut self, device: Box<dyn DeviceOps>) -> Result<()> {
        crate::info!("Binding device '{}' to driver '{}'", device.name(), self.name);

        // Initialize the device
        let mut dev = device;
        dev.init()?;

        // Add to bound devices list
        {
            let mut devices = self.devices.lock();
            devices.push(dev);
        }

        Ok(())
    }

    fn unbind(&mut self, device: &dyn DeviceOps) -> Result<()> {
        crate::info!("Unbinding device '{}' from driver '{}'", device.name(), self.name);

        // Remove from bound devices list
        {
            let mut devices = self.devices.lock();
            devices.retain(|d| d.name() != device.name());
        }

        Ok(())
    }

    fn get_info(&self) -> crate::drivers::DriverInfo {
        crate::drivers::DriverInfo {
            name: self.name.to_string(),
            version: "1.0.0".to_string(),
            author: "Ferrovisor Team".to_string(),
            description: "Generic device driver".to_string(),
            supported_types: self.supported_types.to_vec(),
        }
    }
}

/// Device registry for managing drivers
pub struct DeviceRegistry {
    /// Registered drivers
    drivers: SpinLock<Vec<Box<dyn crate::drivers::Driver>>>,
    /// Driver name to index mapping
    driver_map: SpinLock<alloc::collections::BTreeMap<&'static str, usize>>,
}

impl DeviceRegistry {
    /// Create a new device registry
    pub const fn new() -> Self {
        Self {
            drivers: SpinLock::new(Vec::new()),
            driver_map: SpinLock::new(alloc::collections::BTreeMap::new()),
        }
    }

    /// Register a driver
    pub fn register_driver(&self, driver: Box<dyn crate::drivers::Driver>) -> Result<()> {
        let name = driver.name();

        {
            let mut drivers = self.drivers.lock();
            let index = drivers.len();
            drivers.push(driver);

            let mut driver_map = self.driver_map.lock();
            driver_map.insert(name, index);
        }

        crate::info!("Registered driver: {}", name);
        Ok(())
    }

    /// Find driver by name
    pub fn find_driver(&self, name: &str) -> Option<Box<dyn crate::drivers::Driver>> {
        let driver_map = self.driver_map.lock();
        if let Some(&index) = driver_map.get(name) {
            let drivers = self.drivers.lock();
            // Note: This is a simplified approach
            // In real implementation, we'd need proper cloning or references
            None
        } else {
            None
        }
    }

    /// Get all registered drivers
    pub fn get_drivers(&self) -> Vec<crate::drivers::DriverInfo> {
        let drivers = self.drivers.lock();
        let mut infos = Vec::new();

        for driver in drivers.iter() {
            infos.push(driver.get_info());
        }

        infos
    }
}

/// Global device registry
static DEVICE_REGISTRY: SpinLock<Option<DeviceRegistry>> = SpinLock::new(None);

/// Initialize the base driver framework
pub fn init() -> Result<()> {
    crate::info!("Initializing base driver framework");

    let registry = DeviceRegistry::new();
    {
        let mut global = DEVICE_REGISTRY.lock();
        *global = Some(registry);
    }

    // Register built-in drivers
    register_builtin_drivers()?;

    crate::info!("Base driver framework initialized");
    Ok(())
}

/// Get the global device registry
pub fn get_registry() -> &'static SpinLock<Option<DeviceRegistry>> {
    &DEVICE_REGISTRY
}

/// Register a driver
pub fn register_driver(driver: Box<dyn crate::drivers::Driver>) -> Result<()> {
    let registry = DEVICE_REGISTRY.lock();
    if let Some(ref reg) = *registry {
        reg.register_driver(driver)
    } else {
        Err(Error::NotInitialized)
    }
}

/// Register built-in drivers
fn register_builtin_drivers() -> Result<()> {
    // Register console driver
    let console_driver = Box::new(GenericDriver::new(
        "console",
        &[DeviceType::Console],
    ));
    register_driver(console_driver)?;

    // Register serial driver
    let serial_driver = Box::new(GenericDriver::new(
        "serial",
        &[DeviceType::Char],
    ));
    register_driver(serial_driver)?;

    // Register timer driver
    let timer_driver = Box::new(GenericDriver::new(
        "timer",
        &[DeviceType::Timer],
    ));
    register_driver(timer_driver)?;

    Ok(())
}

/// Initialize platform devices (called from drivers::init)
pub fn init_platform_devices() -> Result<()> {
    crate::info!("Initializing platform devices");

    // Create and register common platform devices
    let console_device = Box::new(BaseDevice::new(
        1,
        DeviceType::Console,
        "console",
        vec![
            DeviceResource::new(ResourceType::Mmio, 0x09000000, 0x090000FF, 0, "console-mmio"),
            DeviceResource::new(ResourceType::Irq, 1, 1, 0, "console-irq"),
        ],
    ));

    if let Ok(device_id) = crate::drivers::register_device(console_device) {
        crate::info!("Registered console device with ID {}", device_id);
    }

    let timer_device = Box::new(BaseDevice::new(
        2,
        DeviceType::Timer,
        "timer",
        vec![
            DeviceResource::new(ResourceType::Mmio, 0x08000000, 0x080000FF, 0, "timer-mmio"),
            DeviceResource::new(ResourceType::Irq, 0, 0, 0, "timer-irq"),
        ],
    ));

    if let Ok(device_id) = crate::drivers::register_device(timer_device) {
        crate::info!("Registered timer device with ID {}", device_id);
    }

    Ok(())
}