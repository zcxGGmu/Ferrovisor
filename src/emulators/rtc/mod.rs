//! RTC (Real Time Clock) Emulator
//!
//! This module provides RTC emulation for guest operating systems,
//! supporting RTC chips like PL031, MC146818, etc.

use crate::{Result, Error};
use crate::emulator::{Emulator, Error as EmulatorError};
use crate::core::mm::{VirtAddr, PhysAddr};
use crate::arch::common::MmioAccess;
use crate::utils::spinlock::SpinLock;
use core::sync::atomic::{AtomicU64, Ordering};

/// PL031 RTC registers
#[allow(dead_code)]
#[repr(usize)]
enum Pl031Register {
    Data = 0x00,
    MatchRegister = 0x04,
    LoadRegister = 0x08,
    ControlRegister = 0x0C,
    InterruptStatusRegister = 0x10,
    InterruptMaskRegister = 0x14,
    InterruptClearRegister = 0x18,
}

/// RTC time structure
#[derive(Debug, Clone, Copy)]
pub struct RtcTime {
    /// Seconds (0-59)
    pub seconds: u8,
    /// Minutes (0-59)
    pub minutes: u8,
    /// Hours (0-23)
    pub hours: u8,
    /// Day of month (1-31)
    pub day: u8,
    /// Month (1-12)
    pub month: u8,
    /// Year (full year, e.g., 2024)
    pub year: u16,
    /// Day of week (0-6, 0=Sunday)
    pub weekday: u8,
}

impl RtcTime {
    /// Get current time as Unix timestamp
    pub fn as_unix_timestamp(&self) -> u64 {
        // Simple conversion - not accounting for timezones or leap seconds
        let mut days = 0;

        // Add days from complete years
        for year in 1970..self.year {
            days += if is_leap_year(year) { 366 } else { 365 };
        }

        // Add days from complete months this year
        let month_days = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        for month in 1..self.month {
            days += month_days[month as usize - 1];
            if month == 2 && is_leap_year(self.year) {
                days += 1;
            }
        }

        // Add days from current month
        days += (self.day - 1) as u64;

        // Convert to seconds
        days * 86400 +
            self.hours as u64 * 3600 +
            self.minutes as u64 * 60 +
            self.seconds as u64
    }

    /// Create from Unix timestamp
    pub fn from_unix_timestamp(ts: u64) -> Self {
        let mut days = ts / 86400;
        let seconds = (ts % 86400) as u32;

        let hours = (seconds / 3600) as u8;
        let minutes = ((seconds % 3600) / 60) as u8;
        let secs = (seconds % 60) as u8;

        // Find year
        let mut year = 1970;
        while days >= if is_leap_year(year) { 366 } else { 365 } {
            days -= if is_leap_year(year) { 366 } else { 365 };
            year += 1;
        }

        // Find month and day
        let month_days = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        let mut month = 1;
        let mut day = (days + 1) as u8;

        while month <= 12 && day > month_days[month as usize - 1] {
            day -= month_days[month as usize - 1];
            if month == 2 && is_leap_year(year) {
                day -= 1;
            }
            month += 1;
        }

        // Calculate weekday (simplified)
        let weekday = ((days + 4) % 7) as u8; // Jan 1, 1970 was Thursday (4)

        Self {
            seconds: secs,
            minutes,
            hours,
            day,
            month,
            year,
            weekday,
        }
    }
}

/// Check if a year is a leap year
fn is_leap_year(year: u16) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// PL031 RTC state
#[derive(Debug, Clone)]
pub struct Pl031State {
    /// Current time (Unix timestamp)
    current_time: AtomicU64,
    /// Match register
    match_value: u32,
    /// Control register
    control: u32,
    /// Interrupt status
    int_status: u32,
    /// Interrupt mask
    int_mask: u32,
    /// RTC enabled
    enabled: bool,
}

/// PL031 RTC emulator
pub struct Pl031Rtc {
    /// Base address
    base_addr: PhysAddr,
    /// Device state
    state: SpinLock<Pl031State>,
    /// MMIO access interface
    mmio: MmioAccess,
    /// Reference time when RTC was initialized
    ref_time: u64,
}

impl Pl031Rtc {
    /// Create a new PL031 RTC emulator
    pub fn new(base_addr: PhysAddr) -> Self {
        // Get current time as reference
        let ref_time = crate::utils::get_timestamp();

        let state = Pl031State {
            current_time: AtomicU64::new(ref_time),
            match_value: 0,
            control: 0,
            int_status: 0,
            int_mask: 0,
            enabled: false,
        };

        Self {
            base_addr,
            state: SpinLock::new(state),
            mmio: MmioAccess,
            ref_time,
        }
    }

