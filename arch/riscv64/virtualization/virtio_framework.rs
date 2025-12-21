//! RISC-V VirtIO Framework for Virtual Devices
//!
//! This module provides a comprehensive VirtIO framework based on xvisor patterns,
//! including:
//! - VirtIO device lifecycle management
//! - VirtQueue implementation and management
//! - Feature negotiation protocol
//! - Device configuration and status management
//! - Interrupt handling and notification
//! - DMA buffer management
//! - Device hotplug support

use crate::arch::riscv64::virtualization::{VmId, VcpuId};
use crate::arch::riscv64::virtualization::vm::{VirtualDevice, VmDeviceConfig, VirtualMachine};
use crate::arch::riscv64::virtualization::discovery::{VirtualDeviceDesc, DeviceResources, MemoryRegion, IrqResource};
use crate::arch::riscv64::virtualization::discovery_manager::{DeviceDiscoveryManager, RiscvDeviceDiscoveryManager};
use crate::drivers::{DeviceId, DeviceType, DeviceStatus};
use crate::core::mm::{PhysAddr, VirtAddr};
use crate::core::sync::SpinLock;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, AtomicU64, AtomicBool, Ordering};

/// VirtIO device types based on specification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtIODeviceType {
    /// Network device (1)
    Network = 1,
    /// Block device (2)
    Block = 2,
    /// Console device (3)
    Console = 3,
    /// Entropy source device (4)
    Rng = 4,
    /// Memory ballooning device (5)
    Balloon = 5,
    /// IO memory device (6)
    IOMemory = 6,
    /// RPMSG device (7)
    Rpmsg = 7,
    /// SCSI host device (8)
    ScsiHost = 8,
    /// 9P transport device (9)
    NineP = 9,
    /// MAC80211 WLAN device (10)
    Mac80211Wlan = 10,
    /// RPROC serial device (11)
    RprocSerial = 11,
    /// CAIF device (12)
    Caif = 12,
    /// Memory balloon device (13)
    MemoryBalloon = 13,
    /// GPU device (16)
    GPU = 16,
    /// Input device (18)
    Input = 18,
    /// VSOCK device (19)
    Vsock = 19,
    /// Crypto device (20)
    Crypto = 20,
    /// Signal distribution device (21)
    SignalDistribution = 21,
    /// Pstore device (22)
    Pstore = 22,
    /// IOMMU device (23)
    IOMMU = 23,
    /// Sound device (24)
    Sound = 24,
    /// FileSystem device (25)
    FileSystem = 25,
    /// Pmem device (26)
    Pmem = 26,
    /// RPMB device (27)
    Rpmb = 27,
    /// I2C adapter device (35)
    I2CAdapter = 35,
    /// SCMI device (36)
    SCMI = 36,
    /// GPIO device (41)
    GPIO = 41,
    /// MDB device (43)
    MDB = 43,
}

impl From<u32> for VirtIODeviceType {
    fn from(value: u32) -> Self {
        match value {
            1 => VirtIODeviceType::Network,
            2 => VirtIODeviceType::Block,
            3 => VirtIODeviceType::Console,
            4 => VirtIODeviceType::Rng,
            5 => VirtIODeviceType::Balloon,
            6 => VirtIODeviceType::IOMemory,
            7 => VirtIODeviceType::Rpmsg,
            8 => VirtIODeviceType::ScsiHost,
            9 => VirtIODeviceType::NineP,
            10 => VirtIODeviceType::Mac80211Wlan,
            11 => VirtIODeviceType::RprocSerial,
            12 => VirtIODeviceType::Caif,
            13 => VirtIODeviceType::MemoryBalloon,
            16 => VirtIODeviceType::GPU,
            18 => VirtIODeviceType::Input,
            19 => VirtIODeviceType::Vsock,
            20 => VirtIODeviceType::Crypto,
            21 => VirtIODeviceType::SignalDistribution,
            22 => VirtIODeviceType::Pstore,
            23 => VirtIODeviceType::IOMMU,
            24 => VirtIODeviceType::Sound,
            25 => VirtIODeviceType::FileSystem,
            26 => VirtIODeviceType::Pmem,
            27 => VirtIODeviceType::Rpmb,
            35 => VirtIODeviceType::I2CAdapter,
            36 => VirtIODeviceType::SCMI,
            41 => VirtIODeviceType::GPIO,
            43 => VirtIODeviceType::MDB,
            _ => VirtIODeviceType::Console, // Default to console for unknown types
        }
    }
}

