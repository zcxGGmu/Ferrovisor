//! Device drivers module
//!
//! This module provides a framework for implementing device drivers
//! in the hypervisor, including common interfaces and utilities.

use crate::{Result, Error};
use crate::core::sync::SpinLock;
use crate::core::mm::{PhysAddr, VirtAddr};

pub mod base;
pub mod platform;
pub mod virtio;

/// Device type identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    /// Unknown device
    Unknown = 0,
    /// Console device
    Console = 1,
    /// Network device
    Network = 2,
    /// Block device (disk, SSD)
    Block = 3,
    /// Character device (serial, etc.)
    Char = 4,
    /// Graphics device
    Graphics = 5,
    /// Input device (keyboard, mouse)
    Input = 6,
    /// Audio device
    Audio = 7,
    /// USB device
    Usb = 8,
    /// PCI device
    Pci = 9,
    /// Platform device
    Platform = 10,
    /// Virtual device
    Virtual = 11,
    /// Timer device
    Timer = 12,
    /// Interrupt controller
    InterruptController = 13,
    /// Memory controller
    MemoryController = 14,
}

impl From<u32> for DeviceType {
    fn from(value: u32) -> Self {
        match value {
            1 => DeviceType::Console,
            2 => DeviceType::Network,
            3 => DeviceType::Block,
            4 => DeviceType::Char,
            5 => DeviceType::Graphics,
            6 => DeviceType::Input,
            7 => DeviceType::Audio,
            8 => DeviceType::Usb,
            9 => DeviceType::Pci,
            10 => DeviceType::Platform,
            11 => DeviceType::Virtual,
            12 => DeviceType::Timer,
            13 => DeviceType::InterruptController,
            14 => DeviceType::MemoryController,
            _ => DeviceType::Unknown,
        }
    }
}

/// Device status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceStatus {
    /// Device is not present
    NotPresent,
    /// Device is present but not initialized
    Present,
    /// Device is initializing
    Initializing,
    /// Device is ready and operational
    Ready,
    /// Device has an error
    Error,
    /// Device is suspended
    Suspended,
    /// Device is being removed
    Removing,
}

/// Device operations trait
pub trait DeviceOps: Send + Sync {
    /// Initialize the device
    fn init(&mut self) -> Result<()>;

    /// Probe if the device exists and is supported
    fn probe(&mut self) -> Result<bool>;

    /// Remove/uninitialize the device
    fn remove(&mut self) -> Result<()>;

    /// Suspend the device
    fn suspend(&mut self) -> Result<()>;

    /// Resume the device
    fn resume(&mut self) -> Result<()>;

    /// Get device type
    fn device_type(&self) -> DeviceType;

    /// Get device name
    fn name(&self) -> &'static str;

    /// Get device status
    fn status(&self) -> DeviceStatus;

    /// Get device-specific information
    fn get_info(&self) -> DeviceInfo;

    /// Handle interrupt for this device
    fn handle_interrupt(&mut self, irq: u32) -> Result<()>;

    /// Perform I/O control operation
    fn ioctl(&mut self, cmd: u32, arg: u64) -> Result<u64>;
}

/// Device information
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    /// Device type
    pub device_type: DeviceType,
    /// Device name
    pub name: String,
    /// Vendor ID
    pub vendor_id: Option<u16>,
    /// Device ID
    pub device_id: Option<u16>,
    /// Class ID
    pub class_id: Option<u8>,
    /// Revision
    pub revision: Option<u8>,
    /// Device-specific data
    pub data: Option<String>,
}

impl Default for DeviceInfo {
    fn default() -> Self {
        Self {
            device_type: DeviceType::Unknown,
            name: "Unknown".to_string(),
            vendor_id: None,
            device_id: None,
            class_id: None,
            revision: None,
            data: None,
        }
    }
}

/// Device resource types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    /// Memory-mapped I/O region
    Mmio,
    /// I/O port region
    IoPort,
    /// IRQ resource
    Irq,
    /// DMA resource
    Dma,
    /// Clock resource
    Clock,
    /// Reset resource
    Reset,
    /// Power resource
    Power,
    /// GPIO resource
    Gpio,
}

/// Device resource
#[derive(Debug, Clone)]
pub struct DeviceResource {
    /// Resource type
    pub resource_type: ResourceType,
    /// Start address/number
    pub start: u64,
    /// End address/number
    pub end: u64,
    /// Flags
    pub flags: u64,
    /// Resource name
    pub name: String,
}

impl DeviceResource {
    /// Create a new resource
    pub fn new(
        resource_type: ResourceType,
        start: u64,
        end: u64,
        flags: u64,
        name: &str,
    ) -> Self {
        Self {
            resource_type,
            start,
            end,
            flags,
            name: name.to_string(),
        }
    }

    /// Get resource size
    pub fn size(&self) -> u64 {
        if self.start <= self.end {
            self.end - self.start + 1
        } else {
            0
        }
    }
}

/// Device driver interface
pub trait Driver: Send + Sync {
    /// Driver name
    fn name(&self) -> &'static str;

    /// Get list of supported device types
    fn supported_types(&self) -> &[DeviceType];

    /// Probe if the driver supports a device
    fn probe(&self, device: &dyn DeviceOps) -> Result<bool>;

    /// Bind the driver to a device
    fn bind(&mut self, device: Box<dyn DeviceOps>) -> Result<()>;

    /// Unbind the driver from a device
    fn unbind(&mut self, device: &dyn DeviceOps) -> Result<()>;

