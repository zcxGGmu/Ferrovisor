//! VirtIO device drivers
//!
//! This module implements drivers for VirtIO devices, providing
//! standardized virtual I/O interface for virtual machines.

use crate::{Result, Error};
use crate::drivers::{DeviceType, DeviceOps, DeviceInfo, DeviceStatus};
use crate::core::mm::{PhysAddr, VirtAddr, PageFrameAllocator};
use crate::core::sync::SpinLock;
use crate::arch::common::MmioAccess;
use core::sync::atomic::{AtomicU16, AtomicU32, Ordering};

pub mod net;
pub mod block;
pub mod console;
pub mod rng;
pub mod gpu;
pub mod input;

/// VirtIO common configuration registers
#[repr(C)]
pub struct VirtioCommonConfig {
    /// Device feature selection
    pub device_feature_select: u32,
    /// Device feature
    pub device_feature: u32,
    /// Driver feature selection
    pub driver_feature_select: u32,
    /// Driver feature
    pub driver_feature: u32,
    /// Queue address
    pub queue_sel: u32,
    /// Queue ready
    pub queue_ready: u32,
    /// Queue notify
    pub queue_notify: u32,
    /// Interrupt status
    pub interrupt_status: u32,
    /// Interrupt acknowledge
    pub interrupt_ack: u32,
    /// Device status
    pub status: u32,
    /// Configuration generation
    pub config_generation: u8,
    /// Queue size
    pub queue_size: u16,
    /// Queue MSI vector
    pub queue_msi_vector: u16,
    /// Queue address
    pub queue_desc: u64,
    /// Queue available ring
    pub queue_avail: u64,
    /// Queue used ring
    pub queue_used: u64,
}

/// VirtIO device status flags
#[derive(Debug, Clone, Copy)]
pub struct VirtioDeviceStatus(u32);

impl VirtioDeviceStatus {
    /// Acknowledge device
    pub const ACKNOWLEDGE: u32 = 0x01;
    /// Know driver
    pub const DRIVER: u32 = 0x02;
    /// Feature negotiation complete
    pub const FEATURES_OK: u32 = 0x08;
    /// Driver OK
    pub const DRIVER_OK: u32 = 0x04;
    /// Device needs reset
    pub const NEEDS_RESET: u32 = 0x40;
    /// Failed
    pub const FAILED: u32 = 0x80;

    pub fn new() -> Self {
        Self(0)
    }

    pub fn set(&mut self, flag: u32) {
        self.0 |= flag;
    }

    pub fn clear(&mut self, flag: u32) {
        self.0 &= !flag;
    }

    pub fn has(&self, flag: u32) -> bool {
        (self.0 & flag) != 0
    }

    pub fn value(&self) -> u32 {
        self.0
    }
}

/// VirtIO feature flags
pub mod features {
    /// VIRTIO_F_RING_INDIRECT_DESC (29)
    pub const RING_INDIRECT_DESC: u32 = 1 << 29;
    /// VIRTIO_F_RING_EVENT_IDX (28)
    pub const RING_EVENT_IDX: u32 = 1 << 28;
    /// VIRTIO_F_VERSION_1 (32)
    pub const VERSION_1: u32 = 1 << 32;
    /// VIRTIO_F_ACCESS_PLATFORM (33)
    pub const ACCESS_PLATFORM: u32 = 1 << 33;
    /// VIRTIO_F_RING_PACKED (34)
    pub const RING_PACKED: u32 = 1 << 34;
    /// VIRTIO_F_IN_ORDER (35)
    pub const IN_ORDER: u32 = 1 << 35;
    /// VIRTIO_F_ORDER_PLATFORM (36)
    pub const ORDER_PLATFORM: u32 = 1 << 36;
    /// VIRTIO_F_SR_IOV (37)
    pub const SR_IOV: u32 = 1 << 37;
    /// VIRTIO_F_NOTIFICATION_DATA (38)
    pub const NOTIFICATION_DATA: u32 = 1 << 38;
}

