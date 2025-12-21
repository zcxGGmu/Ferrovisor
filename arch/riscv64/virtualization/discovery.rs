//! RISC-V Virtual Device Discovery and Enumeration
//!
//! This module provides comprehensive virtual device discovery and enumeration
//! functionality based on xvisor patterns, including:
//! - Runtime virtual device discovery
//! - Device capability detection and enumeration
//! - Dynamic device creation based on guest requirements
//! - Device class hierarchy and compatibility checking
//! - Device namespace management
//! - Hotplug device discovery
//! - Device tree-based discovery
//! - VirtIO device enumeration

use crate::arch::riscv64::devtree::{FlattenedDeviceTree, DeviceTreeParser};
use crate::arch::riscv64::virtualization::devices::{VirtualDevice, VmDeviceConfig};
use crate::arch::riscv64::virtualization::{VmId, VcpuId};
use crate::core::mm::{PhysAddr, VirtAddr};
use crate::drivers::{DeviceType, DeviceId, DeviceStatus};
use alloc::collections::{BTreeMap, VecDeque};
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Device capability descriptor
#[derive(Debug, Clone)]
pub struct DeviceCapability {
    /// Capability name
    pub name: String,
    /// Capability version
    pub version: u32,
    /// Capability type
    pub cap_type: CapabilityType,
    /// Capability value (if applicable)
    pub value: u64,
    /// Additional properties
    pub properties: BTreeMap<String, String>,
}

/// Device capability types
#[derive(Debug, Clone, PartialEq)]
pub enum CapabilityType {
    /// Boolean capability
    Boolean,
    /// Integer capability
    Integer,
    /// String capability
    String,
    /// Range capability (min, max)
    Range(u64, u64),
    /// List capability
    List,
}

/// Device class information
#[derive(Debug, Clone)]
pub struct DeviceClass {
    /// Class name
    pub name: String,
    /// Class ID
    pub id: u32,
    /// Parent class ID (if any)
    pub parent_id: Option<u32>,
    /// Required capabilities
    pub required_caps: Vec<String>,
    /// Optional capabilities
    pub optional_caps: Vec<String>,
    /// Compatibility strings
    pub compatible: Vec<String>,
}

/// Virtual device descriptor
#[derive(Debug, Clone)]
pub struct VirtualDeviceDesc {
    /// Device identifier
    pub device_id: DeviceId,
    /// Device name
    pub name: String,
    /// Device class
    pub class: DeviceClass,
    /// Device type
    pub device_type: DeviceType,
    /// Vendor ID
    pub vendor_id: u32,
    /// Device ID
    pub product_id: u32,
    /// Capabilities
    pub capabilities: Vec<DeviceCapability>,
    /// Compatible strings
    pub compatible: Vec<String>,
    /// Device tree path (if from device tree)
    pub dt_path: Option<String>,
    /// Resource requirements
    pub resources: DeviceResources,
    /// Device status
    pub status: DeviceStatus,
}

/// Device resource requirements
#[derive(Debug, Clone, Default)]
pub struct DeviceResources {
    /// Memory regions
    pub memory_regions: Vec<MemoryRegion>,
    /// IRQ requirements
    pub irqs: Vec<IrqResource>,
    /// DMA requirements
    pub dma: Vec<DmaResource>,
    /// Clock requirements
    pub clocks: Vec<ClockResource>,
    /// Power requirements
    pub power: Vec<PowerResource>,
}

/// Memory region requirement
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    /// Region name
    pub name: String,
    /// Physical base address (None for dynamic allocation)
    pub base: Option<PhysAddr>,
    /// Size in bytes
    pub size: u64,
    /// Alignment requirement
    pub alignment: u64,
    /// Memory type
    pub mem_type: MemoryType,
    /// Access flags
    pub flags: MemoryFlags,
}

/// Memory types
#[derive(Debug, Clone, PartialEq)]
pub enum MemoryType {
    /// Regular memory
    Regular,
    /// Device memory
    Device,
    /// Prefetchable memory
    Prefetchable,
    /// Coherent memory
    Coherent,
}

/// Memory access flags
#[derive(Debug, Clone)]
pub struct MemoryFlags {
    /// Readable
    pub readable: bool,
    /// Writable
    pub writable: bool,
    /// Executable
    pub executable: bool,
    /// Cacheable
    pub cacheable: bool,
    /// Shareable
    pub shareable: bool,
}

