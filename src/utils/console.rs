//! Console output functionality
//!
//! This module provides console output capabilities for the
//! hypervisor, supporting multiple output destinations.

use core::fmt;

/// Console interface trait
pub trait Console {
    /// Write a single character
    fn write_char(&self, c: u8);

    /// Write a buffer of characters
    fn write(&self, buf: &[u8]) {
        for &c in buf {
            self.write_char(c);
        }
    }

    /// Flush any buffered output
    fn flush(&self) {
        // Default: nothing to flush
    }

    /// Check if console is ready
    fn is_ready(&self) -> bool {
        true
    }
}

/// UART console implementation
pub struct UartConsole {
    base_address: usize,
}

impl UartConsole {
    /// Create a new UART console
    pub const fn new(base_address: usize) -> Self {
        Self { base_address }
    }

    /// Check if UART is ready to transmit
    fn is_transmit_ready(&self) -> bool {
        #[cfg(target_arch = "aarch64")]
        {
            // PL011 UART flags register
            let flags = unsafe { core::ptr::read_volatile((self.base_address + 0x18) as *const u32) };
            (flags & (1 << 5)) != 0
        }

        #[cfg(target_arch = "riscv64")]
        {
            // Simple UART implementation
            true
        }

        #[cfg(target_arch = "x86_64")]
        {
            // COM1 port
            let line_status = unsafe { core::ptr::read_volatile((self.base_address + 5) as *const u8) };
            (line_status & 0x20) != 0
        }
    }
}

impl Console for UartConsole {
    fn write_char(&self, c: u8) {
        // Wait for UART to be ready
        while !self.is_transmit_ready() {
            #[cfg(target_arch = "aarch64")]
            cortex_a::asm::nop();

            #[cfg(target_arch = "riscv64")]
            riscv::asm::nop();

            #[cfg(target_arch = "x86_64")]
            x86_64::instructions::nop();
        }

        // Write character to UART data register
        unsafe {
            #[cfg(target_arch = "aarch64")]
            core::ptr::write_volatile(self.base_address as *mut u8, c);

            #[cfg(target_arch = "riscv64")]
            core::ptr::write_volatile(self.base_address as *mut u8, c);

            #[cfg(target_arch = "x86_64")]
            core::ptr::write_volatile(self.base_address as *mut u8, c);
        }

        // Handle carriage return
        if c == b'\n' {
            self.write_char(b'\r');
        }
    }

    fn is_ready(&self) -> bool {
        true // UART is always ready
    }
}

/// Default console instance
static mut DEFAULT_CONSOLE: Option<UartConsole> = None;
static CONSOLE_INIT: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// Initialize the console
pub fn init() {
    if !CONSOLE_INIT.load(core::sync::atomic::Ordering::Relaxed) {
        unsafe {
            #[cfg(target_arch = "aarch64")]
            {
                // Use PL011 UART at 0x9000000
                DEFAULT_CONSOLE = Some(UartConsole::new(0x9000000));
            }

            #[cfg(target_arch = "riscv64")]
            {
                // Use UART at 0x10000000
                DEFAULT_CONSOLE = Some(UartConsole::new(0x10000000));
            }

            #[cfg(target_arch = "x86_64")]
            {
                // Use COM1 port
                DEFAULT_CONSOLE = Some(UartConsole::new(0x3F8));
            }
        }
        CONSOLE_INIT.store(true, core::sync::atomic::Ordering::Relaxed);
    }
}

/// Get the default console
fn get_console() -> &'static UartConsole {
    unsafe {
        DEFAULT_CONSOLE.as_ref().unwrap_or(&UartConsole::new(0))
    }
}

/// Print a formatted string
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::utils::console::print_fmt(format_args!($($arg)*))
    };
}

/// Print a formatted string with newline
#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {
        $crate::utils::console::print_fmt(format_args!($($arg)*));
        $crate::utils::console::print_char(b'\n');
    };
}

/// Print using format arguments
pub fn print_fmt(args: fmt::Arguments<'_>) {
    if !CONSOLE_INIT.load(core::sync::atomic::Ordering::Relaxed) {
        init();
    }
    let console = get_console();
    let mut writer = ConsoleWriter { console };
    fmt::write(&mut writer, args).unwrap();
}

/// Print a single character
pub fn print_char(c: u8) {
    if !CONSOLE_INIT.load(core::sync::atomic::Ordering::Relaxed) {
        init();
    }
    get_console().write_char(c);
}

/// Print a buffer
pub fn print_bytes(buf: &[u8]) {
    if !CONSOLE_INIT.load(core::sync::atomic::Ordering::Relaxed) {
        init();
    }
    get_console().write(buf);
}

/// Flush the console
pub fn flush() {
    if !CONSOLE_INIT.load(core::sync::atomic::Ordering::Relaxed) {
        init();
    }
    get_console().flush();
}

/// Writer for formatted output
struct ConsoleWriter<'a> {
    console: &'a dyn Console,
}

impl<'a> fmt::Write for ConsoleWriter<'a> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.console.write(s.as_bytes());
        Ok(())
    }

    fn write_char(&mut self, c: char) -> fmt::Result {
        self.console.write_char(c as u8);
        Ok(())
    }
}

/// Set console color (ANSI escape sequences)
pub fn set_color(color: Color) {
    print!("\x1b[{}m", color as u8);
}

/// Reset console color
pub fn reset_color() {
    print!("\x1b[0m");
}

/// Console colors
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Black = 30,
    Red = 31,
    Green = 32,
    Yellow = 33,
    Blue = 34,
    Magenta = 35,
    Cyan = 36,
    White = 37,
    BrightBlack = 90,
    BrightRed = 91,
    BrightGreen = 92,
    BrightYellow = 93,
    BrightBlue = 94,
    BrightMagenta = 95,
    BrightCyan = 96,
    BrightWhite = 97,
}