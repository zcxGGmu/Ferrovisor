//! Architecture-agnostic common utilities and traits

use core::default::Default;

/// Generic CPU context structure
#[derive(Debug, Clone, Copy)]
pub struct CpuContext {
    /// General-purpose registers (architecture-specific size)
    pub gpr: [u64; 32],
    /// Stack pointer
    pub sp: u64,
    /// Program counter
    pub pc: u64,
    /// Processor state
    pub pstate: u64,
}

impl Default for CpuContext {
    fn default() -> Self {
        Self {
            gpr: [0; 32],
            sp: 0,
            pc: 0,
            pstate: 0,
        }
    }
}

/// ARM64-specific CPU context
#[derive(Debug, Clone, Copy)]
pub struct Arm64Context {
    /// General-purpose registers X0-X30
    pub x: [u64; 31],
    /// Stack pointer
    pub sp: u64,
    /// Program counter
    pub pc: u64,
    /// Processor state
    pub pstate: u64,
    /// EL1 system registers (for VCPU)
    pub elr_el1: u64,
    pub spsr_el1: u64,
    pub sctlr_el1: u64,
    pub ttbr0_el1: u64,
    pub ttbr1_el1: u64,
}

impl Default for Arm64Context {
    fn default() -> Self {
        Self {
            x: [0; 31],
            sp: 0,
            pc: 0,
            pstate: 0,
            elr_el1: 0,
            spsr_el1: 0,
            sctlr_el1: 0,
            ttbr0_el1: 0,
            ttbr1_el1: 0,
        }
    }
}

/// RISC-V 64-bit specific CPU context
#[derive(Debug, Clone, Copy)]
pub struct Riscv64Context {
    /// General-purpose registers x1-x31
    pub x: [u64; 31],
    /// Stack pointer
    pub sp: u64,
    /// Program counter
    pub pc: u64,
    /// Supervisor status register
    pub sstatus: u64,
    /// Supervisor exception program counter
    pub sepc: u64,
    /// Supervisor trap cause
    pub scause: u64,
    /// Supervisor trap value
    pub stval: u64,
    /// Satp register (page table root)
    pub satp: u64,
}

impl Default for Riscv64Context {
    fn default() -> Self {
        Self {
            x: [0; 31],
            sp: 0,
            pc: 0,
            sstatus: 0,
            sepc: 0,
            scause: 0,
            stval: 0,
            satp: 0,
        }
    }
}

/// x86_64 specific CPU context
#[derive(Debug, Clone, Copy)]
pub struct X86_64Context {
    /// General-purpose registers (RAX, RBX, RCX, RDX, RSI, RDI, RBP, R8-R15)
    pub gpr: [u64; 16],
    /// Stack pointer
    pub rsp: u64,
    /// Base pointer
    pub rbp: u64,
    /// Instruction pointer
    pub rip: u64,
    /// RFLAGS register
    pub rflags: u64,
    /// Control registers
    pub cr0: u64,
    pub cr2: u64,
    pub cr3: u64,
    pub cr4: u64,
}

impl Default for X86_64Context {
    fn default() -> Self {
        Self {
            gpr: [0; 16],
            rsp: 0,
            rbp: 0,
            rip: 0,
            rflags: 0,
            cr0: 0,
            cr2: 0,
            cr3: 0,
            cr4: 0,
        }
    }
}

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