//! GPIO (General Purpose Input/Output) Emulator
//!
//! This module provides GPIO emulation for guest operating systems,
//! supporting GPIO controllers like PL061, etc.

use crate::{Result, Error};
use crate::emulator::{Emulator, Error as EmulatorError};
use crate::core::mm::{VirtAddr, PhysAddr};
use crate::arch::common::MmioAccess;
use crate::utils::spinlock::SpinLock;

/// PL061 GPIO registers
#[allow(dead_code)]
#[repr(usize)]
enum Pl061Register {
    Data = 0x00,
    Direction = 0x04,
    InterruptSense = 0x08,
    InterruptBothEdges = 0x0C,
    InterruptEvent = 0x10,
    InterruptMask = 0x14,
    RawInterruptStatus = 0x18,
    MaskedInterruptStatus = 0x1C,
    InterruptClear = 0x20,
    AlternateFunctionSelect = 0x24,
    PullUpSelect = 0x28,
    PullDownSelect = 0x2C,
    PullEnable = 0x30,
    PadControl = 0x34,
}

/// GPIO pin mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpioMode {
    /// Input
    Input,
    /// Output
    Output,
    /// Alternate function
    Alternate(u8),
}

/// GPIO interrupt mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpioInterruptMode {
    /// No interrupt
    None,
    /// Edge triggered
    Edge,
    /// Level triggered
    Level,
    /// Both edges
    BothEdges,
}

/// GPIO pin state
#[derive(Debug, Clone, Copy)]
pub struct GpioPinState {
    /// Pin mode
    mode: GpioMode,
    /// Current value
    value: bool,
    /// Interrupt mode
    interrupt_mode: GpioInterruptMode,
    /// Interrupt pending
    interrupt_pending: bool,
    /// Pull-up enabled
    pull_up: bool,
    /// Pull-down enabled
    pull_down: bool,
}

/// PL061 GPIO state
#[derive(Debug, Clone)]
pub struct Pl061State {
    /// GPIO pins (8 pins)
    pins: [GpioPinState; 8],
    /// Data register (input values)
    data: u8,
    /// Direction register (0=input, 1=output)
    direction: u8,
    /// Interrupt sense register (0=edge, 1=level)
    interrupt_sense: u8,
    /// Interrupt both edges register
    interrupt_both_edges: u8,
    /// Interrupt event register (0=falling, 1=rising)
    interrupt_event: u8,
    /// Interrupt mask register (0=masked, 1=enabled)
    interrupt_mask: u8,
    /// Raw interrupt status
    raw_interrupt_status: u8,
    /// Masked interrupt status
    masked_interrupt_status: u8,
    /// Alternate function select registers
    afsel: [u8; 2],
    /// Pull-up select
    pull_up: u8,
    /// Pull-down select
    pull_down: u8,
    /// Pull enable
    pull_enable: u8,
}

/// PL061 GPIO emulator
pub struct Pl061Gpio {
    /// Base address
    base_addr: PhysAddr,
    /// Device state
    state: SpinLock<Pl061State>,
    /// MMIO access interface
    mmio: MmioAccess,
}