/// VirtIO queue descriptor
#[derive(Debug)]
#[repr(C)]
pub struct VirtQueueDesc {
    /// Buffer address
    pub addr: u64,
    /// Buffer length
    pub len: u32,
    /// Flags
    pub flags: u16,
    /// Next descriptor
    pub next: u16,
}

/// VirtIO queue available ring
#[derive(Debug)]
#[repr(C)]
pub struct VirtQueueAvail {
    /// Flags
    pub flags: u16,
    /// Index of next entry to use
    pub idx: u16,
    /// Ring of available descriptors
    pub ring: [u16; 0],
}

/// VirtIO queue used element
#[derive(Debug)]
#[repr(C)]
pub struct VirtQueueUsedElem {
    /// Index of used descriptor
    pub id: u32,
    /// Length of buffer used
    pub len: u32,
}

/// VirtIO queue used ring
#[derive(Debug)]
#[repr(C)]
pub struct VirtQueueUsed {
    /// Flags
    pub flags: u16,
    /// Index of next entry to use
    pub idx: u16,
    /// Ring of used descriptors
    pub ring: [VirtQueueUsedElem; 0],
}

/// VirtIO queue
#[derive(Debug)]
pub struct VirtQueue {
    /// Queue size
    size: u16,
    /// Descriptor table
    desc: VirtAddr,
    /// Available ring
    avail: VirtAddr,
    /// Used ring
    used: VirtAddr,
    /// Last used index
    last_used_idx: AtomicU16,
    /// Available index
    avail_idx: AtomicU16,
    /// Queue index
    queue_index: u16,
}

impl VirtQueue {
    /// Create a new virtqueue
    pub fn new(queue_index: u16, size: u16) -> Result<Self> {
        if size == 0 || (size & (size - 1)) != 0 {
            return Err(Error::InvalidArgument); // Size must be power of 2
        }

        let desc_size = core::mem::size_of::<VirtQueueDesc>() * size as usize;
        let avail_size = core::mem::size_of::<VirtQueueAvail>() + (size as usize + 3) * core::mem::size_of::<u16>();
        let used_size = core::mem::size_of::<VirtQueueUsed>() + (size as usize + 3) * core::mem::size_of::<VirtQueueUsedElem>();

        // Allocate memory for descriptor table and rings
        let desc = PageFrameAllocator::alloc(desc_size)?;
        let avail = PageFrameAllocator::alloc(avail_size)?;
        let used = PageFrameAllocator::alloc(used_size)?;

        // Initialize allocated memory
        unsafe {
            core::ptr::write_bytes(desc.as_mut_ptr(), 0, desc_size);
            core::ptr::write_bytes(avail.as_mut_ptr(), 0, avail_size);
            core::ptr::write_bytes(used.as_mut_ptr(), 0, used_size);
        }

        Ok(Self {
            size,
            desc,
            avail,
            used,
            last_used_idx: AtomicU16::new(0),
            avail_idx: AtomicU16::new(0),
            queue_index,
        })
    }

    /// Get queue size
    pub fn size(&self) -> u16 {
        self.size
    }

    /// Get descriptor table address
    pub fn desc_addr(&self) -> VirtAddr {
        self.desc
    }

    /// Get available ring address
    pub fn avail_addr(&self) -> VirtAddr {
        self.avail
    }

    /// Get used ring address
    pub fn used_addr(&self) -> VirtAddr {
        self.used
    }

