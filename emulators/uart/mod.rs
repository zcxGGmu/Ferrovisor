//! UART (Serial Port) Emulator
//!
//! This module provides UART emulation for guest operating systems,
//! supporting common UART chips like PL011, 16550, etc.

use crate::{Result, Error};
use crate::emulator::{Emulator, Error as EmulatorError};
use crate::core::mm::{VirtAddr, PhysAddr};
use crate::arch::common::MmioAccess;
use crate::utils::spinlock::SpinLock;
use core::sync::atomic::{AtomicUsize, Ordering};

/// PL011 UART registers
#[allow(dead_code)]
#[repr(usize)]
enum Pl011Register {
    Data = 0x00,
    Status = 0x04,
    BaudRateDiv = 0x08,
    LineControl = 0x0C,
    Control = 0x10,
    InterruptFifoLevelSelect = 0x14,
    InterruptMaskSetClear = 0x18,
    RawInterruptStatus = 0x1C,
    MaskedInterruptStatus = 0x20,
    InterruptClear = 0x24,
    DMAControl = 0x28,
    TestControl = 0x2C,
}

/// PL011 UART state
#[derive(Debug, Clone)]
pub struct Pl011State {
    /// Data register
    data: u32,
    /// Status register
    status: u32,
    /// Baud rate divisor
    baud_div: u32,
    /// Line control register
    line_ctrl: u32,
    /// Control register
    ctrl: u32,
    /// FIFO level select
    ifls: u32,
    /// Interrupt mask
    int_mask: u32,
    /// Raw interrupt status
    raw_int: u32,
    /// Masked interrupt status
    masked_int: u32,
    /// Transmit FIFO
    tx_fifo: SpinLock<Vec<u8>>,
    /// Receive FIFO
    rx_fifo: SpinLock<Vec<u8>>,
    /// FIFO depth
    fifo_depth: usize,
    /// Character received from host
    host_char: Option<u8>,
}

/// PL011 UART emulator
pub struct Pl011Uart {
    /// Base address
    base_addr: PhysAddr,
    /// Device state
    state: SpinLock<Pl011State>,
    /// MMIO access interface
    mmio: MmioAccess,
}

impl Pl011Uart {
    /// Create a new PL011 UART emulator
    pub fn new(base_addr: PhysAddr) -> Self {
        let state = Pl011State {
            data: 0,
            status: 0x90, // TX empty, RX empty
            baud_div: 0,
            line_ctrl: 0,
            ctrl: 0,
            ifls: 0,
            int_mask: 0,
            raw_int: 0,
            masked_int: 0,
            tx_fifo: SpinLock::new(Vec::new()),
            rx_fifo: SpinLock::new(Vec::new()),
            fifo_depth: 16,
            host_char: None,
        };

        Self {
            base_addr,
            state: SpinLock::new(state),
            mmio: MmioAccess,
        }
    }

    /// Get the base address
    pub fn base_address(&self) -> PhysAddr {
        self.base_addr
    }

    /// Read a character from host (for testing)
    pub fn read_host_char(&self) -> Option<u8> {
        let mut state = self.state.lock();
        state.host_char
    }

    /// Write a character to host
    pub fn write_host_char(&self, c: u8) {
        // Echo to console
        crate::print!("{}", c as char);

        // Add to RX FIFO if UART is enabled for receive
        let mut state = self.state.lock();
        if state.ctrl & 0x01 != 0 { // UARTEN
            let mut rx_fifo = state.rx_fifo.lock();
            if rx_fifo.len() < state.fifo_depth {
                rx_fifo.push(c);
                state.raw_int |= 0x10; // RX interrupt
                state.masked_int = state.raw_int & !state.int_mask;
            }
        }
    }

    /// Write a string to host
    pub fn write_host_string(&self, s: &str) {
        for c in s.bytes() {
            self.write_host_char(c);
        }
    }
}

impl Emulator for Pl011Uart {
    fn name(&self) -> &str {
        "PL011-UART"
    }