impl VirtIODeviceType {
    /// Convert to human-readable string
    pub fn as_str(&self) -> &'static str {
        match self {
            VirtIODeviceType::Network => "network",
            VirtIODeviceType::Block => "block",
            VirtIODeviceType::Console => "console",
            VirtIODeviceType::Rng => "rng",
            VirtIODeviceType::Balloon => "balloon",
            VirtIODeviceType::IOMemory => "io-memory",
            VirtIODeviceType::Rpmsg => "rpmsg",
            VirtIODeviceType::ScsiHost => "scsi-host",
            VirtIODeviceType::NineP => "9p",
            VirtIODeviceType::Mac80211Wlan => "mac80211-wlan",
            VirtIODeviceType::RprocSerial => "rproc-serial",
            VirtIODeviceType::Caif => "caif",
            VirtIODeviceType::MemoryBalloon => "memory-balloon",
            VirtIODeviceType::GPU => "gpu",
            VirtIODeviceType::Input => "input",
            VirtIODeviceType::Vsock => "vsock",
            VirtIODeviceType::Crypto => "crypto",
            VirtIODeviceType::SignalDistribution => "signal-distribution",
            VirtIODeviceType::Pstore => "pstore",
            VirtIODeviceType::IOMMU => "iommu",
            VirtIODeviceType::Sound => "sound",
            VirtIODeviceType::FileSystem => "fs",
            VirtIODeviceType::Pmem => "pmem",
            VirtIODeviceType::Rpmb => "rpmb",
            VirtIODeviceType::I2CAdapter => "i2c-adapter",
            VirtIODeviceType::SCMI => "scmi",
            VirtIODeviceType::GPIO => "gpio",
            VirtIODeviceType::MDB => "mdb",
        }
    }
}

/// VirtIO feature flags
pub mod features {
    pub const VIRTIO_F_RING_INDIRECT_DESC: u32 = 28;
    pub const VIRTIO_F_RING_EVENT_IDX: u32 = 29;
    pub const VIRTIO_F_VERSION_1: u32 = 32;
    pub const VIRTIO_F_ACCESS_PLATFORM: u32 = 33;
    pub const VIRTIO_F_RING_PACKED: u32 = 34;
    pub const VIRTIO_F_IN_ORDER: u32 = 35;
    pub const VIRTIO_F_ORDER_PLATFORM: u32 = 36;
    pub const VIRTIO_F_SR_IOV: u32 = 37;
    pub const VIRTIO_F_NOTIFICATION_DATA: u32 = 38;

    // Network device specific features
    pub mod net {
        pub const VIRTIO_NET_F_CSUM: u32 = 0;
        pub const VIRTIO_NET_F_GUEST_CSUM: u32 = 1;
        pub const VIRTIO_NET_F_CTRL_GUEST_OFFLOADS: u32 = 2;
        pub const VIRTIO_NET_F_MTU: u32 = 3;
        pub const VIRTIO_NET_F_MAC: u32 = 5;
        pub const VIRTIO_NET_F_GUEST_TSO4: u32 = 7;
        pub const VIRTIO_NET_F_GUEST_TSO6: u32 = 8;
        pub const VIRTIO_NET_F_GUEST_ECN: u32 = 9;
        pub const VIRTIO_NET_F_GUEST_UFO: u32 = 10;
        pub const VIRTIO_NET_F_HOST_TSO4: u32 = 11;
        pub const VIRTIO_NET_F_HOST_TSO6: u32 = 12;
        pub const VIRTIO_NET_F_HOST_ECN: u32 = 13;
        pub const VIRTIO_NET_F_HOST_UFO: u32 = 14;
        pub const VIRTIO_NET_F_MRG_RXBUF: u32 = 15;
        pub const VIRTIO_NET_F_STATUS: u32 = 16;
        pub const VIRTIO_NET_F_CTRL_VQ: u32 = 17;
        pub const VIRTIO_NET_F_CTRL_RX: u32 = 18;
        pub const VIRTIO_NET_F_CTRL_VLAN: u32 = 19;
        pub const VIRTIO_NET_F_CTRL_RX_EXTRA: u32 = 20;
        pub const VIRTIO_NET_F_GUEST_ANNOUNCE: u32 = 21;
        pub const VIRTIO_NET_F_MQ: u32 = 22;
        pub const VIRTIO_NET_F_CTRL_MAC_ADDR: u32 = 23;
        pub const VIRTIO_NET_F_VQ_NOTF_COAL: u32 = 52;
    }

    // Block device specific features
    pub mod blk {
        pub const VIRTIO_BLK_F_SIZE_MAX: u32 = 1;
        pub const VIRTIO_BLK_F_SEG_MAX: u32 = 2;
        pub const VIRTIO_BLK_F_GEOMETRY: u32 = 4;
        pub const VIRTIO_BLK_F_RO: u32 = 5;
        pub const VIRTIO_BLK_F_BLK_SIZE: u32 = 6;
        pub const VIRTIO_BLK_F_FLUSH: u32 = 9;
        pub const VIRTIO_BLK_F_TOPOLOGY: u32 = 10;
        pub const VIRTIO_BLK_F_CONFIG_WCE: u32 = 11;
        pub const VIRTIO_BLK_F_DISCARD: u32 = 13;
        pub const VIRTIO_BLK_F_WRITE_ZEROES: u32 = 14;
        pub const VIRTIO_BLK_F_LIFETIME: u32 = 15;
        pub const VIRTIO_BLK_F_ZONED: u32 = 17;
    }
}