    /// Get the base address
    pub fn base_address(&self) -> PhysAddr {
        self.base_addr
    }

    /// Get current RTC time
    pub fn get_time(&self) -> RtcTime {
        let state = self.state.lock();
        let current_ts = state.current_time.load(Ordering::Relaxed);
        RtcTime::from_unix_timestamp(current_ts)
    }

    /// Set RTC time
    pub fn set_time(&self, time: &RtcTime) {
        let state = self.state.lock();
        state.current_time.store(time.as_unix_timestamp(), Ordering::Relaxed);
    }

    /// Update RTC (called periodically)
    pub fn update(&self) {
        let state = self.state.lock();
        if state.enabled {
            let current = crate::utils::get_timestamp();
            state.current_time.store(
                self.ref_time + (current - self.ref_time),
                Ordering::Relaxed
            );

            // Check for match
            let current_value = (state.current_time.load(Ordering::Relaxed) & 0xFFFFFFFF) as u32;
            if current_value == state.match_value && (state.int_mask & 0x01) != 0 {
                state.int_status = 0x01; // Set interrupt
            }
        }
    }
}

impl Emulator for Pl031Rtc {
    fn name(&self) -> &str {
        "PL031-RTC"
    }

    fn read(&self, offset: u64, size: u32) -> Result<u64, EmulatorError> {
        if size != 8 && size != 16 && size != 32 {
            return Err(EmulatorError::InvalidAccess);
        }

        let mut state = self.state.lock();
        let addr = offset as usize;

        let value = match addr {
            x if x == Pl031Register::Data as usize => {
                // Update current time
                self.update();
                let current_ts = state.current_time.load(Ordering::Relaxed);
                (current_ts & 0xFFFFFFFF) as u64
            }
            x if x == Pl031Register::MatchRegister as usize => state.match_value as u64,
            x if x == Pl031Register::ControlRegister as usize => state.control as u64,
            x if x == Pl031Register::InterruptStatusRegister as usize => state.int_status as u64,
            x if x == Pl031Register::InterruptMaskRegister as usize => state.int_mask as u64,
            _ => {
                crate::warn!("PL031: Unhandled read from offset 0x{:x}", addr);
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
        let byte_value = (value & 0xFFFFFFFF) as u32;

        match addr {
            x if x == Pl031Register::LoadRegister as usize => {
                // Load register - set new time
                state.current_time.store(byte_value as u64, Ordering::Relaxed);
                state.ref_time = crate::utils::get_timestamp();
            }
            x if x == Pl031Register::MatchRegister as usize => {
                state.match_value = byte_value;
            }
            x if x == Pl031Register::ControlRegister as usize => {
                state.control = byte_value;
                state.enabled = (byte_value & 0x01) != 0;
            }
            x if x == Pl031Register::InterruptMaskRegister as usize => {
                state.int_mask = byte_value & 0x01;
            }
            x if x == Pl031Register::InterruptClearRegister as usize => {
                if byte_value & 0x01 != 0 {
                    state.int_status &= !0x01;
                }
            }
            _ => {
                crate::warn!("PL031: Unhandled write 0x{:x} to offset 0x{:x}", value, addr);
            }
        }

        Ok(())
    }

    fn reset(&mut self) -> Result<(), EmulatorError> {
        let mut state = self.state.lock();

        // Reset to default state
        state.current_time.store(crate::utils::get_timestamp(), Ordering::Relaxed);
        state.match_value = 0;
        state.control = 0;
        state.int_status = 0;
        state.int_mask = 0;
        state.enabled = false;
        self.ref_time = crate::utils::get_timestamp();

        Ok(())
    }
}

/// MC146818-compatible RTC emulator
pub struct Mc146818Rtc {
    /// Base address
    base_addr: PhysAddr,
    /// Device state
    state: SpinLock<Mc146818State>,
    /// MMIO access interface
    mmio: MmioAccess,
}

/// MC146818 RTC state
#[derive(Debug, Clone)]
pub struct Mc146818State {
    /// RTC registers (64 bytes)
    regs: [u8; 64],
    /// Current index register
    index: u8,
    /// BCD mode flag
    bcd_mode: bool,
    /// 24-hour mode flag
    hour_24_mode: bool,
    /// Daylight saving enabled
    dst_enabled: bool,
}

impl Mc146818Rtc {
    /// Create a new MC146818 RTC emulator
    pub fn new(base_addr: PhysAddr) -> Self {
        let mut regs = [0u8; 64];
        let current_time = RtcTime::from_unix_timestamp(crate::utils::get_timestamp());

        // Initialize time registers (BCD format)
        regs[0] = to_bcd(current_time.seconds);     // Seconds
        regs[1] = to_bcd(current_time.minutes);     // Minutes
        regs[2] = to_bcd(current_time.hours);       // Hours
        regs[3] = to_bcd(current_time.weekday);     // Day of week
        regs[4] = to_bcd(current_time.day);         // Day of month
        regs[5] = to_bcd(current_time.month);       // Month
        regs[6] = to_bcd((current_time.year % 100) as u8); // Year (2 digits)

        // Initialize status registers
        regs[0x0A] = 0x20; // Update in progress
        regs[0x0B] = 0x82; // 24-hour mode, BCD mode

        Self {
            base_addr,
            state: SpinLock::new(Mc146818State {
                regs,
                index: 0,
                bcd_mode: true,
                hour_24_mode: true,
                dst_enabled: false,
            }),
            mmio: MmioAccess,
        }
    }

    /// Convert binary to BCD
    fn to_bcd(value: u8) -> u8 {
        ((value / 10) << 4) | (value % 10)
    }

    /// Convert BCD to binary
    fn from_bcd(bcd: u8) -> u8 {
        ((bcd >> 4) * 10) + (bcd & 0x0F)
    }
}

impl Emulator for Mc146818Rtc {
    fn name(&self) -> &str {
        "MC146818-RTC"
    }

    fn read(&self, offset: u64, size: u32) -> Result<u64, EmulatorError> {
        if size != 8 && size != 16 && size != 32 {
            return Err(EmulatorError::InvalidAccess);
        }

        let mut state = self.state.lock();
        let addr = offset as usize & 0x1; // 2-register window

        let value = if addr == 0 {
            // Index register
            state.index as u64
        } else {
            // Data register - read from indexed location
            if state.index < 64 {
                let reg_value = state.regs[state.index as usize];
                reg_value as u64
            } else {
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
        let addr = offset as usize & 0x1; // 2-register window
        let byte_value = (value & 0xFF) as u8;

        if addr == 0 {
            // Index register
            state.index = byte_value;
        } else {
            // Data register - write to indexed location
            if state.index < 64 {
                state.regs[state.index as usize] = byte_value;

                // Handle special registers
                match state.index {
                    0x0B => {
                        // Status Register B
                        state.bcd_mode = (byte_value & 0x04) == 0;
                        state.hour_24_mode = (byte_value & 0x02) == 0;
                    }
                    0x0C => {
                        // Status Register C (read-only)
                        // Do nothing
                    }
                    0x0D => {
                        // Status Register D (read-only)
                        // Do nothing
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    fn reset(&mut self) -> Result<(), EmulatorError> {
        let mut state = self.state.lock();

        // Reset registers to default
        let current_time = RtcTime::from_unix_timestamp(crate::utils::get_timestamp());

        state.regs[0] = to_bcd(current_time.seconds);
        state.regs[1] = to_bcd(current_time.minutes);
        state.regs[2] = to_bcd(current_time.hours);
        state.regs[3] = to_bcd(current_time.weekday);
        state.regs[4] = to_bcd(current_time.day);
        state.regs[5] = to_bcd(current_time.month);
        state.regs[6] = to_bcd((current_time.year % 100) as u8);

        state.regs[0x0A] = 0x20;
        state.regs[0x0B] = 0x82;
        state.regs[0x0C] = 0x00;
        state.regs[0x0D] = 0x80;

        state.index = 0;
        state.bcd_mode = true;
        state.hour_24_mode = true;
        state.dst_enabled = false;

        Ok(())
    }
}

/// Convert binary to BCD
fn to_bcd(value: u8) -> u8 {
    ((value / 10) << 4) | (value % 10)
}

/// Initialize RTC emulators
pub fn init() -> Result<(), crate::Error> {
    crate::info!("Initializing RTC emulators");

    // Register PL031 RTC
    let pl031 = Pl031Rtc::new(0x9010000);
    crate::emulator::register_emulator("rtc-pl031", &pl031)?;

    // Register MC146818 RTC
    let mc146818 = Mc146818Rtc::new(0x70);
    crate::emulator::register_emulator("rtc-mc146818", &mc146818)?;

    Ok(())
}