    fn read(&self, offset: u64, size: u32) -> Result<u64, EmulatorError> {
        if size != 8 && size != 32 && size != 64 {
            return Err(EmulatorError::InvalidAccess);
        }

        let mut state = self.state.lock();
        let addr = offset as usize;

        let value = match addr {
            x if x == Pl011Register::Data as usize => {
                // Read from RX FIFO
                let mut rx_fifo = state.rx_fifo.lock();
                if let Some(c) = rx_fifo.pop_front() {
                    // Update FIFO status
                    state.status &= !0x10; // Clear RX FIFO full
                    if rx_fifo.is_empty() {
                        state.status |= 0x10; // RX FIFO empty
                    }
                    c as u64
                } else {
                    state.status & 0x10 // Return empty flag if no data
                }
            }
            x if x == Pl011Register::Status as usize => {
                let mut status = state.status;

                // Update TX FIFO status
                let tx_fifo = state.tx_fifo.lock();
                if tx_fifo.is_empty() {
                    status |= 0x80; // TX FIFO empty
                } else {
                    status &= !0x80;
                }

                status as u64
            }
            x if x == Pl011Register::BaudRateDiv as usize => state.baud_div as u64,
            x if x == Pl011Register::LineControl as usize => state.line_ctrl as u64,
            x if x == Pl011Register::Control as usize => state.ctrl as u64,
            x if x == Pl011Register::InterruptFifoLevelSelect as usize => state.ifls as u64,
            x if x == Pl011Register::RawInterruptStatus as usize => state.raw_int as u64,
            x if x == Pl011Register::MaskedInterruptStatus as usize => state.masked_int as u64,
            _ => {
                crate::warn!("PL011: Unhandled read from offset 0x{:x}", addr);
                0
            }
        };

        // Apply size mask
        match size {
            8 => value & 0xFF,
            32 => value & 0xFFFFFFFF,
            64 => value,
            _ => return Err(EmulatorError::InvalidAccess),
        }
    }

    fn write(&mut self, offset: u64, value: u64, size: u32) -> Result<(), EmulatorError> {
        if size != 8 && size != 32 && size != 64 {
            return Err(EmulatorError::InvalidAccess);
        }

        let mut state = self.state.lock();
        let addr = offset as usize;

        match addr {
            x if x == Pl011Register::Data as usize => {
                // Write to TX FIFO
                if state.ctrl & 0x01 != 0 { // UARTEN
                    let c = (value & 0xFF) as u8;

                    // Echo to console
                    crate::print!("{}", c as char);

                    let mut tx_fifo = state.tx_fifo.lock();
                    if tx_fifo.len() < state.fifo_depth {
                        tx_fifo.push(c);
                        state.status &= !0x20; // Clear TX FIFO full flag
                    } else {
                        state.status |= 0x20; // TX FIFO full
                    }
                }
            }
            x if x == Pl011Register::BaudRateDiv as usize => {
                state.baud_div = (value & 0xFFFF) as u32;
            }
            x if x == Pl011Register::LineControl as usize => {
                state.line_ctrl = (value & 0xFF) as u32;
            }
            x if x == Pl011Register::Control as usize => {
                let new_ctrl = (value & 0x7FF) as u32;
                if (new_ctrl & 0x01) == 0 && (state.ctrl & 0x01) != 0 {
                    // UART being disabled - clear FIFOs
                    state.tx_fifo.lock().clear();
                    state.rx_fifo.lock().clear();
                    state.status = 0x90; // Both FIFOs empty
                }
                state.ctrl = new_ctrl;
            }
            x if x == Pl011Register::InterruptFifoLevelSelect as usize => {
                state.ifls = (value & 0x3F) as u32;
            }
            x if x == Pl011Register::InterruptMaskSetClear as usize => {
                let mask = (value & 0x7FF) as u32;
                if value & (1 << 11) != 0 {
                    // Clear bits
                    state.int_mask &= !mask;
                } else {
                    // Set bits
                    state.int_mask |= mask;
                }
                state.masked_int = state.raw_int & !state.int_mask;
            }
            x if x == Pl011Register::InterruptClear as usize => {
                let clear = (value & 0x7FF) as u32;
                state.raw_int &= !clear;
                state.masked_int = state.raw_int & !state.int_mask;
            }
            _ => {
                crate::warn!("PL011: Unhandled write 0x{:x} to offset 0x{:x}", value, addr);
            }
        }

        Ok(())
    }

    fn reset(&mut self) -> Result<(), EmulatorError> {
        let mut state = self.state.lock();

        // Reset to default state
        state.data = 0;
        state.status = 0x90; // TX empty, RX empty
        state.baud_div = 0;
        state.line_ctrl = 0;
        state.ctrl = 0;
        state.ifls = 0;
        state.int_mask = 0;
        state.raw_int = 0;
        state.masked_int = 0;
        state.tx_fifo.lock().clear();
        state.rx_fifo.lock().clear();
        state.host_char = None;

        Ok(())
    }
}

/// 16550-compatible UART emulator
pub struct Uart16550 {
    /// Base address
    base_addr: PhysAddr,
    /// Device state
    state: SpinLock<Uart16550State>,
    /// MMIO access interface
    mmio: MmioAccess,
}

/// 16550 UART state
#[derive(Debug, Clone)]
pub struct Uart16550State {
    /// Read/Write Holding Register
    rhr_thr: u8,
    /// Interrupt Enable Register
    ier: u8,
    /// Interrupt Identification Register
    iir: u8,
    /// Line Control Register
    lcr: u8,
    /// Modem Control Register
    mcr: u8,
    /// Line Status Register
    lsr: u8,
    /// Modem Status Register
    msr: u8,
    /// Scratch Register
    scr: u8,
    /// Divisor Latch (Least Significant Byte)
    dll: u8,
    /// Divisor Latch (Most Significant Byte)
    dlm: u8,
    /// FIFO control register
    fcr: u8,
    /// RX FIFO
    rx_fifo: Vec<u8>,
    /// TX FIFO
    tx_fifo: Vec<u8>,
    /// FIFO enabled flag
    fifo_enabled: bool,
}