/// VirtIO register offsets
pub mod registers {
    pub const MAGIC_VALUE: usize = 0x000;
    pub const VERSION: usize = 0x004;
    pub const DEVICE_ID: usize = 0x008;
    pub const VENDOR_ID: usize = 0x00C;
    pub const DEVICE_FEATURES: usize = 0x010;
    pub const DEVICE_FEATURES_SEL: usize = 0x014;
    pub const DRIVER_FEATURES: usize = 0x020;
    pub const DRIVER_FEATURES_SEL: usize = 0x024;
    pub const QUEUE_SEL: usize = 0x030;
    pub const QUEUE_NUM_MAX: usize = 0x034;
    pub const QUEUE_NUM: usize = 0x038;
    pub const QUEUE_READY: usize = 0x044;
    pub const NOTIFY_OFF: usize = 0x050;
    pub const DEVICE_STATUS: usize = 0x070;
    pub const CONFIG_GENERATION: usize = 0x074;
    pub const CONFIG: usize = 0x100;
    pub const QUEUE_DESC_LOW: usize = 0x080;
    pub const QUEUE_DESC_HIGH: usize = 0x084;
    pub const QUEUE_DRIVER_LOW: usize = 0x090;
    pub const QUEUE_DRIVER_HIGH: usize = 0x094;
    pub const QUEUE_DEVICE_LOW: usize = 0x0A0;
    pub const QUEUE_DEVICE_HIGH: usize = 0x0A4;
    pub const SHMSIZE_LOW: usize = 0x0B0;
    pub const SHMSIZE_HIGH: usize = 0x0B4;
    pub const SHM_BASE_LOW: usize = 0x0C0;
    pub const SHM_BASE_HIGH: usize = 0x0C4;
    pub const QUEUE_SHM_ADDR_LOW: usize = 0x0D0;
    pub const QUEUE_SHM_ADDR_HIGH: usize = 0x0D4;
    pub const QUEUE_SHM_LEN_LOW: usize = 0x0D8;
    pub const QUEUE_SHM_LEN_HIGH: usize = 0x0DC;
    pub const QUEUE_SHM_V2_CFG: usize = 0x0E0;
}

/// VirtIO device status flags
pub mod status_flags {
    pub const ACKNOWLEDGE: u32 = 0x01;
    pub const DRIVER: u32 = 0x02;
    pub const FAILED: u32 = 0x80;
    pub const FEATURES_OK: u32 = 0x08;
    pub const DRIVER_OK: u32 = 0x04;
    pub const DEVICE_NEEDS_RESET: u32 = 0x40;
}

/// VirtIO queue descriptor
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VirtQueueDescriptor {
    /// Physical address of buffer
    pub addr: u64,
    /// Length of buffer
    pub len: u32,
    /// Flags
    pub flags: u16,
    /// Next descriptor if VIRTQ_DESC_F_NEXT is set
    pub next: u16,
}

/// VirtIO queue flags
pub mod desc_flags {
    pub const VIRTQ_DESC_F_NEXT: u16 = 1;
    pub const VIRTQ_DESC_F_WRITE: u16 = 2;
    pub const VIRTQ_DESC_F_INDIRECT: u16 = 4;
}

/// VirtIO available ring
#[repr(C)]
#[derive(Debug, Clone)]
pub struct VirtQueueAvailable {
    pub flags: u16,
    pub idx: u16,
    pub ring: [u16; 0], // Variable length
    pub used_event: u16, // Only if VIRTIO_F_EVENT_IDX
}

/// VirtIO used ring
#[repr(C)]
#[derive(Debug, Clone)]
pub struct VirtQueueUsed {
    pub flags: u16,
    pub idx: u16,
    pub ring: [VirtQueueUsedElem; 0], // Variable length
    pub avail_event: u16, // Only if VIRTIO_F_EVENT_IDX
}

/// VirtIO used element
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VirtQueueUsedElem {
    pub id: u32,
    pub len: u32,
}

/// VirtIO queue implementation
pub struct VirtQueue {
    /// Queue index
    pub index: u16,
    /// Maximum queue size
    pub max_size: u16,
    /// Current queue size
    pub size: u16,
    /// Queue ready status
    pub ready: bool,
    /// Descriptor table
    pub descriptors: Vec<VirtQueueDescriptor>,
    /// Available ring
    pub available: VirtQueueAvailable,
    /// Used ring
    pub used: VirtQueueUsed,
    /// Free descriptor list
    pub free_desc: Vec<u16>,
    /// Last used index
    pub last_used_idx: u16,
    /// Queue physical address
    pub phys_addr: Option<PhysAddr>,
}