/// IRQ resource requirement
#[derive(Debug, Clone)]
pub struct IrqResource {
    /// IRQ name
    pub name: String,
    /// IRQ type
    pub irq_type: IrqType,
    /// IRQ number (if fixed)
    pub irq_num: Option<u32>,
    /// Trigger type
    pub trigger: IrqTrigger,
    /// Priority
    pub priority: u32,
    /// Affinity mask (CPU mask)
    pub affinity: u64,
}

/// IRQ types
#[derive(Debug, Clone, PartialEq)]
pub enum IrqType {
    /// Legacy IRQ
    Legacy,
    /// MSI
    MSI,
    /// MSI-X
    MSIX,
    /// LPI
    LPI,
}

/// IRQ trigger types
#[derive(Debug, Clone, PartialEq)]
pub enum IrqTrigger {
    /// Edge triggered (rising)
    EdgeRising,
    /// Edge triggered (falling)
    EdgeFalling,
    /// Edge triggered (both)
    EdgeBoth,
    /// Level triggered (high)
    LevelHigh,
    /// Level triggered (low)
    LevelLow,
}

/// DMA resource requirement
#[derive(Debug, Clone)]
pub struct DmaResource {
    /// DMA channel name
    pub name: String,
    /// Channel ID (if fixed)
    pub channel_id: Option<u32>,
    /// Maximum transfer size
    pub max_transfer_size: u32,
    /// Alignment requirement
    pub alignment: u32,
    /// Coherent DMA required
    pub coherent: bool,
}

/// Clock resource requirement
#[derive(Debug, Clone)]
pub struct ClockResource {
    /// Clock name
    pub name: String,
    /// Clock ID (if fixed)
    pub clock_id: Option<u32>,
    /// Minimum frequency
    pub min_freq: u32,
    /// Maximum frequency
    pub max_freq: u32,
    /// Required frequency (if fixed)
    pub required_freq: Option<u32>,
}

/// Power resource requirement
#[derive(Debug, Clone)]
pub struct PowerResource {
    /// Power domain name
    pub name: String,
    /// Domain ID (if fixed)
    pub domain_id: Option<u32>,
    /// Voltage requirement (mV)
    pub voltage: Option<u32>,
    /// Current requirement (mA)
    pub current: Option<u32>,
}

/// Device discovery statistics
#[derive(Debug, Default)]
pub struct DiscoveryStats {
    /// Total devices discovered
    pub total_devices: AtomicU32,
    /// Device types discovered
    pub device_types: BTreeMap<DeviceType, AtomicU32>,
    /// Discovery time (microseconds)
    pub discovery_time_us: AtomicU64,
    /// Failed discoveries
    pub failed_discoveries: AtomicU32,
    /// Hotplug events
    pub hotplug_events: AtomicU32,
}

/// Virtual device discovery manager
pub struct VirtualDeviceDiscovery {
    /// Device registry
    devices: BTreeMap<DeviceId, VirtualDeviceDesc>,
    /// Device class registry
    classes: BTreeMap<u32, DeviceClass>,
    /// Device tree parser
    dt_parser: DeviceTreeParser,
    /// Next device ID
    next_device_id: AtomicU32,
    /// Discovery statistics
    stats: DiscoveryStats,
    /// Hotplug queue
    hotplug_queue: VecDeque<HotplugEvent>,
}

impl VirtualDeviceDiscovery {
    /// Create new device discovery manager
    pub fn new() -> Self {
        let mut discovery = Self {
            devices: BTreeMap::new(),
            classes: BTreeMap::new(),
            dt_parser: DeviceTreeParser::new(),
            next_device_id: AtomicU32::new(1),
            stats: DiscoveryStats::default(),
            hotplug_queue: VecDeque::new(),
        };

        discovery.init_device_classes();
        discovery
    }