impl Uart16550 {
    /// Create a new 16550 UART emulator
    pub fn new(base_addr: PhysAddr) -> Self {
        let state = Uart16550State {
            rhr_thr: 0,
            ier: 0,
            iir: 0x01, // No interrupt pending
            lcr: 0,
            mcr: 0,
            lsr: 0x60, // TX empty, RX empty
            msr: 0,
            scr: 0,
            dll: 0,
            dlm: 0,
            fcr: 0,
            rx_fifo: Vec::new(),
            tx_fifo: Vec::new(),
            fifo_enabled: false,
        };

        Self {
            base_addr,
            state: SpinLock::new(state),
            mmio: MmioAccess,
        }
    }
}

impl Emulator for Uart16550 {
    fn name(&self) -> &str {
        "16550-UART"
    }

    fn read(&self, offset: u64, size: u32) -> Result<u64, EmulatorError> {
        if size != 8 && size != 16 && size != 32 {
            return Err(EmulatorError::InvalidAccess);
        }

        let mut state = self.state.lock();
        let addr = (offset & 0x7) as usize; // 8-register window

        let value = match (addr, state.lcr & 0x80) {
            // DLL/DLM access when DLAB=1
            (0, 0x80) => state.dll as u64,       // DLL
            (1, 0x80) => state.dlm as u64,       // DLM
            // Normal register access
            (0, _) => state.rhr_thr as u64,      // RHR
            (1, _) => state.ier as u64,          // IER
            (2, _) => state.iir as u64,          // IIR
            (3, _) => state.lcr as u64,          // LCR
            (4, _) => state.mcr as u64,          // MCR
            (5, _) => state.lsr as u64,          // LSR
            (6, _) => state.msr as u64,          // MSR
            (7, _) => state.scr as u64,          // SCR
            _ => 0,
        };

        // Apply size mask
        match size {
            8 => value & 0xFF,
            16 => value & 0xFFFF,
            32 => value & 0xFFFFFFFF,
            _ => return Err(EmulatorError::InvalidAccess),
        }
    }

    fn write(&mut self, offset: u64, value: u64, size: u32) -> Result<(), EmulatorError> {
        if size != 8 && size != 16 && size != 32 {
            return Err(EmulatorError::InvalidAccess);
        }

        let mut state = self.state.lock();
        let addr = (offset & 0x7) as usize; // 8-register window
        let byte_value = (value & 0xFF) as u8;

        match (addr, state.lcr & 0x80) {
            // DLL/DLM access when DLAB=1
            (0, 0x80) => state.dll = byte_value,     // DLL
            (1, 0x80) => state.dlm = byte_value,     // DLM
            // Normal register access
            (0, _) => {
                // THR - transmit holding register
                crate::print!("{}", byte_value as char);
                state.lsr |= 0x20; // TX empty
                state.lsr |= 0x40; // TX holding register empty
            }
            (1, _) => state.ier = byte_value & 0x0F, // IER
            (2, _) => {
                // FCR - FIFO control register
                state.fcr = byte_value;
                if byte_value & 0x01 != 0 {
                    state.fifo_enabled = true;
                    state.rx_fifo.clear();
                    state.tx_fifo.clear();
                }
                state.iir = if state.fifo_enabled { 0xC0 } else { 0x01 };
            }
            (3, _) => state.lcr = byte_value,         // LCR
            (4, _) => state.mcr = byte_value & 0x1F, // MCR
            (5, _) => {},                             // LSR (read-only)
            (6, _) => {},                             // MSR (read-only)
            (7, _) => state.scr = byte_value,         // SCR
            _ => {},
        }

        Ok(())
    }

    fn reset(&mut self) -> Result<(), EmulatorError> {
        let mut state = self.state.lock();

        // Reset to default state
        state.rhr_thr = 0;
        state.ier = 0;
        state.iir = 0x01;
        state.lcr = 0;
        state.mcr = 0;
        state.lsr = 0x60;
        state.msr = 0;
        state.scr = 0;
        state.dll = 0;
        state.dlm = 0;
        state.fcr = 0;
        state.rx_fifo.clear();
        state.tx_fifo.clear();
        state.fifo_enabled = false;

        Ok(())
    }
}

/// Initialize UART emulators
pub fn init() -> Result<(), crate::Error> {
    crate::info!("Initializing UART emulators");

    // Register PL011 UART at typical ARM location
    let pl011 = Pl011Uart::new(0x9000000);
    crate::emulator::register_emulator("uart-pl011", &pl011)?;

    // Register 16550 UART at typical PC location
    let uart16550 = Uart16550::new(0x3F8);
    crate::emulator::register_emulator("uart-16550", &uart16550)?;

    Ok(())
}