impl VirtQueue {
    /// Create new VirtIO queue
    pub fn new(index: u16, max_size: u16) -> Self {
        Self {
            index,
            max_size,
            size: 0,
            ready: false,
            descriptors: Vec::new(),
            available: VirtQueueAvailable { flags: 0, idx: 0, ring: [], used_event: 0 },
            used: VirtQueueUsed { flags: 0, idx: 0, ring: [], avail_event: 0 },
            free_desc: Vec::new(),
            last_used_idx: 0,
            phys_addr: None,
        }
    }

    /// Initialize queue with size
    pub fn init(&mut self, size: u16) -> Result<(), &'static str> {
        if size > self.max_size || size == 0 {
            return Err("Invalid queue size");
        }

        self.size = size;
        self.descriptors.resize(size as usize, VirtQueueDescriptor {
            addr: 0,
            len: 0,
            flags: 0,
            next: 0,
        });

        // Initialize free descriptor list
        self.free_desc.clear();
        for i in 0..size {
            self.free_desc.push(i);
        }

        self.last_used_idx = 0;

        Ok(())
    }

    /// Allocate a descriptor
    pub fn alloc_desc(&mut self) -> Option<u16> {
        self.free_desc.pop()
    }

    /// Free a descriptor
    pub fn free_desc(&mut self, desc: u16) {
        self.free_desc.push(desc);
    }

    /// Add buffer to available ring
    pub fn add_buf(&mut self, desc: u16, len: u32, write: bool, next_desc: Option<u16>) -> Result<(), &'static str> {
        if desc >= self.size as u16 {
            return Err("Invalid descriptor");
        }

        let descriptor = &mut self.descriptors[desc as usize];
        descriptor.addr = 0; // Will be set by caller
        descriptor.len = len;
        descriptor.flags = if write { desc_flags::VIRTQ_DESC_F_WRITE } else { 0 };
        descriptor.next = next_desc.unwrap_or(0);

        if next_desc.is_some() {
            descriptor.flags |= desc_flags::VIRTQ_DESC_F_NEXT;
        }

        // Add to available ring (simplified)
        // In a real implementation, this would need proper ring management

        Ok(())
    }

    /// Get used buffer
    pub fn get_used_buf(&mut self) -> Option<(u32, u32)> {
        if self.last_used_idx != self.used.idx {
            // Return the next used buffer (simplified)
            let idx = self.last_used_idx;
            self.last_used_idx = (self.last_used_idx + 1) % self.size;
            Some((0, 0)) // Would return actual used element data
        } else {
            None
        }
    }
}

/// VirtIO device configuration
pub struct VirtIODeviceConfig {
    /// Device type
    pub device_type: VirtIODeviceType,
    /// Device ID
    pub device_id: u32,
    /// Vendor ID
    pub vendor_id: u32,
    /// Device features
    pub device_features: u64,
    /// Driver features (negotiated)
    pub driver_features: u64,
    /// Queue configurations
    pub queues: Vec<VirtQueueConfig>,
    /// Device-specific configuration data
    pub config_data: Vec<u8>,
    /// Config space generation
    pub config_generation: u32,
    /// MMIO base address
    pub mmio_base: PhysAddr,
    /// MMIO size
    pub mmio_size: u64,
    /// Interrupt line
    pub interrupt: u32,
}

/// VirtIO queue configuration
#[derive(Debug, Clone)]
pub struct VirtQueueConfig {
    /// Queue index
    pub index: u16,
    /// Maximum queue size
    pub max_size: u16,
    /// Actual queue size
    pub size: u16,
    /// Queue ready status
    pub ready: bool,
    /// Notification offset
    pub notify_off: u32,
}

/// VirtIO device statistics
#[derive(Debug, Default)]
pub struct VirtIODeviceStats {
    /// Total bytes transferred
    pub bytes_transferred: AtomicU64,
    /// Total operations
    pub operations: AtomicU64,
    /// Total interrupts
    pub interrupts: AtomicU64,
    /// Errors
    pub errors: AtomicU64,
    /// Queue statistics
    pub queue_stats: Vec<VirtQueueStats>,
}

/// VirtIO queue statistics
#[derive(Debug, Default)]
pub struct VirtQueueStats {
    /// Total buffers processed
    pub buffers_processed: AtomicU64,
    /// Available ring updates
    pub available_updates: AtomicU64,
    /// Used ring updates
    pub used_updates: AtomicU64,
    /// Queue full events
    pub full_events: AtomicU64,
}

