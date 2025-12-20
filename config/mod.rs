//! Configuration management
//!
//! This module handles configuration for the hypervisor,
//! including VM configurations and runtime settings.

use crate::{Error, Result};

/// VM configuration structure
#[derive(Debug, Clone)]
pub struct VmConfig {
    /// VM name
    pub name: String,
    /// Number of VCPUs
    pub vcpu_count: usize,
    /// Memory size in bytes
    pub memory_size: u64,
    /// Kernel image path
    pub kernel_path: Option<String>,
    /// Initrd image path
    pub initrd_path: Option<String>,
    /// Kernel command line
    pub cmdline: Option<String>,
    /// List of device configurations
    pub devices: Vec<DeviceConfig>,
}

/// Device configuration structure
#[derive(Debug, Clone)]
pub struct DeviceConfig {
    /// Device type
    pub device_type: DeviceType,
    /// Device name
    pub name: String,
    /// Base address
    pub base_address: Option<u64>,
    /// Size
    pub size: Option<u64>,
    /// IRQ number
    pub irq: Option<u32>,
    /// Device-specific parameters
    pub params: std::collections::HashMap<String, String>,
}

/// Device types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceType {
    /// UART device
    Uart,
    /// RTC device
    Rtc,
    /// GPIO device
    Gpio,
    /// VirtIO block device
    VirtioBlk,
    /// VirtIO network device
    VirtioNet,
    /// VirtIO console device
    VirtioConsole,
    /// PCI device
    Pci,
    /// Platform device
    Platform,
    /// Custom device
    Custom(String),
}

/// Global hypervisor configuration
#[derive(Debug, Clone)]
pub struct HypervisorConfig {
    /// Maximum number of VMs
    pub max_vms: usize,
    /// Maximum number of VCPUs
    pub max_vcpus: usize,
    /// Default memory size per VM
    pub default_memory_size: u64,
    /// Enable debug output
    pub debug_enabled: bool,
    /// Enable verbose logging
    pub verbose_enabled: bool,
    /// Timer frequency in Hz
    pub timer_freq: u64,
}

impl Default for HypervisorConfig {
    fn default() -> Self {
        Self {
            max_vms: 4,
            max_vcpus: 8,
            default_memory_size: 512 * 1024 * 1024, // 512MB
            debug_enabled: cfg!(feature = "debug"),
            verbose_enabled: cfg!(feature = "verbose"),
            timer_freq: 1000,
        }
    }
}

/// Load configuration from TOML file
pub fn load_config(path: &str) -> Result<HypervisorConfig> {
    // TODO: Implement TOML configuration loading
    // For now, return default configuration
    Ok(HypervisorConfig::default())
}

/// Save configuration to TOML file
pub fn save_config(config: &HypervisorConfig, path: &str) -> Result<()> {
    // TODO: Implement TOML configuration saving
    Err(Error::NotImplemented)
}

/// Load VM configuration
pub fn load_vm_config(path: &str) -> Result<VmConfig> {
    // TODO: Implement VM configuration loading
    Err(Error::NotImplemented)
}

/// Save VM configuration
pub fn save_vm_config(config: &VmConfig, path: &str) -> Result<()> {
    // TODO: Implement VM configuration saving
    Err(Error::NotImplemented)
}

/// Get hypervisor configuration
pub fn get_hypervisor_config() -> &'static HypervisorConfig {
    // TODO: Return actual configuration
    &HypervisorConfig::default()
}

/// Set hypervisor configuration
pub fn set_hypervisor_config(config: HypervisorConfig) -> Result<()> {
    // TODO: Set configuration
    Err(Error::NotImplemented)
}

/// Validate VM configuration
pub fn validate_vm_config(config: &VmConfig) -> Result<()> {
    if config.name.is_empty() {
        return Err(Error::InvalidArgument);
    }

    if config.vcpu_count == 0 || config.vcpu_count > 8 {
        return Err(Error::InvalidArgument);
    }

    if config.memory_size == 0 {
        return Err(Error::InvalidArgument);
    }

    Ok(())
}