    /// Get driver-specific information
    fn get_info(&self) -> DriverInfo;
}

/// Driver information
#[derive(Debug, Clone)]
pub struct DriverInfo {
    /// Driver name
    pub name: String,
    /// Driver version
    pub version: String,
    /// Driver author
    pub author: String,
    /// Driver description
    pub description: String,
    /// Supported device types
    pub supported_types: Vec<DeviceType>,
}

/// Device manager
pub struct DeviceManager {
    /// List of registered devices
    devices: SpinLock<Vec<Box<dyn DeviceOps>>>,
    /// List of registered drivers
    drivers: SpinLock<Vec<Box<dyn Driver>>>,
    /// Device-to-driver mappings
    bindings: SpinLock<Vec<(usize, usize)>>, // (device_index, driver_index)
    /// Next device ID
    next_device_id: SpinLock<u32>,
}

impl DeviceManager {
    /// Create a new device manager
    pub const fn new() -> Self {
        Self {
            devices: SpinLock::new(Vec::new()),
            drivers: SpinLock::new(Vec::new()),
            bindings: SpinLock::new(Vec::new()),
            next_device_id: SpinLock::new(0),
        }
    }

    /// Register a device
    pub fn register_device(&self, device: Box<dyn DeviceOps>) -> Result<u32> {
        let device_id = {
            let mut id = self.next_device_id.lock();
            *id += 1;
            *id
        };

        {
            let mut devices = self.devices.lock();
            devices.push(device);
        }

        crate::info!("Registered device with ID {}", device_id);
        Ok(device_id)
    }

    /// Unregister a device
    pub fn unregister_device(&self, device_id: u32) -> Result<()> {
        {
            let mut devices = self.devices.lock();
            // Remove device
            devices.retain(|d| {
                // This is a simplified check - in real implementation,
                // devices would have IDs
                true
            });
        }

        crate::info!("Unregistered device with ID {}", device_id);
        Ok(())
    }

    /// Register a driver
    pub fn register_driver(&self, driver: Box<dyn Driver>) -> Result<()> {
        {
            let mut drivers = self.drivers.lock();
            drivers.push(driver);
        }

        crate::info!("Registered driver");
        Ok(())
    }

    /// Probe and bind devices to drivers
    pub fn probe_and_bind(&self) -> Result<()> {
        let devices = self.devices.lock();
        let drivers = self.drivers.lock();

        for (device_idx, device) in devices.iter().enumerate() {
            for (driver_idx, driver) in drivers.iter().enumerate() {
                if driver.probe(device.as_ref())? {
                    crate::info!("Binding device '{}' to driver '{}'",
                               device.name(), driver.name());

                    // Store binding
                    {
                        let mut bindings = self.bindings.lock();
                        bindings.push((device_idx, driver_idx));
                    }
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle interrupt for a device
    pub fn handle_interrupt(&self, device_type: DeviceType, irq: u32) -> Result<()> {
        let devices = self.devices.lock();

        for device in devices.iter() {
            if device.device_type() == device_type {
                device.handle_interrupt(irq)?;
            }
        }

        Ok(())
    }

    /// Get all devices
    pub fn get_devices(&self) -> Vec<DeviceInfo> {
        let devices = self.devices.lock();
        let mut infos = Vec::new();

        for device in devices.iter() {
            infos.push(device.get_info());
        }

        infos
    }

    /// Get all drivers
    pub fn get_drivers(&self) -> Vec<DriverInfo> {
        let drivers = self.drivers.lock();
        let mut infos = Vec::new();

        for driver in drivers.iter() {
            infos.push(driver.get_info());
        }

        infos
    }

    /// Initialize all devices
    pub fn init_devices(&self) -> Result<()> {
        let devices = self.devices.lock();

        for device in devices.iter() {
            if device.probe()? {
                device.init()?;
                crate::info!("Initialized device: {}", device.name());
            }
        }

        Ok(())
    }
}

/// Global device manager instance
static DEVICE_MANAGER: SpinLock<Option<DeviceManager>> = SpinLock::new(None);

/// Initialize the device management subsystem
pub fn init() -> Result<()> {
    crate::info!("Initializing device management");

    let manager = DeviceManager::new();

    {
        let mut global = DEVICE_MANAGER.lock();
        *global = Some(manager);
    }

    // Initialize base driver framework
    base::init()?;

    // Initialize platform-specific drivers
    platform::init()?;

    // Initialize VirtIO drivers
    virtio::init()?;

    crate::info!("Device management initialized");

    Ok(())
}

/// Get the global device manager
pub fn get_manager() -> &'static SpinLock<Option<DeviceManager>> {
    &DEVICE_MANAGER
}

/// Register a device
pub fn register_device(device: Box<dyn DeviceOps>) -> Result<u32> {
    let manager = DEVICE_MANAGER.lock();
    if let Some(ref mgr) = *manager {
        mgr.register_device(device)
    } else {
        Err(Error::NotInitialized)
    }
}

/// Register a driver
pub fn register_driver(driver: Box<dyn Driver>) -> Result<()> {
    let manager = DEVICE_MANAGER.lock();
    if let Some(ref mgr) = *manager {
        mgr.register_driver(driver)
    } else {
        Err(Error::NotInitialized)
    }
}

/// Handle device interrupt
pub fn handle_interrupt(device_type: DeviceType, irq: u32) -> Result<()> {
    let manager = DEVICE_MANAGER.lock();
    if let Some(ref mgr) = *manager {
        mgr.handle_interrupt(device_type, irq)
    } else {
        Err(Error::NotInitialized)
    }
}