/// VirtIO device implementation
pub struct VirtIODevice {
    /// Device configuration
    pub config: VirtIODeviceConfig,
    /// VirtIO queues
    pub queues: Vec<VirtQueue>,
    /// Device state
    pub status: DeviceStatus,
    /// Device features
    pub device_features: u64,
    /// Driver features
    pub driver_features: u64,
    /// Current feature selection
    pub feature_select: u32,
    /// Current queue selection
    pub queue_select: u16,
    /// MMIO registers
    pub registers: VirtIORegisters,
    /// Statistics
    pub stats: VirtIODeviceStats,
    /// VM ID this device belongs to
    pub vm_id: Option<VmId>,
    /// Device lock
    pub lock: SpinLock<()>,
}

/// VirtIO MMIO registers
#[derive(Debug)]
pub struct VirtIORegisters {
    /// Magic value
    pub magic_value: u32,
    /// Version
    pub version: u32,
    /// Device ID
    pub device_id: u32,
    /// Vendor ID
    pub vendor_id: u32,
    /// Device features
    pub device_features: u32,
    /// Device features select
    pub device_features_sel: u32,
    /// Driver features
    pub driver_features: u32,
    /// Driver features select
    pub driver_features_sel: u32,
    /// Queue selector
    pub queue_sel: u32,
    /// Queue number max
    pub queue_num_max: u32,
    /// Queue number
    pub queue_num: u32,
    /// Queue ready
    pub queue_ready: u32,
    /// Notification offset
    pub notify_off: u32,
    /// Device status
    pub device_status: u32,
    /// Configuration generation
    pub config_generation: u32,
}

impl VirtIODevice {
    /// Create new VirtIO device
    pub fn new(config: VirtIODeviceConfig) -> Self {
        let mut device = Self {
            config,
            queues: Vec::new(),
            status: DeviceStatus::NotPresent,
            device_features: 0,
            driver_features: 0,
            feature_select: 0,
            queue_select: 0,
            registers: VirtIORegisters {
                magic_value: 0x74726976, // "virt" in little endian
                version: 2, // VirtIO 1.0+ legacy
                device_id: 0,
                vendor_id: 0,
                device_features: 0,
                device_features_sel: 0,
                driver_features: 0,
                driver_features_sel: 0,
                queue_sel: 0,
                queue_num_max: 0,
                queue_num: 0,
                queue_ready: 0,
                notify_off: 0,
                device_status: 0,
                config_generation: 0,
            },
            stats: VirtIODeviceStats {
                queue_stats: Vec::new(),
                ..Default::default()
            },
            vm_id: None,
            lock: SpinLock::new(()),
        };

        // Initialize registers from config
        device.registers.device_id = device.config.device_id;
        device.registers.vendor_id = device.config.vendor_id;
        device.device_features = device.config.device_features;
        device.registers.device_features = device.device_features as u32;

        // Initialize queues
        for queue_config in &device.config.queues {
            let mut queue = VirtQueue::new(queue_config.index, queue_config.max_size);
            let _ = queue.init(queue_config.size);
            device.queues.push(queue);
        }

        // Initialize queue statistics
        device.stats.queue_stats.resize(device.queues.len(), VirtQueueStats::default());

        device
    }

