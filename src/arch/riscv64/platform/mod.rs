//! RISC-V Platform Support
//!
//! This module provides platform-specific support for RISC-V including:
//! - Platform detection and initialization
//! - Platform-specific configurations
//! - Board support packages (BSPs)
//! - Hardware abstraction layer
//! - Platform resource management

pub mod config;
pub mod memory;
pub mod timer;
pub mod uart;
pub mod clint;
pub mod plic;

use crate::arch::riscv64::*;
use config::PlatformConfig;

/// Platform type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformType {
    /// QEMU Virt platform
    QemuVirt,
    /// SiFive HiFive Unleashed
    SiFiveUnleashed,
    /// Allwinner D1
    AllwinnerD1,
    /// Custom platform
    Custom,
}

/// Platform information
#[derive(Debug, Clone)]
pub struct PlatformInfo {
    /// Platform type
    pub platform_type: PlatformType,
    /// Platform name
    pub name: String,
    /// Platform version
    pub version: String,
    /// CPU count
    pub cpu_count: u32,
    /// Memory size
    pub memory_size: u64,
    /// UART base address
    pub uart_base: u64,
    /// CLINT base address
    pub clint_base: u64,
    /// PLIC base address
    pub plic_base: u64,
    /// Timer frequency
    pub timer_freq: u64,
}

impl Default for PlatformInfo {
    fn default() -> Self {
        Self {
            platform_type: PlatformType::QemuVirt,
            name: "QEMU Virt".to_string(),
            version: "1.0".to_string(),
            cpu_count: 4,
            memory_size: 0x80000000, // 2GB
            uart_base: 0x10000000,
            clint_base: 0x02000000,
            plic_base: 0x0c000000,
            timer_freq: 10000000, // 10MHz
        }
    }
}

/// Platform-specific configurations
#[derive(Debug, Clone)]
pub struct PlatformConfigurations {
    /// Memory configuration
    pub memory: memory::MemoryConfig,
    /// Timer configuration
    pub timer: timer::TimerConfig,
    /// UART configuration
    pub uart: uart::UartConfig,
    /// CLINT configuration
    pub clint: clint::ClintConfig,
    /// PLIC configuration
    pub plic: plic::PlicConfig,
}

impl Default for PlatformConfigurations {
    fn default() -> Self {
        Self {
            memory: memory::MemoryConfig::default(),
            timer: timer::TimerConfig::default(),
            uart: uart::UartConfig::default(),
            clint: clint::ClintConfig::default(),
            plic: plic::PlicConfig::default(),
        }
    }
}

/// Global platform information
static mut PLATFORM_INFO: Option<PlatformInfo> = None;
static mut PLATFORM_CONFIG: Option<PlatformConfig> = None;
static mut PLATFORM_CONFIGURATIONS: Option<PlatformConfigurations> = None;

/// Initialize platform
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing RISC-V platform support");

    // Detect platform from device tree if available
    let platform_info = detect_platform().unwrap_or_else(|_| PlatformInfo::default());
    log::info!("Detected platform: {}", platform_info.name);

    // Store platform information
    unsafe {
        PLATFORM_INFO = Some(platform_info.clone());
    }

    // Initialize platform-specific configurations
    let platform_config = PlatformConfig::from_platform_info(&platform_info);
    unsafe {
        PLATFORM_CONFIG = Some(platform_config);
    }

    // Initialize sub-configurations
    let platform_configurations = PlatformConfigurations::default();
    unsafe {
        PLATFORM_CONFIGURATIONS = Some(platform_configurations);
    }

    log::info!("RISC-V platform support initialized");
    Ok(())
}