    /// Add a buffer to the available ring
    pub fn add_buf(&self, desc_index: u16, len: u32, write_only: bool, has_next: bool) -> Result<()> {
        if desc_index >= self.size {
            return Err(Error::InvalidArgument);
        }

        // Update descriptor
        let desc = unsafe {
            &mut *(self.desc.as_mut_ptr() as *mut VirtQueueDesc)
        };

        let desc_entry = &mut desc[desc_index as usize];
        desc_entry.len = len;
        desc_entry.flags = if write_only { 2 } else { 0 }; // VIRTQ_DESC_F_WRITE = 2
        if has_next {
            desc_entry.flags |= 1; // VIRTQ_DESC_F_NEXT = 1
        }

        // Add to available ring
        let avail = unsafe {
            &mut *(self.avail.as_mut_ptr() as *mut VirtQueueAvail)
        };

        let idx = self.avail_idx.fetch_add(1, Ordering::Release) as usize;
        let ring = unsafe {
            core::slice::from_raw_parts_mut(
                avail.ring.as_mut_ptr() as *mut u16,
                self.size as usize,
            )
        };

        if (idx % (self.size as usize)) < ring.len() {
            ring[idx % self.size as usize] = desc_index;
        }

        // Update available index in ring
        if idx % self.size as usize == self.size as usize - 1 {
            avail.idx = self.avail_idx.load(Ordering::Release);
        }

        Ok(())
    }

    /// Get used buffers from the used ring
    pub fn get_used_buf(&self) -> Option<(u32, u32)> {
        let used = unsafe {
            &*(self.used.as_mut_ptr() as *const VirtQueueUsed)
        };

        let last_used = self.last_used_idx.load(Ordering::Acquire);
        if last_used < used.idx {
            let ring = unsafe {
                core::slice::from_raw_parts(
                    used.ring.as_ptr() as *const VirtQueueUsedElem,
                    self.size as usize,
                )
            };

            let idx = last_used as usize % self.size as usize;
            if idx < ring.len() {
                let elem = &ring[idx];
                self.last_used_idx.store(last_used + 1, Ordering::Release);
                return Some((elem.id, elem.len));
            }
        }

        None
    }
}

/// VirtIO device base
pub struct VirtioDevice {
    /// Device type
    device_type: DeviceType,
    /// Device name
    name: &'static str,
    /// Base address
    base_addr: VirtAddr,
    /// Device ID
    device_id: u32,
    /// Virt queues
    queues: SpinLock<Vec<Option<VirtQueue>>>,
    /// Device status
    status: SpinLock<VirtioDeviceStatus>,
    /// Features offered by device
    device_features: SpinLock<u64>,
    /// Features selected by driver
    driver_features: SpinLock<u64>,
    /// IRQ number
    irq: u32,
    /// Common configuration
    common_config: VirtAddr,
}

impl VirtioDevice {
    /// Create a new VirtIO device
    pub fn new(
        device_type: DeviceType,
        name: &'static str,
        base_addr: VirtAddr,
        device_id: u32,
        irq: u32,
        common_config: VirtAddr,
    ) -> Self {
        Self {
            device_type,
            name,
            base_addr,
            device_id,
            queues: SpinLock::new(Vec::new()),
            status: SpinLock::new(VirtioDeviceStatus::new()),
            device_features: SpinLock::new(0),
            driver_features: SpinLock::new(0),
            irq,
            common_config,
        }
    }

    /// Reset the device
    pub fn reset(&self) -> Result<()> {
        {
            let mut status = self.status.lock();
            *status = VirtioDeviceStatus::new();
        }

        // Write reset value to device status register
        self.write_config_u32(0, 0);

        Ok(())
    }

    /// Acknowledge the device
    pub fn acknowledge(&self) -> Result<()> {
        {
            let mut status = self.status.lock();
            status.set(VirtioDeviceStatus::ACKNOWLEDGE);
        }

        self.write_config_u32(0, self.status.lock().value());
        Ok(())
    }

    /// Set driver flag
    pub fn set_driver(&self) -> Result<()> {
        {
            let mut status = self.status.lock();
            status.set(VirtioDeviceStatus::DRIVER);
        }

        self.write_config_u32(0, self.status.lock().value());
        Ok(())
    }

    /// Read device features
    pub fn read_device_features(&self) -> Result<u64> {
        // Select feature bits 0-31
        self.write_config_u32(0, 0);
        let features_lo = self.read_config_u32(1);

        // Select feature bits 32-63
        self.write_config_u32(0, 1);
        let features_hi = self.read_config_u32(1);

        let features = ((features_hi as u64) << 32) | (features_lo as u64);
        {
            let mut device_features = self.device_features.lock();
            *device_features = features;
        }

        Ok(features)
    }