    /// Read MMIO register
    pub fn read_register(&self, offset: usize) -> Result<u32, &'static str> {
        match offset {
            registers::MAGIC_VALUE => Ok(self.registers.magic_value),
            registers::VERSION => Ok(self.registers.version),
            registers::DEVICE_ID => Ok(self.registers.device_id),
            registers::VENDOR_ID => Ok(self.registers.vendor_id),
            registers::DEVICE_FEATURES => Ok(self.registers.device_features),
            registers::DRIVER_FEATURES => Ok(self.registers.driver_features),
            registers::QUEUE_NUM_MAX => {
                let queue_index = self.registers.queue_sel as usize;
                if queue_index < self.queues.len() {
                    Ok(self.queues[queue_index].max_size as u32)
                } else {
                    Ok(0)
                }
            }
            registers::QUEUE_NUM => {
                let queue_index = self.registers.queue_sel as usize;
                if queue_index < self.queues.len() {
                    Ok(self.queues[queue_index].size as u32)
                } else {
                    Ok(0)
                }
            }
            registers::QUEUE_READY => {
                let queue_index = self.registers.queue_sel as usize;
                if queue_index < self.queues.len() && self.queues[queue_index].ready {
                    Ok(1)
                } else {
                    Ok(0)
                }
            }
            registers::DEVICE_STATUS => Ok(self.registers.device_status),
            registers::CONFIG_GENERATION => Ok(self.registers.config_generation),
            _ => {
                // Handle queue-specific registers
                if offset >= registers::QUEUE_DESC_LOW && offset <= registers::QUEUE_DESC_HIGH {
                    // Queue descriptor table address
                    self.get_queue_address(offset)
                } else if offset >= registers::QUEUE_DRIVER_LOW && offset <= registers::QUEUE_DRIVER_HIGH {
                    // Queue available ring address
                    self.get_queue_driver_address(offset)
                } else if offset >= registers::QUEUE_DEVICE_LOW && offset <= registers::QUEUE_DEVICE_HIGH {
                    // Queue used ring address
                    self.get_queue_device_address(offset)
                } else {
                    // Read from config space
                    self.read_config(offset)
                }
            }
        }
    }

    /// Write MMIO register
    pub fn write_register(&mut self, offset: usize, value: u32) -> Result<(), &'static str> {
        match offset {
            registers::DEVICE_FEATURES_SEL => {
                self.registers.device_features_sel = value;
                self.registers.device_features = ((self.device_features >> (value * 32)) & 0xFFFFFFFF) as u32;
            }
            registers::DRIVER_FEATURES_SEL => {
                self.registers.driver_features_sel = value;
            }
            registers::DRIVER_FEATURES => {
                self.registers.driver_features = value;
                let bit_shift = self.registers.driver_features_sel * 32;
                self.driver_features = (self.driver_features & !(0xFFFFFFFFu64 << bit_shift)) | ((value as u64) << bit_shift);
            }
            registers::QUEUE_SEL => {
                self.registers.queue_sel = value;
            }
            registers::QUEUE_NUM => {
                let queue_index = self.registers.queue_sel as usize;
                if queue_index < self.queues.len() {
                    if value <= self.queues[queue_index].max_size as u32 {
                        self.queues[queue_index].size = value as u16;
                        self.registers.queue_num = value;
                    } else {
                        return Err("Queue size too large");
                    }
                }
            }
            registers::QUEUE_READY => {
                let queue_index = self.registers.queue_sel as usize;
                if queue_index < self.queues.len() {
                    self.queues[queue_index].ready = value != 0;
                    self.registers.queue_ready = value;
                }
            }
            registers::DEVICE_STATUS => {
                self.registers.device_status = value;
                self.handle_status_change(value)?;
            }
            registers::QUEUE_NOTIFY => {
                // Queue notification - driver is notifying us about new buffers
                self.handle_queue_notification(self.registers.queue_sel)?;
            }
            _ => {
                if offset >= registers::QUEUE_DESC_LOW && offset <= registers::QUEUE_DESC_HIGH {
                    // Set queue descriptor table address
                    self.set_queue_address(offset, value)?;
                } else if offset >= registers::QUEUE_DRIVER_LOW && offset <= registers::QUEUE_DRIVER_HIGH {
                    // Set queue available ring address
                    self.set_queue_driver_address(offset, value)?;
                } else if offset >= registers::QUEUE_DEVICE_LOW && offset <= registers::QUEUE_DEVICE_HIGH {
                    // Set queue used ring address
                    self.set_queue_device_address(offset, value)?;
                } else {
                    // Write to config space
                    self.write_config(offset, value)?;
                }
            }
        }

        Ok(())
    }

    /// Handle device status change
    fn handle_status_change(&mut self, status: u32) -> Result<(), &'static str> {
        if status & status_flags::FAILED != 0 {
            log::warn!("VirtIO device {:?} reported failure", self.config.device_type);
            self.status = DeviceStatus::Error;
        } else if status & status_flags::DRIVER_OK != 0 {
            log::info!("VirtIO device {:?} initialization complete", self.config.device_type);
            self.status = DeviceStatus::Ready;
        } else if status & status_flags::FEATURES_OK != 0 {
            log::info!("VirtIO device {:?} features negotiated", self.config.device_type);
            // Feature negotiation complete
        } else if status & status_flags::DRIVER != 0 {
            log::info!("VirtIO device {:?} driver loaded", self.config.device_type);
            self.status = DeviceStatus::Initializing;
        } else if status & status_flags::ACKNOWLEDGE != 0 {
            log::info!("VirtIO device {:?} acknowledged", self.config.device_type);
            self.status = DeviceStatus::Present;
        }

        Ok(())
    }

    /// Handle queue notification
    fn handle_queue_notification(&mut self, queue_index: u32) -> Result<(), &'static str> {
        let queue_index = queue_index as usize;
        if queue_index >= self.queues.len() {
            return Err("Invalid queue index");
        }

        log::debug!("VirtIO queue {} notification received", queue_index);

        // Update statistics
        self.stats.queue_stats[queue_index].available_updates.fetch_add(1, Ordering::Relaxed);

        // Process queue buffers
        self.process_queue_buffers(queue_index)?;

        Ok(())
    }

    /// Process queue buffers
    fn process_queue_buffers(&mut self, queue_index: usize) -> Result<(), &'static str> {
        // This would be implemented by specific device types
        // For now, just simulate processing
        log::debug!("Processing buffers for queue {}", queue_index);
        self.stats.operations.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Get queue address
    fn get_queue_address(&self, offset: usize) -> Result<u32, &'static str> {
        let queue_index = self.registers.queue_sel as usize;
        if queue_index >= self.queues.len() {
            return Err("Invalid queue index");
        }

        let queue = &self.queues[queue_index];
        if let Some(phys_addr) = queue.phys_addr {
            let addr = phys_addr.as_u64();
            match offset {
                registers::QUEUE_DESC_LOW => Ok((addr & 0xFFFFFFFF) as u32),
                registers::QUEUE_DESC_HIGH => Ok((addr >> 32) as u32),
                _ => Err("Invalid queue address register"),
            }
        } else {
            Ok(0)
        }
    }

    /// Get queue driver address
    fn get_queue_driver_address(&self, offset: usize) -> Result<u32, &'static str> {
        // Simplified implementation
        Ok(0)
    }

    /// Get queue device address
    fn get_queue_device_address(&self, offset: usize) -> Result<u32, &'static str> {
        // Simplified implementation
        Ok(0)
    }

    /// Set queue address
    fn set_queue_address(&mut self, offset: usize, value: u32) -> Result<(), &'static str> {
        let queue_index = self.registers.queue_sel as usize;
        if queue_index >= self.queues.len() {
            return Err("Invalid queue index");
        }

        // Simplified implementation - would need proper address handling
        log::debug!("Set queue {} descriptor address: {:#x}", queue_index, value);

        Ok(())
    }

    /// Set queue driver address
    fn set_queue_driver_address(&mut self, offset: usize, value: u32) -> Result<(), &'static str> {
        // Simplified implementation
        log::debug!("Set queue driver address: {:#x}", value);
        Ok(())
    }

    /// Set queue device address
    fn set_queue_device_address(&mut self, offset: usize, value: u32) -> Result<(), &'static str> {
        // Simplified implementation
        log::debug!("Set queue device address: {:#x}", value);
        Ok(())
    }

    /// Read from config space
    fn read_config(&self, offset: usize) -> Result<u32, &'static str> {
        let config_offset = offset - registers::CONFIG;
        if config_offset >= self.config.config_data.len() {
            return Ok(0);
        }

        let data = &self.config.config_data[config_offset..];
        if data.len() >= 4 {
            Ok(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
        } else {
            let mut buf = [0u8; 4];
            buf[..data.len()].copy_from_slice(data);
            Ok(u32::from_le_bytes(buf))
        }
    }

    /// Write to config space
    fn write_config(&mut self, offset: usize, value: u32) -> Result<(), &'static str> {
        let config_offset = offset - registers::CONFIG;
        if config_offset >= self.config.config_data.len() {
            return Err("Config offset out of range");
        }

        let data = value.to_le_bytes();
        let end = core::cmp::min(config_offset + 4, self.config.config_data.len());
        self.config.config_data[config_offset..end].copy_from_slice(&data[..end - config_offset]);

        Ok(())
    }

    /// Get device statistics
    pub fn get_stats(&self) -> &VirtIODeviceStats {
        &self.stats
    }

    /// Interrupt the VM
    pub fn interrupt_vm(&self) -> Result<(), &'static str> {
        // This would trigger an interrupt to the VM
        log::debug!("Interrupting VM for VirtIO device {:?}", self.config.device_type);
        self.stats.interrupts.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Reset the device
    pub fn reset(&mut self) {
        self.registers.device_status = 0;
        self.status = DeviceStatus::NotPresent;
        self.driver_features = 0;

        for queue in &mut self.queues {
            queue.ready = false;
        }

        log::info!("VirtIO device {:?} reset", self.config.device_type);
    }
}