    /// Initialize device classes
    fn init_device_classes(&mut self) {
        // Base device class
        let base_class = DeviceClass {
            name: "base".to_string(),
            id: 0,
            parent_id: None,
            required_caps: vec![],
            optional_caps: vec![],
            compatible: vec![],
        };
        self.classes.insert(0, base_class);

        // Network device class
        let net_class = DeviceClass {
            name: "network".to_string(),
            id: 1,
            parent_id: Some(0),
            required_caps: vec!["ethernet".to_string()],
            optional_caps: vec!["tsn".to_string(), "offload".to_string()],
            compatible: vec!["virtio,net".to_string(), "cdns,macb".to_string()],
        };
        self.classes.insert(1, net_class);

        // Block device class
        let block_class = DeviceClass {
            name: "block".to_string(),
            id: 2,
            parent_id: Some(0),
            required_caps: vec!["block".to_string()],
            optional_caps: vec!["trim".to_string(), "discard".to_string(), "writeback".to_string()],
            compatible: vec!["virtio,block".to_string(), "arm,pl061".to_string()],
        };
        self.classes.insert(2, block_class);

        // Console device class
        let console_class = DeviceClass {
            name: "console".to_string(),
            id: 3,
            parent_id: Some(0),
            required_caps: vec!["serial".to_string()],
            optional_caps: vec!["console".to_string(), "break".to_string()],
            compatible: vec!["virtio,console".to_string(), "ns16550a".to_string()],
        };
        self.classes.insert(3, console_class);

        // GPU device class
        let gpu_class = DeviceClass {
            name: "gpu".to_string(),
            id: 4,
            parent_id: Some(0),
            required_caps: vec!["display".to_string()],
            optional_caps: vec!["3d".to_string(), "video".to_string(), "cursor".to_string()],
            compatible: vec!["virtio,gpu".to_string(), "pl111".to_string()],
        };
        self.classes.insert(4, gpu_class);

        // Input device class
        let input_class = DeviceClass {
            name: "input".to_string(),
            id: 5,
            parent_id: Some(0),
            required_caps: vec!["input".to_string()],
            optional_caps: vec!["keyboard".to_string(), "mouse".to_string(), "touch".to_string()],
            compatible: vec!["virtio,input".to_string(), "hid".to_string()],
        };
        self.classes.insert(5, input_class);

        // RNG device class
        let rng_class = DeviceClass {
            name: "rng".to_string(),
            id: 6,
            parent_id: Some(0),
            required_caps: vec!["rng".to_string()],
            optional_caps: vec!["quality".to_string(), "speed".to_string()],
            compatible: vec!["virtio,rng".to_string(), "hwrng".to_string()],
        };
        self.classes.insert(6, rng_class);
    }

    /// Discover devices from device tree
    pub fn discover_from_device_tree(&mut self, fdt: &FlattenedDeviceTree) -> Result<Vec<DeviceId>, &'static str> {
        let start_time = crate::utils::time::get_microseconds();
        let mut discovered_devices = Vec::new();

        // Get root node
        let root = fdt.get_root().ok_or("No root node in device tree")?;

        // Discover platform devices
        self.discover_platform_devices(fdt, &root, &mut discovered_devices)?;

        // Discover VirtIO devices
        self.discover_virtio_devices(fdt, &mut discovered_devices)?;

        // Discover interrupt controllers
        self.discover_interrupt_controllers(fdt, &mut discovered_devices)?;

        // Discover CPU devices
        self.discover_cpu_devices(fdt, &mut discovered_devices)?;

        let end_time = crate::utils::time::get_microseconds();
        self.stats.discovery_time_us.store(
            end_time.saturating_sub(start_time),
            Ordering::Relaxed
        );