    /// Write driver features
    pub fn write_driver_features(&self, features: u64) -> Result<()> {
        {
            let mut driver_features = self.driver_features.lock();
            *driver_features = features;
        }

        // Write feature bits 0-31
        self.write_config_u32(2, 0);
        self.write_config_u32(3, features as u32);

        // Write feature bits 32-63
        self.write_config_u32(2, 1);
        self.write_config_u32(3, (features >> 32) as u32);

        {
            let mut status = self.status.lock();
            status.set(VirtioDeviceStatus::FEATURES_OK);
        }

        self.write_config_u32(0, self.status.lock().value());

        Ok(())
    }

    /// Set DRIVER_OK
    pub fn set_driver_ok(&self) -> Result<()> {
        {
            let mut status = self.status.lock();
            status.set(VirtioDeviceStatus::DRIVER_OK);
        }

        self.write_config_u32(0, self.status.lock().value());
        Ok(())
    }

    /// Set up a virtqueue
    pub fn setup_queue(&self, queue_index: u16, size: u16) -> Result<()> {
        let queue = VirtQueue::new(queue_index, size)?;

        // Select queue
        self.write_config_u32(4, queue_index as u32);
        // Set queue size
        self.write_config_u32(7, size as u32);
        // Set queue addresses
        self.write_config_u32(8, queue.desc_addr().value() as u64 as u32);
        self.write_config_u32(9, (queue.desc_addr().value() >> 32) as u32);
        self.write_config_u32(10, queue.avail_addr().value() as u64 as u32);
        self.write_config_u32(11, (queue.avail_addr().value() >> 32) as u32);
        self.write_config_u32(12, queue.used_addr().value() as u64 as u32);
        self.write_config_u32(13, (queue.used_addr().value() >> 32) as u32);
        // Set queue ready
        self.write_config_u32(5, 1);

        {
            let mut queues = self.queues.lock();
            if queue_index as usize >= queues.len() {
                queues.resize(queue_index as usize + 1, None);
            }
            queues[queue_index as usize] = Some(queue);
        }

        crate::info!("Setup VirtIO queue {} with size {}", queue_index, size);
        Ok(())
    }

    /// Notify queue
    pub fn notify_queue(&self, queue_index: u16) -> Result<()> {
        // Write to queue notify register
        self.write_config_u32(6, queue_index as u32);
        Ok(())
    }

    /// Read configuration register
    fn read_config_u32(&self, offset: usize) -> u32 {
        let mmio = MmioAccess;
        mmio.read_u32(self.common_config + offset as u64 * 4)
    }

    /// Write configuration register
    fn write_config_u32(&self, offset: usize, value: u32) {
        let mmio = MmioAccess;
        mmio.write_u32(self.common_config + offset as u64 * 4, value);
    }

    /// Get queue
    pub fn get_queue(&self, index: u16) -> Option<&VirtQueue> {
        let queues = self.queues.lock();
        if (index as usize) < queues.len() {
            queues[index as usize].as_ref()
        } else {
            None
        }
    }
}

impl DeviceOps for VirtioDevice {
    fn init(&mut self) -> Result<()> {
        crate::info!("Initializing VirtIO device: {}", self.name);

        // Reset device
        self.reset()?;

        // Acknowledge device
        self.acknowledge()?;

        // Set driver flag
        self.set_driver()?;

        // Read device features
        let device_features = self.read_device_features()?;
        crate::debug!("Device features: 0x{:x}", device_features);

        // Negotiate features (for now, accept VIRTIO_F_VERSION_1)
        let mut driver_features = features::VERSION_1;
        if (device_features & features::VERSION_1) != 0 {
            driver_features |= features::VERSION_1;
        }

        // Write driver features
        self.write_driver_features(driver_features)?;

        // Set DRIVER_OK
        self.set_driver_ok()?;

        crate::info!("Initialized VirtIO device: {}", self.name);
        Ok(())
    }