/// VirtIO device factory
pub struct VirtIODeviceFactory;

impl VirtIODeviceFactory {
    /// Create VirtIO device from description
    pub fn create_device(desc: &VirtualDeviceDesc) -> Result<VirtIODevice, &'static str> {
        // Determine device type
        let device_type = match desc.device_type {
            DeviceType::Network => VirtIODeviceType::Network,
            DeviceType::Block => VirtIODeviceType::Block,
            DeviceType::Console => VirtIODeviceType::Console,
            DeviceType::Rng => VirtIODeviceType::Rng,
            DeviceType::Graphics => VirtIODeviceType::GPU,
            DeviceType::Input => VirtIODeviceType::Input,
            _ => return Err("Unsupported VirtIO device type"),
        };

        // Create device configuration
        let config = VirtIODeviceConfig {
            device_type,
            device_id: desc.vendor_id, // Use as device ID
            vendor_id: desc.product_id,  // Use as vendor ID
            device_features: Self::get_device_features(device_type),
            driver_features: 0,
            queues: Self::create_queue_configs(device_type),
            config_data: Self::create_config_data(device_type),
            config_generation: 0,
            mmio_base: desc.resources.memory_regions.first()
                .ok_or("No MMIO region found")?
                .base
                .ok_or("MMIO region not mapped")?,
            mmio_size: desc.resources.memory_regions.first()
                .map(|r| r.size)
                .unwrap_or(0x1000),
            interrupt: desc.resources.irqs.first()
                .and_then(|irq| irq.irq_num)
                .unwrap_or(0),
        };