/// Detect platform from device tree or hardware
pub fn detect_platform() -> Result<PlatformInfo, &'static str> {
    // Try to detect from device tree first
    if let Some(_compatible) = crate::arch::riscv64::devtree::find_compatible("qemu,riscv-virt") {
        return Ok(PlatformInfo {
            platform_type: PlatformType::QemuVirt,
            name: "QEMU Virt".to_string(),
            version: "1.0".to_string(),
            cpu_count: crate::arch::riscv64::devtree::get_cpu_info().len() as u32,
            memory_size: {
                let regions = crate::arch::riscv64::devtree::get_memory_info();
                regions.iter().map(|r| r.size).sum()
            },
            uart_base: 0x10000000,
            clint_base: 0x02000000,
            plic_base: 0x0c000000,
            timer_freq: 10000000,
        });
    }

    // Default to QEMU Virt
    Err("Unable to detect platform, using default")
}

/// Get platform information
pub fn get_platform_info() -> Option<&'static PlatformInfo> {
    unsafe { PLATFORM_INFO.as_ref() }
}

/// Get platform configuration
pub fn get_platform_config() -> Option<&'static PlatformConfig> {
    unsafe { PLATFORM_CONFIG.as_ref() }
}

/// Get platform-specific configurations
pub fn get_platform_configurations() -> Option<&'static PlatformConfigurations> {
    unsafe { PLATFORM_CONFIGURATIONS.as_ref() }
}

/// Get platform type
pub fn get_platform_type() -> PlatformType {
    get_platform_info()
        .map(|info| info.platform_type)
        .unwrap_or(PlatformType::QemuVirt)
}

/// Get CPU count
pub fn get_cpu_count() -> u32 {
    get_platform_info()
        .map(|info| info.cpu_count)
        .unwrap_or(1)
}

/// Get memory size
pub fn get_memory_size() -> u64 {
    get_platform_info()
        .map(|info| info.memory_size)
        .unwrap_or(0)
}

/// Get UART base address
pub fn get_uart_base() -> u64 {
    get_platform_info()
        .map(|info| info.uart_base)
        .unwrap_or(0x10000000)
}

/// Get CLINT base address
pub fn get_clint_base() -> u64 {
    get_platform_info()
        .map(|info| info.clint_base)
        .unwrap_or(0x02000000)
}

/// Get PLIC base address
pub fn get_plic_base() -> u64 {
    get_platform_info()
        .map(|info| info.plic_base)
        .unwrap_or(0x0c000000)
}

/// Get timer frequency
pub fn get_timer_frequency() -> u64 {
    get_platform_info()
        .map(|info| info.timer_freq)
        .unwrap_or(10000000)
}

/// Early platform initialization
pub fn early_init() -> Result<(), &'static str> {
    log::debug!("Early platform initialization");

    // Initialize console UART
    uart::early_init()?;

    // Initialize timer (basic)
    timer::early_init()?;

    Ok(())
}

/// Late platform initialization
pub fn late_init() -> Result<(), &'static str> {
    log::debug!("Late platform initialization");

    // Initialize CLINT
    clint::init()?;

    // Initialize PLIC
    plic::init()?;

    // Initialize full UART features
    uart::late_init()?;

    // Initialize full timer features
    timer::late_init()?;

    // Initialize memory management
    memory::init()?;

    Ok(())
}

/// Platform reset
pub fn reset() -> ! {
    log::warn!("Platform reset requested");

    // Platform-specific reset implementation
    match get_platform_type() {
        PlatformType::QemuVirt => {
            // QEMU virt reset via SiFive test device
            const TEST_DEVICE: u64 = 0x100000;
            const TEST_RESET: u32 = 0x5555;
            unsafe {
                core::ptr::write_volatile(TEST_DEVICE as *mut u32, TEST_RESET);
            }
        }
        _ => {
            // Generic reset - trigger system reset exception
            riscv::asm::wfi();
        }
    }

    // Should not reach here
    loop {
        riscv::asm::wfi();
    }
}

/// Platform power off
pub fn power_off() -> ! {
    log::warn!("Platform power off requested");

    // Platform-specific power off implementation
    match get_platform_type() {
        PlatformType::QemuVirt => {
            // QEMU virt power off via SiFive test device
            const TEST_DEVICE: u64 = 0x100000;
            const TEST_POWER_OFF: u32 = 0x3333;
            unsafe {
                core::ptr::write_volatile(TEST_DEVICE as *mut u32, TEST_POWER_OFF);
            }
        }
        _ => {
            // Generic power off - infinite loop
        }
    }

    // Should not reach here
    loop {
        riscv::asm::wfi();
    }
}

