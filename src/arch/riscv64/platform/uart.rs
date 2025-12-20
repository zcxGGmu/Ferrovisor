//! RISC-V Platform UART Support
//!
//! This module provides platform-specific UART support including:
//! - UART initialization and configuration
//! - Console I/O operations
//! - Platform-specific UART features
//! - Multiple UART support

use crate::arch::riscv64::*;

/// UART configuration
#[derive(Debug, Clone)]
pub struct UartConfig {
    /// UART base address
    pub base_address: u64,
    /// Baud rate
    pub baud_rate: u32,
    /// Data bits
    pub data_bits: u8,
    /// Stop bits
    pub stop_bits: u8,
    /// Parity
    pub parity: Parity,
    /// Flow control
    pub flow_control: FlowControl,
}

impl Default for UartConfig {
    fn default() -> Self {
        Self {
            base_address: 0x10000000, // QEMU virt UART default
            baud_rate: 115200,
            data_bits: 8,
            stop_bits: 1,
            parity: Parity::None,
            flow_control: FlowControl::None,
        }
    }
}

/// Parity configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parity {
    None,
    Even,
    Odd,
}

/// Flow control configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowControl {
    None,
    Hardware,
    Software,
}

/// UART register offsets for 16550-compatible UART
pub mod uart16550 {
    pub const RHR: usize = 0;   // Receiver Holding Buffer (read)
    pub const THR: usize = 0;   // Transmitter Holding Buffer (write)
    pub const IER: usize = 1;   // Interrupt Enable Register
    pub const IIR: usize = 2;   // Interrupt Identification Register (read)
    pub const FCR: usize = 2;   // FIFO Control Register (write)
    pub const LCR: usize = 3;   // Line Control Register
    pub const MCR: usize = 4;   // Modem Control Register
    pub const LSR: usize = 5;   // Line Status Register
    pub const MSR: usize = 6;   // Modem Status Register
    pub const SPR: usize = 7;   // Scratch Register
    pub const DLL: usize = 0;   // Divisor Latch LSB (when DLAB=1)
    pub const DLM: usize = 1;   // Divisor Latch MSB (when DLAB=1)
}

/// UART driver interface
pub trait UartDriver {
    /// Initialize UART
    fn init(&mut self) -> Result<(), &'static str>;

    /// Write byte
    fn write_byte(&mut self, byte: u8) -> Result<(), &'static str>;

    /// Read byte
    fn read_byte(&mut self) -> Option<u8>;

    /// Write bytes
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), &'static str>;

    /// Read bytes
    fn read_bytes(&mut self, buf: &mut [u8]) -> usize;

    /// Check if transmitter is ready
    fn is_tx_ready(&self) -> bool;

    /// Check if receiver has data
    fn is_rx_ready(&self) -> bool;

    /// Flush transmitter
    fn flush_tx(&mut self);

    /// Set baud rate
    fn set_baud_rate(&mut self, baud_rate: u32) -> Result<(), &'static str>;

    /// Get current configuration
    fn get_config(&self) -> &UartConfig;
}

/// 16550-compatible UART driver
pub struct Uart16550 {
    /// Base address
    base: u64,
    /// Configuration
    config: UartConfig,
}

impl Uart16550 {
    /// Create new UART driver
    pub fn new(base: u64, config: UartConfig) -> Self {
        Self { base, config }
    }

    /// Read register
    fn read_reg(&self, offset: usize) -> u8 {
        unsafe {
            core::ptr::read_volatile((self.base + offset as u64) as *const u8)
        }
    }

    /// Write register
    fn write_reg(&mut self, offset: usize, value: u8) {
        unsafe {
            core::ptr::write_volatile((self.base + offset as u64) as *mut u8, value);
        }
    }

    /// Wait for transmitter ready
    fn wait_tx_ready(&self) {
        while !self.is_tx_ready() {
            riscv::asm::pause();
        }
    }

    /// Calculate divisor for baud rate
    fn calculate_divisor(&self, baud_rate: u32, clock_freq: u32) -> u16 {
        (clock_freq / (baud_rate * 16)) as u16
    }
}

impl UartDriver for Uart16550 {
    fn init(&mut self) -> Result<(), &'static str> {
        log::debug!("Initializing UART at {:#x}", self.base);