        let mut device = VirtIODevice::new(config);
        device.status = DeviceStatus::Present;

        log::info!("Created VirtIO {:?} device", device_type);

        Ok(device)
    }

    /// Get device features for specific device type
    fn get_device_features(device_type: VirtIODeviceType) -> u64 {
        let mut features = (1u64 << features::VIRTIO_F_VERSION_1) |
                         (1u64 << features::VIRTIO_F_RING_EVENT_IDX);

        match device_type {
            VirtIODeviceType::Network => {
                features |= (1u64 << features::net::VIRTIO_NET_F_MAC) |
                           (1u64 << features::net::VIRTIO_NET_F_STATUS) |
                           (1u64 << features::net::VIRTIO_NET_F_MRG_RXBUF) |
                           (1u64 << features::net::VIRTIO_NET_F_CTRL_VQ);
            }
            VirtIODeviceType::Block => {
                features |= (1u64 << features::blk::VIRTIO_BLK_F_FLUSH) |
                           (1u64 << features::blk::VIRTIO_BLK_F_DISCARD) |
                           (1u64 << features::blk::VIRTIO_BLK_F_WRITE_ZEROES) |
                           (1u64 << features::blk::VIRTIO_BLK_F_CONFIG_WCE);
            }
            VirtIODeviceType::Console => {
                // Console devices have minimal features
            }
            VirtIODeviceType::Rng => {
                // RNG devices have minimal features
            }
            _ => {}
        }

        features
    }

    /// Create queue configurations for device type
    fn create_queue_configs(device_type: VirtIODeviceType) -> Vec<VirtQueueConfig> {
        match device_type {
            VirtIODeviceType::Network => vec![
                VirtQueueConfig { index: 0, max_size: 256, size: 0, ready: false, notify_off: 0 }, // RX
                VirtQueueConfig { index: 1, max_size: 256, size: 0, ready: false, notify_off: 1 }, // TX
                VirtQueueConfig { index: 2, max_size: 64, size: 0, ready: false, notify_off: 2 },  // Control
            ],
            VirtIODeviceType::Block => vec![
                VirtQueueConfig { index: 0, max_size: 128, size: 0, ready: false, notify_off: 0 }, // Request
            ],
            VirtIODeviceType::Console => vec![
                VirtQueueConfig { index: 0, max_size: 128, size: 0, ready: false, notify_off: 0 }, // RX/TX
            ],
            VirtIODeviceType::Rng => vec![
                VirtQueueConfig { index: 0, max_size: 16, size: 0, ready: false, notify_off: 0 }, // Request
                VirtQueueConfig { index: 1, max_size: 16, size: 0, ready: false, notify_off: 1 }, // Response
            ],
            VirtIODeviceType::GPU => vec![
                VirtQueueConfig { index: 0, max_size: 256, size: 0, ready: false, notify_off: 0 }, // Control
                VirtQueueConfig { index: 1, max_size: 256, size: 0, ready: false, notify_off: 1 }, // Cursor
                VirtQueueConfig { index: 2, max_size: 256, size: 0, ready: false, notify_off: 2 }, // Events
            ],
            VirtIODeviceType::Input => vec![
                VirtQueueConfig { index: 0, max_size: 64, size: 0, ready: false, notify_off: 0 }, // Events
                VirtQueueConfig { index: 1, max_size: 64, size: 0, ready: false, notify_off: 1 }, // Status
            ],
            _ => vec![],
        }
    }

    /// Create config data for device type
    fn create_config_data(device_type: VirtIODeviceType) -> Vec<u8> {
        match device_type {
            VirtIODeviceType::Network => {
                // Network device config: MAC address (6 bytes) + status + other fields
                let mut config = vec![0u8; 12]; // Standard network config space
                config[0] = 0x52; // Default MAC: 52:54:00:12:34:56
                config[1] = 0x54;
                config[2] = 0x00;
                config[3] = 0x12;
                config[4] = 0x34;
                config[5] = 0x56;
                config[6] = 1; // Link up
                config[7] = 1; // Link up
                config
            }
            VirtIODeviceType::Block => {
                // Block device config: capacity, size, etc.
                let mut config = vec![0u8; 8]; // 8-byte capacity
                // Set 1GB capacity
                config[0..8].copy_from_slice(&0x40000000u64.to_le_bytes());
                config
            }
            VirtIODeviceType::Console => {
                // Console device has minimal config
                vec![0u8]
            }
            VirtIODeviceType::Rng => {
                // RNG device has minimal config
                vec![0u8]
            }
            VirtIODeviceType::GPU => {
                // GPU config: display information
                let mut config = vec![0u8; 16];
                config[0] = 1; // Number of scanouts
                config
            }
            VirtIODeviceType::Input => {
                // Input config: device information
                vec![0u8; 8]
            }
            _ => vec![],
        }
    }
}