/// Platform panic handler
pub fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    use crate::arch::riscv64::cpu::halt;

    log::error!("Platform panic: {}", info);

    // Try to write panic info to UART
    if let Ok(mut uart) = uart::Console::new() {
        let _ = uart.write_str("\n!!! PANIC !!!\n");
        let _ = uart.write_str(info.to_string().as_str());
        let _ = uart.write_str("\nSystem halted.\n");
    }

    // Halt the system
    halt();
}

/// Get platform-specific memory regions
pub fn get_memory_regions() -> Vec<memory::MemoryRegion> {
    if let Some(config) = get_platform_configurations() {
        config.memory.get_regions()
    } else {
        memory::MemoryConfig::default().get_regions()
    }
}

/// Validate platform configuration
pub fn validate_config(config: &PlatformConfig) -> Result<(), String> {
    // Validate CPU count
    if config.cpu_count == 0 || config.cpu_count > MAX_CPUS as u32 {
        return Err(format!("Invalid CPU count: {}", config.cpu_count));
    }

    // Validate memory size
    if config.memory_size == 0 || config.memory_size > 0x1000000000 {
        return Err(format!("Invalid memory size: {:#x}", config.memory_size));
    }

    // Validate base addresses
    if config.uart_base == 0 || (config.uart_base & 0xFFF) != 0 {
        return Err(format!("Invalid UART base address: {:#x}", config.uart_base));
    }

    if config.clint_base == 0 || (config.clint_base & 0xFFF) != 0 {
        return Err(format!("Invalid CLINT base address: {:#x}", config.clint_base));
    }

    if config.plic_base == 0 || (config.plic_base & 0xFFF) != 0 {
        return Err(format!("Invalid PLIC base address: {:#x}", config.plic_base));
    }

    Ok(())
}

/// Platform statistics
#[derive(Debug, Clone, Default)]
pub struct PlatformStats {
    /// Uptime in milliseconds
    pub uptime_ms: u64,
    /// Context switches
    pub context_switches: u64,
    /// Interrupts handled
    pub interrupts_handled: u64,
    /// System calls
    pub syscalls: u64,
}

/// Get platform statistics
pub fn get_stats() -> PlatformStats {
    // TODO: Collect actual platform statistics
    PlatformStats::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_info() {
        let info = PlatformInfo::default();
        assert_eq!(info.platform_type, PlatformType::QemuVirt);
        assert_eq!(info.name, "QEMU Virt");
        assert_eq!(info.cpu_count, 4);
        assert_eq!(info.memory_size, 0x80000000);
        assert_eq!(info.uart_base, 0x10000000);
        assert_eq!(info.clint_base, 0x02000000);
        assert_eq!(info.plic_base, 0x0c000000);
        assert_eq!(info.timer_freq, 10000000);
    }

    #[test]
    fn test_platform_configurations() {
        let config = PlatformConfigurations::default();
        // Verify default configurations are created
        assert_eq!(config.memory, memory::MemoryConfig::default());
        assert_eq!(config.timer, timer::TimerConfig::default());
        assert_eq!(config.uart, uart::UartConfig::default());
        assert_eq!(config.clint, clint::ClintConfig::default());
        assert_eq!(config.plic, plic::PlicConfig::default());
    }

    #[test]
    fn test_platform_info_defaults() {
        // When platform info is not set, defaults should be returned
        assert_eq!(get_cpu_count(), 1);
        assert_eq!(get_memory_size(), 0);
        assert_eq!(get_uart_base(), 0x10000000);
        assert_eq!(get_clint_base(), 0x02000000);
        assert_eq!(get_plic_base(), 0x0c000000);
        assert_eq!(get_timer_frequency(), 10000000);
    }

    #[test]
    fn test_platform_type() {
        assert_eq!(get_platform_type(), PlatformType::QemuVirt);
    }
}