        Ok(discovered_devices)
    }

    /// Discover platform devices from device tree
    fn discover_platform_devices(
        &mut self,
        fdt: &FlattenedDeviceTree,
        root: &crate::arch::riscv64::devtree::fdt::Node,
        discovered_devices: &mut Vec<DeviceId>,
    ) -> Result<(), &'static str> {
        // Look for common platform devices
        let platform_device_paths = [
            "/soc/serial",
            "/soc/uart",
            "/soc/timer",
            "/soc/gpio",
            "/soc/i2c",
            "/soc/spi",
            "/pci",
            "/ahb",
            "/apb",
        ];

        for path in &platform_device_paths {
            if let Some(node) = fdt.find_node(path) {
                if let Some(device_desc) = self.create_platform_device_desc(fdt, node, path)? {
                    let device_id = device_desc.device_id;
                    self.devices.insert(device_id, device_desc);
                    discovered_devices.push(device_id);

                    self.stats.total_devices.fetch_add(1, Ordering::Relaxed);
                    *self.stats.device_types.entry(DeviceType::Platform)
                        .or_insert_with(|| AtomicU32::new(0))
                        .fetch_add(1, Ordering::Relaxed);
                }
            }
        }

        Ok(())
    }

    /// Create platform device descriptor
    fn create_platform_device_desc(
        &self,
        fdt: &FlattenedDeviceTree,
        node: &crate::arch::riscv64::devtree::fdt::Node,
        path: &str,
    ) -> Result<Option<VirtualDeviceDesc>, &'static str> {
        // Get compatible property
        let compatible = if let Some(prop) = node.get_property("compatible") {
            let compat_str = core::str::from_utf8(prop.data).unwrap_or("");
            compat_str.split('\0')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect()
        } else {
            return Ok(None);
        };

        // Determine device class based on compatible strings
        let class = self.determine_device_class(&compatible)?;

        // Get device name
        let name = if let Some(prop) = node.get_property("name") {
            core::str::from_utf8(prop.data).unwrap_or("unknown").to_string()
        } else {
            format!("platform_device_{}", path.split('/').last().unwrap_or("unknown"))
        };

        // Parse resources
        let resources = self.parse_device_resources(fdt, node)?;

        // Create device descriptor
        let device_desc = VirtualDeviceDesc {
            device_id: DeviceId::from(self.next_device_id.fetch_add(1, Ordering::Relaxed)),
            name,
            class,
            device_type: DeviceType::Platform,
            vendor_id: 0, // Platform devices don't have vendor/product IDs
            product_id: 0,
            capabilities: vec![], // Would be enhanced with actual capability detection
            compatible,
            dt_path: Some(path.to_string()),
            resources,
            status: DeviceStatus::Present,
        };

        Ok(Some(device_desc))
    }

    /// Discover VirtIO devices
    fn discover_virtio_devices(
        &mut self,
        fdt: &FlattenedDeviceTree,
        discovered_devices: &mut Vec<DeviceId>,
    ) -> Result<(), &'static str> {
        // Look for VirtIO devices in device tree
        let mut virtio_device_count = 0;

        // Standard VirtIO MMIO region starts at 0x01001000
        let virtio_base = 0x01001000;
        let virtio_size = 0x1000;
        let max_virtio_devices = 32;

        for i in 0..max_virtio_devices {
            let virtio_addr = virtio_base + (i * virtio_size);

            // Check if VirtIO device is present by reading magic value
            let magic_value = unsafe {
                core::ptr::read_volatile(virtio_addr as *const u32)
            };

            // VirtIO magic value: 0x74726976 ("vir" in little endian)
            if magic_value == 0x74726976 {
                // Read device ID
                let device_id_value = unsafe {
                    core::ptr::read_volatile((virtio_addr + 4) as *const u32)
                };

                if let Some(virtio_type) = self.virtio_device_id_to_type(device_id_value) {
                    let device_desc = self.create_virtio_device_desc(virtio_addr, virtio_type, i)?;
                    let device_id = device_desc.device_id;

                    self.devices.insert(device_id, device_desc);
                    discovered_devices.push(device_id);

                    virtio_device_count += 1;
                    self.stats.total_devices.fetch_add(1, Ordering::Relaxed);
                    *self.stats.device_types.entry(DeviceType::Virtio)
                        .or_insert_with(|| AtomicU32::new(0))
                        .fetch_add(1, Ordering::Relaxed);
                }
            }
        }

        Ok(())
    }

    /// Convert VirtIO device ID to device type
    fn virtio_device_id_to_type(&self, device_id: u32) -> Option<DeviceType> {
        match device_id {
            1 => Some(DeviceType::Network),
            2 => Some(DeviceType::Block),
            3 => Some(DeviceType::Console),
            4 => Some(DeviceType::Rng),
            5 => Some(DeviceType::Graphics),
            6 => Some(DeviceType::Input),
            _ => None,
        }
    }

    /// Create VirtIO device descriptor
    fn create_virtio_device_desc(
        &self,
        mmio_base: u64,
        device_type: DeviceType,
        instance: u32,
    ) -> Result<VirtualDeviceDesc, &'static str> {
        // Read VirtIO device features
        let features = unsafe {
            core::ptr::read_volatile((mmio_base + 0x010) as *const u32)
        };

        // Determine device class
        let class = self.device_type_to_class(device_type)?;

        // Create device name
        let name = match device_type {
            DeviceType::Network => format!("virtio-net{}", instance),
            DeviceType::Block => format!("virtio-blk{}", instance),
            DeviceType::Console => format!("virtio-console{}", instance),
            DeviceType::Rng => format!("virtio-rng{}", instance),
            DeviceType::Graphics => format!("virtio-gpu{}", instance),
            DeviceType::Input => format!("virtio-input{}", instance),
            _ => format!("virtio-unknown{}", instance),
        };

        // Parse VirtIO capabilities
        let capabilities = self.parse_virtio_capabilities(features, device_type);

        // Create VirtIO device resources
        let resources = DeviceResources {
            memory_regions: vec![MemoryRegion {
                name: "mmio".to_string(),
                base: Some(PhysAddr::from(mmio_base)),
                size: 0x1000,
                alignment: 0x1000,
                mem_type: MemoryType::Device,
                flags: MemoryFlags {
                    readable: true,
                    writable: true,
                    executable: false,
                    cacheable: false,
                    shareable: false,
                },
            }],
            irqs: vec![IrqResource {
                name: "device".to_string(),
                irq_type: IrqType::Legacy,
                irq_num: Some(32 + instance), // VirtIO IRQs typically start at 32
                trigger: IrqTrigger::EdgeRising,
                priority: 1,
                affinity: u64::MAX, // Any CPU
            }],
            ..Default::default()
        };

        Ok(VirtualDeviceDesc {
            device_id: DeviceId::from(self.next_device_id.fetch_add(1, Ordering::Relaxed)),
            name,
            class,
            device_type,
            vendor_id: 0x1AF4, // VirtIO vendor ID
            product_id: device_type.to_virtio_id(),
            capabilities,
            compatible: vec![format!("virtio,{}", device_type.to_string())],
            dt_path: None,
            resources,
            status: DeviceStatus::Present,
        })
    }

    /// Parse VirtIO device capabilities
    fn parse_virtio_capabilities(&self, features: u32, device_type: DeviceType) -> Vec<DeviceCapability> {
        let mut capabilities = Vec::new();

        // Common VirtIO capabilities
        if features & 0x1 != 0 {
            capabilities.push(DeviceCapability {
                name: "virtio-1.0".to_string(),
                version: 1,
                cap_type: CapabilityType::Boolean,
                value: 1,
                properties: BTreeMap::new(),
            });
        }

        // Device-specific capabilities
        match device_type {
            DeviceType::Network => {
                if features & (1 << 5) != 0 {
                    capabilities.push(DeviceCapability {
                        name: "mac".to_string(),
                        version: 1,
                        cap_type: CapabilityType::Boolean,
                        value: 1,
                        properties: BTreeMap::new(),
                    });
                }
                if features & (1 << 6) != 0 {
                    capabilities.push(DeviceCapability {
                        name: "status".to_string(),
                        version: 1,
                        cap_type: CapabilityType::Boolean,
                        value: 1,
                        properties: BTreeMap::new(),
                    });
                }
            }
            DeviceType::Block => {
                if features & (1 << 1) != 0 {
                    capabilities.push(DeviceCapability {
                        name: "blk-size".to_string(),
                        version: 1,
                        cap_type: CapabilityType::Integer,
                        value: 512,
                        properties: BTreeMap::new(),
                    });
                }
                if features & (1 << 5) != 0 {
                    capabilities.push(DeviceCapability {
                        name: "flush".to_string(),
                        version: 1,
                        cap_type: CapabilityType::Boolean,
                        value: 1,
                        properties: BTreeMap::new(),
                    });
                }
            }
            DeviceType::Console => {
                if features & (1 << 0) != 0 {
                    capabilities.push(DeviceCapability {
                        name: "size".to_string(),
                        version: 1,
                        cap_type: CapabilityType::Integer,
                        value: 16,
                        properties: BTreeMap::new(),
                    });
                }
                if features & (1 << 1) != 0 {
                    capabilities.push(DeviceCapability {
                        name: "mult".to_string(),
                        version: 1,
                        cap_type: CapabilityType::Integer,
                        value: 32,
                        properties: BTreeMap::new(),
                    });
                }
            }
            _ => {}
        }

        capabilities
    }

    /// Determine device class from compatible strings
    fn determine_device_class(&self, compatible: &[String]) -> Result<DeviceClass, &'static str> {
        // Check for specific compatible strings
        for compat_str in compatible {
            if compat_str.contains("virtio,net") {
                return Ok(self.classes.get(&1).unwrap().clone());
            } else if compat_str.contains("virtio,block") {
                return Ok(self.classes.get(&2).unwrap().clone());
            } else if compat_str.contains("virtio,console") || compat_str.contains("serial") {
                return Ok(self.classes.get(&3).unwrap().clone());
            } else if compat_str.contains("virtio,gpu") {
                return Ok(self.classes.get(&4).unwrap().clone());
            } else if compat_str.contains("virtio,input") {
                return Ok(self.classes.get(&5).unwrap().clone());
            } else if compat_str.contains("virtio,rng") {
                return Ok(self.classes.get(&6).unwrap().clone());
            }
        }

        // Default to base class
        Ok(self.classes.get(&0).unwrap().clone())
    }

    /// Map device type to device class
    fn device_type_to_class(&self, device_type: DeviceType) -> Result<DeviceClass, &'static str> {
        let class_id = match device_type {
            DeviceType::Network => 1,
            DeviceType::Block => 2,
            DeviceType::Console => 3,
            DeviceType::Graphics => 4,
            DeviceType::Input => 5,
            DeviceType::Rng => 6,
            _ => 0,
        };

        self.classes.get(&class_id)
            .cloned()
            .ok_or("Device class not found")
    }

    /// Parse device resources from device tree
    fn parse_device_resources(
        &self,
        fdt: &FlattenedDeviceTree,
        node: &crate::arch::riscv64::devtree::fdt::Node,
    ) -> Result<DeviceResources, &'static str> {
        let mut resources = DeviceResources::default();

        // Parse memory regions from 'reg' property
        if let Some(reg_prop) = node.get_property("reg") {
            let addr_cells = fdt.get_address_cells(node).unwrap_or(2);
            let size_cells = fdt.get_size_cells(node).unwrap_or(1);

            let reg_data = reg_prop.data;
            let entry_size = (addr_cells + size_cells) * 4;

            for chunk in reg_data.chunks_exact(entry_size) {
                if chunk.len() == entry_size {
                    let mut addr = 0u64;
                    let mut size = 0u64;

                    // Parse address
                    for i in 0..addr_cells {
                        addr |= (chunk[i * 4] as u64) << (8 * (addr_cells - 1 - i) * 4);
                    }

                    // Parse size
                    for i in 0..size_cells {
                        size |= (chunk[(addr_cells + i) * 4] as u64) << (8 * (size_cells - 1 - i) * 4);
                    }

                    if size > 0 {
                        resources.memory_regions.push(MemoryRegion {
                            name: "mmio".to_string(),
                            base: Some(PhysAddr::from(addr)),
                            size,
                            alignment: 0x1000,
                            mem_type: MemoryType::Device,
                            flags: MemoryFlags {
                                readable: true,
                                writable: true,
                                executable: false,
                                cacheable: false,
                                shareable: false,
                            },
                        });
                    }
                }
            }
        }

        // Parse interrupts from 'interrupts' property
        if let Some(interrupts_prop) = node.get_property("interrupts") {
            let interrupt_data = interrupts_prop.data;

            for chunk in interrupt_data.chunks_exact(4) {
                if chunk.len() == 4 {
                    let irq_num = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);

                    resources.irqs.push(IrqResource {
                        name: "interrupt".to_string(),
                        irq_type: IrqType::Legacy,
                        irq_num: Some(irq_num),
                        trigger: IrqTrigger::EdgeRising,
                        priority: 1,
                        affinity: u64::MAX,
                    });
                }
            }
        }

        Ok(resources)
    }

    /// Discover interrupt controllers
    fn discover_interrupt_controllers(
        &mut self,
        fdt: &FlattenedDeviceTree,
        discovered_devices: &mut Vec<DeviceId>,
    ) -> Result<(), &'static str> {
        // Look for common interrupt controllers
        let ic_paths = [
            "/interrupt-controller@0",
            "/interrupt-controller@8000000", // PLIC
            "/interrupt-controller@0c000000", // APLIC
            "/interrupt-controller@2000000", // IMSIC
        ];

        for path in &ic_paths {
            if let Some(node) = fdt.find_node(path) {
                let device_desc = self.create_interrupt_controller_desc(fdt, node, path)?;
                let device_id = device_desc.device_id;

                self.devices.insert(device_id, device_desc);
                discovered_devices.push(device_id);

                self.stats.total_devices.fetch_add(1, Ordering::Relaxed);
                *self.stats.device_types.entry(DeviceType::InterruptController)
                    .or_insert_with(|| AtomicU32::new(0))
                    .fetch_add(1, Ordering::Relaxed);
            }
        }

        Ok(())
    }

    /// Create interrupt controller device descriptor
    fn create_interrupt_controller_desc(
        &self,
        fdt: &FlattenedDeviceTree,
        node: &crate::arch::riscv64::devtree::fdt::Node,
        path: &str,
    ) -> Result<VirtualDeviceDesc, &'static str> {
        // Get compatible string
        let compatible = if let Some(prop) = node.get_property("compatible") {
            let compat_str = core::str::from_utf8(prop.data).unwrap_or("");
            compat_str.split('\0')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect()
        } else {
            vec!["riscv,cpu-intc".to_string()]
        };

        let name = path.split('/').last()
            .unwrap_or("interrupt-controller")
            .to_string();

        // Parse resources
        let resources = self.parse_device_resources(fdt, node)?;

        Ok(VirtualDeviceDesc {
            device_id: DeviceId::from(self.next_device_id.fetch_add(1, Ordering::Relaxed)),
            name,
            class: self.classes.get(&0).unwrap().clone(),
            device_type: DeviceType::InterruptController,
            vendor_id: 0,
            product_id: 0,
            capabilities: vec![],
            compatible,
            dt_path: Some(path.to_string()),
            resources,
            status: DeviceStatus::Present,
        })
    }

    /// Discover CPU devices
    fn discover_cpu_devices(
        &mut self,
        fdt: &FlattenedDeviceTree,
        discovered_devices: &mut Vec<DeviceId>,
    ) -> Result<(), &'static str> {
        // Look for CPU nodes
        if let Some(cpus_node) = fdt.find_node("/cpus") {
            // Iterate through CPU nodes
            for child in &cpus_node.children {
                if child.name.starts_with("cpu@") {
                    let device_desc = self.create_cpu_device_desc(fdt, child)?;
                    let device_id = device_desc.device_id;

                    self.devices.insert(device_id, device_desc);
                    discovered_devices.push(device_id);

                    self.stats.total_devices.fetch_add(1, Ordering::Relaxed);
                    *self.stats.device_types.entry(DeviceType::Cpu)
                        .or_insert_with(|| AtomicU32::new(0))
                        .fetch_add(1, Ordering::Relaxed);
                }
            }
        }

        Ok(())
    }

    /// Create CPU device descriptor
    fn create_cpu_device_desc(
        &self,
        fdt: &FlattenedDeviceTree,
        node: &crate::arch::riscv64::devtree::fdt::Node,
    ) -> Result<VirtualDeviceDesc, &'static str> {
        let name = node.name.clone();

        // Get CPU capabilities
        let mut capabilities = Vec::new();

        // Check for ISA extensions
        if let Some(isa_prop) = node.get_property("riscv,isa") {
            let isa_str = core::str::from_utf8(isa_prop.data).unwrap_or("");

            // Parse ISA string for extensions
            if isa_str.contains('i') {
                capabilities.push(DeviceCapability {
                    name: "rv32i".to_string(),
                    version: 1,
                    cap_type: CapabilityType::Boolean,
                    value: 1,
                    properties: BTreeMap::new(),
                });
            }
            if isa_str.contains('m') {
                capabilities.push(DeviceCapability {
                    name: "rv32m".to_string(),
                    version: 1,
                    cap_type: CapabilityType::Boolean,
                    value: 1,
                    properties: BTreeMap::new(),
                });
            }
            if isa_str.contains('a') {
                capabilities.push(DeviceCapability {
                    name: "rv32a".to_string(),
                    version: 1,
                    cap_type: CapabilityType::Boolean,
                    value: 1,
                    properties: BTreeMap::new(),
                });
            }
            if isa_str.contains('f') {
                capabilities.push(DeviceCapability {
                    name: "rv32f".to_string(),
                    version: 1,
                    cap_type: CapabilityType::Boolean,
                    value: 1,
                    properties: BTreeMap::new(),
                });
            }
            if isa_str.contains('d') {
                capabilities.push(DeviceCapability {
                    name: "rv32d".to_string(),
                    version: 1,
                    cap_type: CapabilityType::Boolean,
                    value: 1,
                    properties: BTreeMap::new(),
                });
            }
            if isa_str.contains('c') {
                capabilities.push(DeviceCapability {
                    name: "rvc".to_string(),
                    version: 1,
                    cap_type: CapabilityType::Boolean,
                    value: 1,
                    properties: BTreeMap::new(),
                });
            }
        }

        // Check for H-extension (virtualization support)
        if let Some(isa_prop) = node.get_property("riscv,isa") {
            let isa_str = core::str::from_utf8(isa_prop.data).unwrap_or("");
            if isa_str.contains('h') {
                capabilities.push(DeviceCapability {
                    name: "h".to_string(),
                    version: 1,
                    cap_type: CapabilityType::Boolean,
                    value: 1,
                    properties: BTreeMap::new(),
                });
            }
        }

        let compatible = vec!["riscv,cpu".to_string()];

        Ok(VirtualDeviceDesc {
            device_id: DeviceId::from(self.next_device_id.fetch_add(1, Ordering::Relaxed)),
            name,
            class: self.classes.get(&0).unwrap().clone(),
            device_type: DeviceType::Cpu,
            vendor_id: 0,
            product_id: 0,
            capabilities,
            compatible,
            dt_path: None,
            resources: DeviceResources::default(),
            status: DeviceStatus::Present,
        })
    }

    /// Get device by ID
    pub fn get_device(&self, device_id: DeviceId) -> Option<&VirtualDeviceDesc> {
        self.devices.get(&device_id)
    }

    /// Get all devices of a specific type
    pub fn get_devices_by_type(&self, device_type: DeviceType) -> Vec<&VirtualDeviceDesc> {
        self.devices.values()
            .filter(|device| device.device_type == device_type)
            .collect()
    }

    /// Get all devices of a specific class
    pub fn get_devices_by_class(&self, class_id: u32) -> Vec<&VirtualDeviceDesc> {
        self.devices.values()
            .filter(|device| device.class.id == class_id)
            .collect()
    }

    /// Find devices by compatible string
    pub fn find_devices_by_compatible(&self, compatible: &str) -> Vec<&VirtualDeviceDesc> {
        self.devices.values()
            .filter(|device| device.compatible.iter().any(|c| c.contains(compatible)))
            .collect()
    }

    /// Get discovery statistics
    pub fn get_stats(&self) -> &DiscoveryStats {
        &self.stats
    }

    /// Process hotplug events
    pub fn process_hotplug_events(&mut self) -> Result<Vec<DeviceId>, &'static str> {
        let mut new_devices = Vec::new();

        while let Some(event) = self.hotplug_queue.pop_front() {
            match event {
                HotplugEvent::DeviceAdd(device_desc) => {
                    let device_id = device_desc.device_id;
                    self.devices.insert(device_id, device_desc);
                    new_devices.push(device_id);

                    self.stats.hotplug_events.fetch_add(1, Ordering::Relaxed);
                }
                HotplugEvent::DeviceRemove(device_id) => {
                    self.devices.remove(&device_id);
                    self.stats.hotplug_events.fetch_add(1, Ordering::Relaxed);
                }
                _ => {}
            }
        }

        Ok(new_devices)
    }

    /// Add hotplug event
    pub fn add_hotplug_event(&mut self, event: HotplugEvent) {
        self.hotplug_queue.push_back(event);
    }
}

