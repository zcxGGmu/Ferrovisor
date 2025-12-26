//! Platform-specific device drivers
//!
//! This module contains drivers for platform-specific devices
//! such as ARM PL011 UART, generic timers, and system registers.

use crate::{Result, Error};
use crate::drivers::{DeviceType, DeviceOps, DeviceInfo, DeviceStatus};
use crate::core::mm::VirtAddr;
use crate::core::sync::SpinLock;
use crate::arch::common::MmioAccess;
use alloc::format;

pub mod uart;
pub mod timer;
pub mod sysreg;
pub mod gpio;

/// Initialize platform-specific drivers
pub fn init() -> Result<()> {
    crate::info!("Initializing platform-specific drivers");

    // Initialize UART driver
    uart::init()?;

    // Initialize timer driver
    timer::init()?;

    // Initialize system register driver
    sysreg::init()?;

    // Initialize GPIO driver
    gpio::init()?;

    crate::info!("Platform-specific drivers initialized");
    Ok(())
}

/// Platform device base class
pub struct PlatformDevice {
    /// Device type
    device_type: DeviceType,
    /// Device name
    name: &'static str,
    /// Base address
    base_addr: VirtAddr,
    /// Size of memory-mapped region
    size: usize,
    /// Device status
    status: SpinLock<DeviceStatus>,
    /// IRQ number
    irq: Option<u32>,
}

impl PlatformDevice {
    /// Create a new platform device
    pub const fn new(
        device_type: DeviceType,
        name: &'static str,
        base_addr: VirtAddr,
        size: usize,
        irq: Option<u32>,
    ) -> Self {
        Self {
            device_type,
            name,
            base_addr,
            size,
            status: SpinLock::new(DeviceStatus::Present),
            irq,
        }
    }

    /// Read a 32-bit register
    pub fn read_reg_u32(&self, offset: usize) -> u32 {
        let mmio = MmioAccess;
        mmio.read_u32(self.base_addr + offset as u64)
    }

    /// Write a 32-bit register
    pub fn write_reg_u32(&self, offset: usize, value: u32) {
        let mmio = MmioAccess;
        mmio.write_u32(self.base_addr + offset as u64, value);
    }

    /// Read a 64-bit register
    pub fn read_reg_u64(&self, offset: usize) -> u64 {
        let mmio = MmioAccess;
        mmio.read_u64(self.base_addr + offset as u64)
    }

    /// Write a 64-bit register
    pub fn write_reg_u64(&self, offset: usize, value: u64) {
        let mmio = MmioAccess;
        mmio.write_u64(self.base_addr + offset as u64, value);
    }

    /// Get base address
    pub fn base_addr(&self) -> VirtAddr {
        self.base_addr
    }

    /// Get size
    pub fn size(&self) -> usize {
        self.size
    }

    /// Get IRQ number
    pub fn irq(&self) -> Option<u32> {
        self.irq
    }
}

impl DeviceOps for PlatformDevice {
    fn init(&mut self) -> Result<()> {
        *self.status.lock() = DeviceStatus::Initializing;

        // Basic initialization - platform-specific devices
        // will override this method for custom initialization
        crate::debug!("Initializing platform device: {}", self.name);

        *self.status.lock() = DeviceStatus::Ready;
        Ok(())
    }

    fn probe(&mut self) -> Result<bool> {
        // Try to read from the device to see if it's present
        // This is a simple probe - platform devices might need
        // more sophisticated probing logic
        let _value = self.read_reg_u32(0);
        Ok(true)
    }

    fn remove(&mut self) -> Result<()> {
        *self.status.lock() = DeviceStatus::Removing;
        *self.status.lock() = DeviceStatus::NotPresent;
        Ok(())
    }

    fn suspend(&mut self) -> Result<()> {
        *self.status.lock() = DeviceStatus::Suspended;
        Ok(())
    }

    fn resume(&mut self) -> Result<()> {
        *self.status.lock() = DeviceStatus::Ready;
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
            data: Some(format!("Base: 0x{:x}, Size: 0x{:x}", self.base_addr.value(), self.size)),
        }
    }

    fn handle_interrupt(&mut self, irq: u32) -> Result<()> {
        crate::debug!("Platform device '{}' handled interrupt {}", self.name, irq);
        Ok(())
    }

    fn ioctl(&mut self, cmd: u32, arg: u64) -> Result<u64> {
        match cmd {
            0x2000 => Ok(self.base_addr.value()),
            0x2001 => Ok(self.size as u64),
            _ => Err(Error::NotSupported),
        }
    }
}

/// Platform bus for managing platform devices
pub struct PlatformBus {
    /// List of platform devices
    devices: SpinLock<Vec<Box<dyn DeviceOps>>>,
}

impl PlatformBus {
    /// Create a new platform bus
    pub const fn new() -> Self {
        Self {
            devices: SpinLock::new(Vec::new()),
        }
    }

    /// Add a device to the platform bus
    pub fn add_device(&self, device: Box<dyn DeviceOps>) -> Result<()> {
        crate::info!("Adding platform device: {}", device.name());

        {
            let mut devices = self.devices.lock();
            devices.push(device);
        }

        Ok(())
    }

    /// Initialize all devices on the bus
    pub fn init_devices(&self) -> Result<()> {
        let devices = self.devices.lock();

        for device in devices.iter() {
            if device.probe()? {
                device.init()?;
                crate::info!("Initialized platform device: {}", device.name());
            }
        }

        Ok(())
    }

    /// Handle interrupt for platform devices
    pub fn handle_interrupt(&self, irq: u32) -> Result<()> {
        let devices = self.devices.lock();

        for device in devices.iter() {
            if let Some(device_irq) = device.get_info().data.as_ref() {
                // Parse IRQ from device data (simplified)
                if device_irq.contains(&format!("irq: {}", irq)) {
                    // This is a hack - in real implementation we'd have
                    // proper IRQ mapping
                    device.handle_interrupt(irq)?;
                }
            }
        }

        Ok(())
    }
}

/// Global platform bus instance
static PLATFORM_BUS: SpinLock<Option<PlatformBus>> = SpinLock::new(None);

/// Get the global platform bus
pub fn get_bus() -> &'static SpinLock<Option<PlatformBus>> {
    &PLATFORM_BUS
}

/// Initialize the platform bus
pub fn init_platform_bus() -> Result<()> {
    crate::info!("Initializing platform bus");

    let bus = PlatformBus::new();
    {
        let mut global = PLATFORM_BUS.lock();
        *global = Some(bus);
    }

    crate::info!("Platform bus initialized");
    Ok(())
}

/// Register a platform device
pub fn register_device(device: Box<dyn DeviceOps>) -> Result<()> {
    let bus = PLATFORM_BUS.lock();
    if let Some(ref b) = *bus {
        b.add_device(device)
    } else {
        Err(Error::NotInitialized)
    }
}