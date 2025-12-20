//! RISC-V Platform Configuration
//!
//! This module provides platform configuration management including:
//! - Platform-specific settings
//! - Configuration validation
//! - Dynamic configuration updates
//! - Configuration persistence

use crate::arch::riscv64::*;
use super::PlatformInfo;

/// Platform configuration
#[derive(Debug, Clone)]
pub struct PlatformConfig {
    /// Platform type
    pub platform_type: super::PlatformType,
    /// CPU configuration
    pub cpu_count: u32,
    /// Memory configuration
    pub memory_size: u64,
    /// UART base address
    pub uart_base: u64,
    /// CLINT base address
    pub clint_base: u64,
    /// PLIC base address
    pub plic_base: u64,
    /// Timer frequency
    pub timer_freq: u64,
    /// Enable virtualization
    pub enable_virtualization: bool,
    /// Enable SMP
    pub enable_smp: bool,
    /// Enable debug
    pub enable_debug: bool,
    /// Boot configuration
    pub boot_config: BootConfig,
}

impl PlatformConfig {
    /// Create new platform configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Create platform configuration from platform info
    pub fn from_platform_info(info: &PlatformInfo) -> Self {
        Self {
            platform_type: info.platform_type,
            cpu_count: info.cpu_count,
            memory_size: info.memory_size,
            uart_base: info.uart_base,
            clint_base: info.clint_base,
            plic_base: info.plic_base,
            timer_freq: info.timer_freq,
            enable_virtualization: true,
            enable_smp: info.cpu_count > 1,
            enable_debug: true,
            boot_config: BootConfig::default(),
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        // Validate CPU count
        if self.cpu_count == 0 || self.cpu_count > MAX_CPUS as u32 {
            return Err(format!("Invalid CPU count: {}", self.cpu_count));
        }

        // Validate memory size
        if self.memory_size == 0 || self.memory_size > 0x1000000000 {
            return Err(format!("Invalid memory size: {:#x}", self.memory_size));
        }

        // Validate base addresses
        if self.uart_base == 0 || (self.uart_base & 0xFFF) != 0 {
            return Err(format!("Invalid UART base address: {:#x}", self.uart_base));
        }

        if self.clint_base == 0 || (self.clint_base & 0xFFF) != 0 {
            return Err(format!("Invalid CLINT base address: {:#x}", self.clint_base));
        }

        if self.plic_base == 0 || (self.plic_base & 0xFFF) != 0 {
            return Err(format!("Invalid PLIC base address: {:#x}", self.plic_base));
        }

        // Validate timer frequency
        if self.timer_freq == 0 || self.timer_freq > 0xFFFFFFFF {
            return Err(format!("Invalid timer frequency: {}", self.timer_freq));
        }

        // Validate boot configuration
        self.boot_config.validate()?;

        Ok(())
    }

    /// Get platform-specific defaults
    pub fn get_platform_defaults(platform_type: super::PlatformType) -> Self {
        match platform_type {
            super::PlatformType::QemuVirt => Self {
                platform_type,
                cpu_count: 4,
                memory_size: 0x80000000, // 2GB
                uart_base: 0x10000000,
                clint_base: 0x02000000,
                plic_base: 0x0c000000,
                timer_freq: 10000000, // 10MHz
                enable_virtualization: true,
                enable_smp: true,
                enable_debug: true,
                boot_config: BootConfig::default(),
            },
            super::PlatformType::SiFiveUnleashed => Self {
                platform_type,
                cpu_count: 5,
                memory_size: 0x80000000, // 2GB
                uart_base: 0x10010000,
                clint_base: 0x02000000,
                plic_base: 0x0c000000,
                timer_freq: 1000000, // 1MHz
                enable_virtualization: true,
                enable_smp: true,
                enable_debug: true,
                boot_config: BootConfig::default(),
            },
            super::PlatformType::AllwinnerD1 => Self {
                platform_type,
                cpu_count: 1,
                memory_size: 0x40000000, // 1GB
                uart_base: 0x02500000,
                clint_base: 0x04000000,
                plic_base: 0x10000000,
                timer_freq: 24000000, // 24MHz
                enable_virtualization: false,
                enable_smp: false,
                enable_debug: true,
                boot_config: BootConfig::default(),
            },
            super::PlatformType::Custom => Self::default(),
        }
    }
}

impl Default for PlatformConfig {
    fn default() -> Self {
        Self {
            platform_type: super::PlatformType::QemuVirt,
            cpu_count: 4,
            memory_size: 0x80000000, // 2GB
            uart_base: 0x10000000,
            clint_base: 0x02000000,
            plic_base: 0x0c000000,
            timer_freq: 10000000, // 10MHz
            enable_virtualization: true,
            enable_smp: true,
            enable_debug: true,
            boot_config: BootConfig::default(),
        }
    }
}

/// Boot configuration
#[derive(Debug, Clone)]
pub struct BootConfig {
    /// Kernel load address
    pub kernel_address: u64,
    /// Device tree address
    pub dtb_address: u64,
    /// Initial stack address
    pub stack_address: u64,
    /// Boot arguments
    pub boot_args: u64,
    /// Enable early console
    pub early_console: bool,
    /// Boot delay in milliseconds
    pub boot_delay_ms: u32,
}

impl BootConfig {
    /// Create new boot configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate boot configuration
    pub fn validate(&self) -> Result<(), String> {
        // Validate addresses
        if self.kernel_address == 0 || (self.kernel_address & 0x3) != 0 {
            return Err(format!("Invalid kernel address: {:#x}", self.kernel_address));
        }

        if self.dtb_address == 0 || (self.dtb_address & 0x7) != 0 {
            return Err(format!("Invalid DTB address: {:#x}", self.dtb_address));
        }

        if self.stack_address == 0 || (self.stack_address & 0xF) != 0 {
            return Err(format!("Invalid stack address: {:#x}", self.stack_address));
        }

        // Validate boot delay
        if self.boot_delay_ms > 10000 {
            return Err(format!("Boot delay too long: {}ms", self.boot_delay_ms));
        }

        Ok(())
    }
}

impl Default for BootConfig {
    fn default() -> Self {
        Self {
            kernel_address: 0x80200000,
            dtb_address: 0x82000000,
            stack_address: 0x80000000,
            boot_args: 0,
            early_console: true,
            boot_delay_ms: 0,
        }
    }
}

/// Runtime configuration manager
pub struct ConfigManager {
    /// Current configuration
    current: PlatformConfig,
    /// Default configuration
    default: PlatformConfig,
    /// Configuration changed flag
    changed: bool,
}

impl ConfigManager {
    /// Create new configuration manager
    pub fn new() -> Self {
        let default = PlatformConfig::default();
        Self {
            current: default.clone(),
            default,
            changed: false,
        }
    }