/// Hotplug event types
#[derive(Debug)]
pub enum HotplugEvent {
    /// Device added
    DeviceAdd(VirtualDeviceDesc),
    /// Device removed
    DeviceRemove(DeviceId),
    /// Device updated
    DeviceUpdate(DeviceId),
    /// Device status changed
    DeviceStatusChange(DeviceId, DeviceStatus),
}

/// Extensions for DeviceType
pub trait DeviceTypeExt {
    /// Convert to VirtIO device ID
    fn to_virtio_id(&self) -> u32;

    /// Convert to string
    fn to_string(&self) -> &'static str;
}

impl DeviceTypeExt for DeviceType {
    fn to_virtio_id(&self) -> u32 {
        match self {
            DeviceType::Network => 1,
            DeviceType::Block => 2,
            DeviceType::Console => 3,
            DeviceType::Rng => 4,
            DeviceType::Graphics => 16,
            DeviceType::Input => 18,
            _ => 0,
        }
    }

    fn to_string(&self) -> &'static str {
        match self {
            DeviceType::Network => "net",
            DeviceType::Block => "block",
            DeviceType::Console => "console",
            DeviceType::Rng => "rng",
            DeviceType::Graphics => "gpu",
            DeviceType::Input => "input",
            DeviceType::Cpu => "cpu",
            DeviceType::Platform => "platform",
            DeviceType::InterruptController => "interrupt-controller",
        }
    }
}

impl Default for MemoryFlags {
    fn default() -> Self {
        Self {
            readable: true,
            writable: false,
            executable: false,
            cacheable: true,
            shareable: false,
        }
    }
}

impl core::fmt::Display for DeviceCapability {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} v{} ({}): {}",
               self.name,
               self.version,
               match self.cap_type {
                   CapabilityType::Boolean => "bool",
                   CapabilityType::Integer => "int",
                   CapabilityType::String => "str",
                   CapabilityType::Range(_, _) => "range",
                   CapabilityType::List => "list",
               },
               self.value)
    }
}

impl core::fmt::Display for VirtualDeviceDesc {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} ({}): class={}, type={}, vendor={:04x}, device={:04x}, caps={}",
               self.name,
               self.device_id,
               self.class.name,
               self.device_type.to_string(),
               self.vendor_id,
               self.product_id,
               self.capabilities.len())
    }
}