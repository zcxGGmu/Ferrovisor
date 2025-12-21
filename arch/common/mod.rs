//! Architecture-agnostic common utilities and traits

/// Memory-mapped I/O access trait
pub trait MmioAccess {
    /// Read an 8-bit value
    fn read_u8(&self, offset: usize) -> u8;

    /// Write an 8-bit value
    fn write_u8(&self, offset: usize, value: u8);

    /// Read a 16-bit value
    fn read_u16(&self, offset: usize) -> u16;

    /// Write a 16-bit value
    fn write_u16(&self, offset: usize, value: u16);

    /// Read a 32-bit value
    fn read_u32(&self, offset: usize) -> u32;

    /// Write a 32-bit value
    fn write_u32(&self, offset: usize, value: u32);

    /// Read a 64-bit value
    fn read_u64(&self, offset: usize) -> u64;

    /// Write a 64-bit value
    fn write_u64(&self, offset: usize, value: u64);
}

/// Simple MMIO region implementation
pub struct MmioRegion {
    base_address: usize,
}

impl MmioRegion {
    /// Create a new MMIO region
    pub const fn new(base_address: usize) -> Self {
        Self { base_address }
    }

    /// Get the base address
    pub const fn base_address(&self) -> usize {
        self.base_address
    }

    /// Calculate the address of an offset
    const fn address(&self, offset: usize) -> usize {
        self.base_address + offset
    }
}

impl MmioAccess for MmioRegion {
    fn read_u8(&self, offset: usize) -> u8 {
        unsafe {
            core::ptr::read_volatile(self.address(offset) as *const u8)
        }
    }

    fn write_u8(&self, offset: usize, value: u8) {
        unsafe {
            core::ptr::write_volatile(self.address(offset) as *mut u8, value);
        }
    }

    fn read_u16(&self, offset: usize) -> u16 {
        unsafe {
            core::ptr::read_volatile(self.address(offset) as *const u16)
        }
    }

    fn write_u16(&self, offset: usize, value: u16) {
        unsafe {
            core::ptr::write_volatile(self.address(offset) as *mut u16, value);
        }
    }

    fn read_u32(&self, offset: usize) -> u32 {
        unsafe {
            core::ptr::read_volatile(self.address(offset) as *const u32)
        }
    }

    fn write_u32(&self, offset: usize, value: u32) {
        unsafe {
            core::ptr::write_volatile(self.address(offset) as *mut u32, value);
        }
    }

    fn read_u64(&self, offset: usize) -> u64 {
        unsafe {
            core::ptr::read_volatile(self.address(offset) as *const u64)
        }
    }

    fn write_u64(&self, offset: usize, value: u64) {
        unsafe {
            core::ptr::write_volatile(self.address(offset) as *mut u64, value);
        }
    }
}

/// Architecture-specific initialization
pub fn init() -> Result<(), crate::Error> {
    crate::info!("Initializing common architecture utilities");
    Ok(())
}