    fn probe(&mut self) -> Result<bool> {
        // Read magic value and version
        let magic = self.read_config_u32(0);
        let version = self.read_config_u32(4);

        // VirtIO magic value: 0x74726976 ("virt" in little endian)
        if magic != 0x74726976 {
            return Ok(false);
        }

        // Version should be 1 or 2
        if version != 1 && version != 2 {
            return Ok(false);
        }

        Ok(true)
    }

    fn remove(&mut self) -> Result<()> {
        // Reset device
        self.reset()?;
        Ok(())
    }

    fn suspend(&mut self) -> Result<()> {
        Ok(())
    }

    fn resume(&mut self) -> Result<()> {
        Ok(())
    }

    fn device_type(&self) -> DeviceType {
        self.device_type
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn status(&self) -> DeviceStatus {
        // Convert VirtIO status to device status
        let virtio_status = self.status.lock();
        if virtio_status.has(VirtioDeviceStatus::DRIVER_OK) {
            DeviceStatus::Ready
        } else if virtio_status.has(VirtioDeviceStatus::ACKNOWLEDGE) {
            DeviceStatus::Initializing
        } else {
            DeviceStatus::Present
        }
    }

    fn get_info(&self) -> DeviceInfo {
        DeviceInfo {
            device_type: self.device_type,
            name: self.name.to_string(),
            vendor_id: Some(0x1AF4), // VirtIO vendor ID
            device_id: Some(self.device_id as u16),
            class_id: None,
            revision: Some(1),
            data: Some(format!("VirtIO device, IRQ: {}", self.irq)),
        }
    }

    fn handle_interrupt(&mut self, irq: u32) -> Result<()> {
        // Read interrupt status
        let status = self.read_config_u32(2);

        if status != 0 {
            crate::debug!("VirtIO device '{}' interrupt status: 0x{:x}", self.name, status);

            // Acknowledge interrupt
            self.write_config_u32(3, status);
        }

        Ok(())
    }

    fn ioctl(&mut self, cmd: u32, arg: u64) -> Result<u64> {
        match cmd {
            0x3000 => Ok(self.read_device_features()?),
            0x3001 => Ok(*self.driver_features.lock()),
            _ => Err(Error::NotSupported),
        }
    }
}

/// Initialize VirtIO drivers
pub fn init() -> Result<()> {
    crate::info!("Initializing VirtIO drivers");

    // Initialize VirtIO network driver
    net::init()?;

    // Initialize VirtIO block driver
    block::init()?;

    // Initialize VirtIO console driver
    console::init()?;

    // Initialize VirtIO RNG driver
    rng::init()?;

    // Initialize VirtIO GPU driver
    gpu::init()?;

    // Initialize VirtIO input driver
    input::init()?;

    crate::info!("VirtIO drivers initialized");
    Ok(())
}

/// Scan for VirtIO devices
pub fn scan_devices() -> Result<()> {
    crate::info!("Scanning for VirtIO devices");

    // This would scan PCIe or MMIO space for VirtIO devices
    // For now, we'll create some example devices

    // Create VirtIO network device
    let net_device = Box::new(VirtioDevice::new(
        DeviceType::Network,
        "virtio-net",
        VirtAddr::new(0xa0000000),
        1, // Network device ID
        32, // IRQ
        VirtAddr::new(0xa0001000), // Common config
    ));

    if let Ok(_device_id) = crate::drivers::register_device(net_device) {
        crate::info!("Found VirtIO network device");
    }

    // Create VirtIO block device
    let block_device = Box::new(VirtioDevice::new(
        DeviceType::Block,
        "virtio-blk",
        VirtAddr::new(0xa0010000),
        2, // Block device ID
        33, // IRQ
        VirtAddr::new(0xa0011000), // Common config
    ));

    if let Ok(_device_id) = crate::drivers::register_device(block_device) {
        crate::info!("Found VirtIO block device");
    }

    crate::info!("VirtIO device scan complete");
    Ok(())
}