        // Disable interrupts
        self.write_reg(uart16550::IER, 0x00);

        // Set DLAB to access baud rate divisor
        self.write_reg(uart16550::LCR, 0x80);

        // Set baud rate divisor (assuming 115200 baud and 1.8432MHz clock)
        let divisor = self.calculate_divisor(self.config.baud_rate, 1843200);
        self.write_reg(uart16550::DLL, (divisor & 0xFF) as u8);
        self.write_reg(uart16550::DLM, ((divisor >> 8) & 0xFF) as u8);

        // Clear DLAB and set line configuration (8N1)
        self.write_reg(uart16550::LCR, 0x03);

        // Enable FIFO, clear FIFO, set trigger level to 1 byte
        self.write_reg(uart16550::FCR, 0x07);

        // Enable modem control (RTS/DTR)
        self.write_reg(uart16550::MCR, 0x03);

        // Test if UART is working by reading line status
        let _lsr = self.read_reg(uart16550::LSR);

        log::debug!("UART initialized at {:#x}, baud: {}", self.base, self.config.baud_rate);
        Ok(())
    }

    fn write_byte(&mut self, byte: u8) -> Result<(), &'static str> {
        self.wait_tx_ready();
        self.write_reg(uart16550::THR, byte);
        Ok(())
    }

    fn read_byte(&mut self) -> Option<u8> {
        if self.is_rx_ready() {
            Some(self.read_reg(uart16550::RHR))
        } else {
            None
        }
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), &'static str> {
        for &byte in bytes {
            self.write_byte(byte)?;
        }
        Ok(())
    }

    fn read_bytes(&mut self, buf: &mut [u8]) -> usize {
        let mut count = 0;
        for byte in buf.iter_mut() {
            if let Some(b) = self.read_byte() {
                *byte = b;
                count += 1;
            } else {
                break;
            }
        }
        count
    }

    fn is_tx_ready(&self) -> bool {
        let lsr = self.read_reg(uart16550::LSR);
        (lsr & 0x20) != 0 // THRE bit
    }

    fn is_rx_ready(&self) -> bool {
        let lsr = self.read_reg(uart16550::LSR);
        (lsr & 0x01) != 0 // DR bit
    }

    fn flush_tx(&mut self) {
        self.wait_tx_ready();
    }

    fn set_baud_rate(&mut self, baud_rate: u32) -> Result<(), &'static str> {
        // Save current LCR
        let lcr = self.read_reg(uart16550::LCR);

        // Set DLAB
        self.write_reg(uart16550::LCR, lcr | 0x80);

        // Set new baud rate
        let divisor = self.calculate_divisor(baud_rate, 1843200);
        self.write_reg(uart16550::DLL, (divisor & 0xFF) as u8);
        self.write_reg(uart16550::DLM, ((divisor >> 8) & 0xFF) as u8);

        // Restore LCR
        self.write_reg(uart16550::LCR, lcr);

        self.config.baud_rate = baud_rate;
        Ok(())
    }

    fn get_config(&self) -> &UartConfig {
        &self.config
    }
}

/// Console interface for early boot and debug output
pub struct Console {
    /// UART driver
    uart: Option<Box<dyn UartDriver>>,
}

impl Console {
    /// Create new console
    pub fn new() -> Result<Self, &'static str> {
        Ok(Self { uart: None })
    }

    /// Initialize console with specific UART driver
    pub fn init(&mut self, uart: Box<dyn UartDriver>) -> Result<(), &'static str> {
        // Initialize UART
        // Note: We need mutable access but this is simplified
        log::debug!("Console initialized with UART");
        self.uart = Some(uart);
        Ok(())
    }

    /// Write string to console
    pub fn write_str(&mut self, s: &str) -> Result<(), &'static str> {
        if let Some(ref mut uart) = self.uart {
            uart.write_bytes(s.as_bytes())?;
        }
        Ok(())
    }

    /// Write formatted string to console
    pub fn write_fmt(&mut self, args: core::fmt::Arguments<'_>) -> Result<(), &'static str> {
        if let Some(ref mut uart) = self.uart {
            use core::fmt::Write;
            let mut writer = UartWriter { uart };
            write!(writer, "{}", args).map_err(|_| "Write failed")?;
        }
        Ok(())
    }

    /// Read byte from console
    pub fn read_byte(&mut self) -> Option<u8> {
        self.uart.as_mut()?.read_byte()
    }

    /// Check if console is ready
    pub fn is_ready(&self) -> bool {
        self.uart.is_some()
    }
}