    /// Create configuration manager with initial config
    pub fn with_config(config: PlatformConfig) -> Self {
        Self {
            default: config.clone(),
            current: config,
            changed: false,
        }
    }

    /// Get current configuration
    pub fn get_config(&self) -> &PlatformConfig {
        &self.current
    }

    /// Get default configuration
    pub fn get_default(&self) -> &PlatformConfig {
        &self.default
    }

    /// Update configuration
    pub fn update_config(&mut self, config: PlatformConfig) -> Result<(), String> {
        // Validate new configuration
        config.validate()?;

        self.current = config;
        self.changed = true;

        Ok(())
    }

    /// Update specific field
    pub fn update_field<F>(&mut self, updater: F) -> Result<(), String>
    where
        F: FnOnce(&mut PlatformConfig),
    {
        let mut new_config = self.current.clone();
        updater(&mut new_config);

        // Validate new configuration
        new_config.validate()?;

        self.current = new_config;
        self.changed = true;

        Ok(())
    }

    /// Reset to default configuration
    pub fn reset_to_default(&mut self) {
        self.current = self.default.clone();
        self.changed = true;
    }

    /// Check if configuration has changed
    pub fn has_changed(&self) -> bool {
        self.changed
    }

    /// Clear changed flag
    pub fn clear_changed(&mut self) {
        self.changed = false;
    }

    /// Save configuration to persistent storage
    pub fn save(&self) -> Result<(), &'static str> {
        // TODO: Implement configuration persistence
        log::debug!("Saving platform configuration");
        Ok(())
    }

    /// Load configuration from persistent storage
    pub fn load(&mut self) -> Result<(), &'static str> {
        // TODO: Implement configuration loading
        log::debug!("Loading platform configuration");
        Ok(())
    }
}

/// Global configuration manager
static mut CONFIG_MANAGER: Option<ConfigManager> = None;
static CONFIG_MANAGER_INIT: spin::Once<()> = spin::Once::new();