impl Pl061Gpio {
    /// Create a new PL061 GPIO emulator
    pub fn new(base_addr: PhysAddr) -> Self {
        let mut pins = [GpioPinState {
            mode: GpioMode::Input,
            value: false,
            interrupt_mode: GpioInterruptMode::None,
            interrupt_pending: false,
            pull_up: false,
            pull_down: false,
        }; 8];

        // Set default pull-up on some pins (common configuration)
        pins[0].pull_up = true;
        pins[1].pull_up = true;
        pins[2].pull_up = true;
        pins[3].pull_up = true;

        let state = Pl061State {
            pins,
            data: 0xFF, // All pins high due to pull-up
            direction: 0x00, // All inputs by default
            interrupt_sense: 0x00,
            interrupt_both_edges: 0x00,
            interrupt_event: 0x00,
            interrupt_mask: 0x00,
            raw_interrupt_status: 0x00,
            masked_interrupt_status: 0x00,
            afsel: [0x00, 0x00],
            pull_up: 0x0F,
            pull_down: 0x00,
            pull_enable: 0x0F,
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

    /// Get value of a specific pin
    pub fn get_pin(&self, pin: u8) -> Option<bool> {
        if pin >= 8 {
            return None;
        }

        let state = self.state.lock();
        Some(state.pins[pin as usize].value)
    }

    /// Set value of a specific pin (for external input)
    pub fn set_pin(&self, pin: u8, value: bool) -> Result<(), Error> {
        if pin >= 8 {
            return Err(Error::InvalidArgument);
        }

        let mut state = self.state.lock();
        let pin_state = &mut state.pins[pin as usize];
        let old_value = pin_state.value;

        if pin_state.mode == GpioMode::Input {
            pin_state.value = value;

            // Update data register for input pins
            if value {
                state.data |= 1 << pin;
            } else {
                state.data &= !(1 << pin);
            }

            // Check for interrupt
            if pin_state.interrupt_mode != GpioInterruptMode::None &&
               (state.interrupt_mask & (1 << pin)) != 0 {
                let mut trigger_interrupt = false;

                match pin_state.interrupt_mode {
                    GpioInterruptMode::Edge => {
                        if value != old_value {
                            trigger_interrupt = true;
                        }
                    }
                    GpioInterruptMode::BothEdges => {
                        if value != old_value {
                            trigger_interrupt = true;
                        }
                    }
                    GpioInterruptMode::Level => {
                        if value && (state.interrupt_sense & (1 << pin)) == 0 {
                            // High level triggered
                            trigger_interrupt = true;
                        } else if !value && (state.interrupt_sense & (1 << pin)) != 0 {
                            // Low level triggered
                            trigger_interrupt = true;
                        }
                    }
                    _ => {}
                }

                if trigger_interrupt {
                    state.raw_interrupt_status |= 1 << pin;
                    state.masked_interrupt_status = state.raw_interrupt_status & state.interrupt_mask;
                    pin_state.interrupt_pending = true;

                    crate::info!("GPIO {} triggered interrupt", pin);
                }
            }
        }

        Ok(())
    }

    /// Configure a pin mode
    pub fn configure_pin(&self, pin: u8, mode: GpioMode) -> Result<(), Error> {
        if pin >= 8 {
            return Err(Error::InvalidArgument);
        }

        let mut state = self.state.lock();
        let pin_state = &mut state.pins[pin as usize];
        pin_state.mode = mode;

        // Update direction register
        match mode {
            GpioMode::Input => {
                state.direction &= !(1 << pin);
            }
            GpioMode::Output | GpioMode::Alternate(_) => {
                state.direction |= 1 << pin;
            }
        }

        // Update alternate function select
        if let GpioMode::Alternate(af) = mode {
            let reg_index = pin / 4;
            let bit_offset = (pin % 4) * 2;
            state.afsel[reg_index as usize] &= !(0x3 << bit_offset);
            state.afsel[reg_index as usize] |= ((af & 0x3) << bit_offset);
        }

        Ok(())
    }
}

impl Emulator for Pl061Gpio {
    fn name(&self) -> &str {
        "PL061-GPIO"
    }

    fn read(&self, offset: u64, size: u32) -> Result<u64, EmulatorError> {
        if size != 8 && size != 16 && size != 32 {
            return Err(EmulatorError::InvalidAccess);
        }

        let mut state = self.state.lock();
        let addr = offset as usize;

        let value = match addr {
            x if x == Pl061Register::Data as usize => {
                // For output pins, return the last written value
                // For input pins, return the current input value
                let mut data = state.data;

                // Set output pins to last written value
                for pin in 0..8 {
                    if (state.direction & (1 << pin)) != 0 {
                        // This is an output pin
                        if let GpioMode::Output = state.pins[pin].mode {
                            if state.pins[pin].value {
                                data |= 1 << pin;
                            } else {
                                data &= !(1 << pin);
                            }
                        }
                    }
                }

                data as u64
            }
            x if x == Pl061Register::Direction as usize => state.direction as u64,
            x if x == Pl061Register::InterruptSense as usize => state.interrupt_sense as u64,
            x if x == Pl061Register::InterruptBothEdges as usize => state.interrupt_both_edges as u64,
            x if x == Pl061Register::InterruptEvent as usize => state.interrupt_event as u64,
            x if x == Pl061Register::InterruptMask as usize => state.interrupt_mask as u64,
            x if x == Pl061Register::RawInterruptStatus as usize => state.raw_interrupt_status as u64,
            x if x == Pl061Register::MaskedInterruptStatus as usize => state.masked_interrupt_status as u64,
            x if x == Pl061Register::AlternateFunctionSelect as usize => {
                // Return the appropriate register based on address
                let reg_offset = (offset >> 2) & 0x1;
                state.afsel[reg_offset as usize] as u64
            }
            x if x == Pl061Register::PullUpSelect as usize => state.pull_up as u64,
            x if x == Pl061Register::PullDownSelect as usize => state.pull_down as u64,
            x if x == Pl061Register::PullEnable as usize => state.pull_enable as u64,
            x if x == Pl061Register::PadControl as usize => 0, // Not implemented
            _ => {
                crate::warn!("PL061: Unhandled read from offset 0x{:x}", addr);
                0
            }
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
        let addr = offset as usize;
        let byte_value = (value & 0xFF) as u8;
        let word_value = (value & 0xFFFFFFFF) as u32;

        match addr {
            x if x == Pl061Register::Data as usize => {
                // Write to output pins
                let mask = 0xFF;
                let write_data = byte_value & mask;

                for pin in 0..8 {
                    if (state.direction & (1 << pin)) != 0 {
                        // This pin is configured as output
                        let pin_value = (write_data >> pin) & 0x1 != 0;
                        state.pins[pin].value = pin_value;
                    }
                }
            }
            x if x == Pl061Register::Direction as usize => {
                let new_direction = byte_value & 0xFF;
                for pin in 0..8 {
                    let is_output = (new_direction & (1 << pin)) != 0;
                    if is_output {
                        state.direction |= 1 << pin;
                        state.pins[pin].mode = GpioMode::Output;
                    } else {
                        state.direction &= !(1 << pin);
                        state.pins[pin].mode = GpioMode::Input;
                    }
                }
            }
            x if x == Pl061Register::InterruptSense as usize => {
                state.interrupt_sense = byte_value & 0xFF;
            }
            x if x == Pl061Register::InterruptBothEdges as usize => {
                state.interrupt_both_edges = byte_value & 0xFF;
            }
            x if x == Pl061Register::InterruptEvent as usize => {
                state.interrupt_event = byte_value & 0xFF;
            }
            x if x == Pl061Register::InterruptMask as usize => {
                state.interrupt_mask = byte_value & 0xFF;
                state.masked_interrupt_status = state.raw_interrupt_status & state.interrupt_mask;
            }
            x if x == Pl061Register::InterruptClear as usize => {
                let clear_mask = byte_value & 0xFF;
                state.raw_interrupt_status &= !clear_mask;
                for pin in 0..8 {
                    if (clear_mask & (1 << pin)) != 0 {
                        state.pins[pin].interrupt_pending = false;
                    }
                }
                state.masked_interrupt_status = state.raw_interrupt_status & state.interrupt_mask;
            }
            x if x == Pl061Register::AlternateFunctionSelect as usize => {
                // Handle address-based AFSEL register selection
                let reg_offset = (offset >> 2) & 0x1;
                state.afsel[reg_offset as usize] = (word_value & 0xFFFF) as u8;
            }
            x if x == Pl061Register::PullUpSelect as usize => {
                state.pull_up = byte_value & 0xFF;
            }
            x if x == Pl061Register::PullDownSelect as usize => {
                state.pull_down = byte_value & 0xFF;
            }
            x if x == Pl061Register::PullEnable as usize => {
                state.pull_enable = byte_value & 0xFF;
            }
            _ => {
                crate::warn!("PL061: Unhandled write 0x{:x} to offset 0x{:x}", value, addr);
            }
        }

        Ok(())
    }

    fn reset(&mut self) -> Result<(), EmulatorError> {
        let mut state = self.state.lock();

        // Reset pins to input mode
        for pin in state.pins.iter_mut() {
            pin.mode = GpioMode::Input;
            pin.value = false;
            pin.interrupt_mode = GpioInterruptMode::None;
            pin.interrupt_pending = false;
        }

        // Reset registers to default
        state.data = 0xFF;
        state.direction = 0x00;
        state.interrupt_sense = 0x00;
        state.interrupt_both_edges = 0x00;
        state.interrupt_event = 0x00;
        state.interrupt_mask = 0x00;
        state.raw_interrupt_status = 0x00;
        state.masked_interrupt_status = 0x00;
        state.afsel = [0x00, 0x00];
        state.pull_up = 0x0F;
        state.pull_down = 0x00;
        state.pull_enable = 0x0F;

        Ok(())
    }
}

/// Initialize GPIO emulators
pub fn init() -> Result<(), crate::Error> {
    crate::info!("Initializing GPIO emulators");

    // Register PL061 GPIO
    let pl061 = Pl061Gpio::new(0x40000000);
    crate::emulator::register_emulator("gpio-pl061", &pl061)?;

    Ok(())
}