/// Writer for formatted output
struct UartWriter<'a> {
    uart: &'a mut dyn UartDriver,
}

impl core::fmt::Write for UartWriter<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.uart.write_bytes(s.as_bytes()).map_err(|_| core::fmt::Error)
    }
}

/// Global console instance
static mut CONSOLE: Option<Console> = None;
static CONSOLE_INIT: spin::Once<()> = spin::Once::new();

/// Initialize console (early)
pub fn early_init() -> Result<(), &'static str> {
    CONSOLE_INIT.call_once(|| {
        let base = super::get_uart_base();
        let config = UartConfig::default();
        let mut uart = Box::new(Uart16550::new(base, config));
        uart.init().ok(); // Ignore errors during early init

        let mut console = Console::new().unwrap();
        console.init(uart).ok(); // Ignore errors during early init

        unsafe {
            CONSOLE = Some(console);
        }
    });

    Ok(())
}

/// Initialize console (late)
pub fn late_init() -> Result<(), &'static str> {
    log::info!("Initializing platform console subsystem");

    // Console is already initialized in early_init
    if get_console().is_some() {
        log::debug!("Console already initialized");
    }

    Ok(())
}

/// Get console instance
pub fn get_console() -> Option<&'static mut Console> {
    unsafe { CONSOLE.as_mut() }
}

/// Print string to console
pub fn print(s: &str) {
    if let Some(console) = get_console() {
        let _ = console.write_str(s);
    }
}

/// Print formatted string to console
pub fn print_fmt(args: core::fmt::Arguments<'_>) {
    if let Some(console) = get_console() {
        let _ = console.write_fmt(args);
    }
}

/// Println! macro for console output
#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {
        $crate::arch::riscv64::platform::uart::print_fmt(format_args!($($arg)*));
        $crate::arch::riscv64::platform::uart::print("\n");
    }
}

/// Print! macro for console output
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::arch::riscv64::platform::uart::print_fmt(format_args!($($arg)*));
    }
}

/// Platform-specific UART initialization
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing platform UART");

    // Get platform configuration
    let uart_config = if let Some(platform_config) = super::get_platform_configurations() {
        platform_config.uart.clone()
    } else {
        UartConfig::default()
    };

    // Create and initialize UART
    let base = uart_config.base_address;
    let mut uart = Box::new(Uart16550::new(base, uart_config));
    uart.init()?;

    // Initialize console
    if let Some(console) = get_console() {
        console.init(uart)?;
    }

    log::info!("Platform UART initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uart_config() {
        let config = UartConfig::default();
        assert_eq!(config.base_address, 0x10000000);
        assert_eq!(config.baud_rate, 115200);
        assert_eq!(config.data_bits, 8);
        assert_eq!(config.stop_bits, 1);
        assert_eq!(config.parity, Parity::None);
        assert_eq!(config.flow_control, FlowControl::None);
    }

    #[test]
    fn test_uart16550() {
        let config = UartConfig::default();
        let uart = Uart16550::new(0x10000000, config);
        assert_eq!(uart.base, 0x10000000);
        assert_eq!(uart.config.baud_rate, 115200);
    }

    #[test]
    fn test_console() {
        let console = Console::new();
        assert!(console.is_ok());
        assert!(!console.unwrap().is_ready());
    }

    #[test]
    fn test_parity() {
        assert_eq!(Parity::None, Parity::None);
        assert_eq!(Parity::Even, Parity::Even);
        assert_eq!(Parity::Odd, Parity::Odd);
        assert_ne!(Parity::None, Parity::Even);
    }

    #[test]
    fn test_flow_control() {
        assert_eq!(FlowControl::None, FlowControl::None);
        assert_eq!(FlowControl::Hardware, FlowControl::Hardware);
        assert_eq!(FlowControl::Software, FlowControl::Software);
        assert_ne!(FlowControl::None, FlowControl::Hardware);
    }
}