/// Initialize configuration manager
pub fn init() -> Result<(), &'static str> {
    CONFIG_MANAGER_INIT.call_once(|| {
        let manager = ConfigManager::new();
        unsafe {
            CONFIG_MANAGER = Some(manager);
        }
    });

    log::debug!("Platform configuration manager initialized");
    Ok(())
}

/// Get configuration manager
pub fn get_manager() -> Option<&'static mut ConfigManager> {
    unsafe { CONFIG_MANAGER.as_mut() }
}

/// Get current configuration
pub fn get_current_config() -> Option<&'static PlatformConfig> {
    unsafe { CONFIG_MANAGER.as_ref().map(|m| m.get_config()) }
}

/// Update configuration
pub fn update_config(config: PlatformConfig) -> Result<(), String> {
    if let Some(manager) = get_manager() {
        manager.update_config(config)
    } else {
        Err("Configuration manager not initialized".to_string())
    }
}

/// Reset configuration to defaults
pub fn reset_to_defaults() -> Result<(), &'static str> {
    if let Some(manager) = get_manager() {
        manager.reset_to_default();
        Ok(())
    } else {
        Err("Configuration manager not initialized")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_config() {
        let config = PlatformConfig::default();
        assert_eq!(config.platform_type, super::super::PlatformType::QemuVirt);
        assert_eq!(config.cpu_count, 4);
        assert_eq!(config.memory_size, 0x80000000);
        assert!(config.enable_virtualization);
        assert!(config.enable_smp);
        assert!(config.enable_debug);
    }

    #[test]
    fn test_boot_config() {
        let config = BootConfig::default();
        assert_eq!(config.kernel_address, 0x80200000);
        assert_eq!(config.dtb_address, 0x82000000);
        assert_eq!(config.stack_address, 0x80000000);
        assert!(config.early_console);
        assert_eq!(config.boot_delay_ms, 0);
    }

    #[test]
    fn test_config_manager() {
        let mut manager = ConfigManager::new();

        // Test initial state
        assert!(!manager.has_changed());
        assert_eq!(manager.get_config().cpu_count, 4);

        // Test update
        let mut new_config = manager.get_config().clone();
        new_config.cpu_count = 8;
        manager.update_config(new_config).unwrap();

        assert!(manager.has_changed());
        assert_eq!(manager.get_config().cpu_count, 8);

        // Test reset
        manager.reset_to_default();
        assert!(manager.has_changed());
        assert_eq!(manager.get_config().cpu_count, 4);
    }

    #[test]
    fn test_platform_defaults() {
        let qemu_config = PlatformConfig::get_platform_defaults(super::super::PlatformType::QemuVirt);
        assert_eq!(qemu_config.platform_type, super::super::PlatformType::QemuVirt);
        assert_eq!(qemu_config.cpu_count, 4);
        assert_eq!(qemu_config.uart_base, 0x10000000);

        let sifive_config = PlatformConfig::get_platform_defaults(super::super::PlatformType::SiFiveUnleashed);
        assert_eq!(sifive_config.platform_type, super::super::PlatformType::SiFiveUnleashed);
        assert_eq!(sifive_config.cpu_count, 5);
        assert_eq!(sifive_config.uart_base, 0x10010000);
    }

    #[test]
    fn test_config_validation() {
        let mut config = PlatformConfig::default();

        // Valid configuration
        assert!(config.validate().is_ok());

        // Invalid CPU count
        config.cpu_count = 0;
        assert!(config.validate().is_err());
        config.cpu_count = 4; // Reset

        // Invalid memory size
        config.memory_size = 0;
        assert!(config.validate().is_err());
        config.memory_size = 0x80000000; // Reset

        // Invalid UART base
        config.uart_base = 0x10000001; // Not aligned
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_boot_config_validation() {
        let mut config = BootConfig::default();

        // Valid configuration
        assert!(config.validate().is_ok());

        // Invalid kernel address
        config.kernel_address = 0x80200001; // Not aligned
        assert!(config.validate().is_err());
        config.kernel_address = 0x80200000; // Reset

        // Invalid DTB address
        config.dtb_address = 0x82000001; // Not aligned
        assert!(config.validate().is_err());
        config.dtb_address = 0x82000000; // Reset

        // Invalid stack address
        config.stack_address = 0x80000001; // Not aligned
        assert!(config.validate().is_err());
        config.stack_address = 0x80000000; // Reset

        // Invalid boot delay
        config.boot_delay_ms = 20000; // Too long
        assert!(config.validate().is_err());
    }
}