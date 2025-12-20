//! RISC-V 64-bit architecture support
//!
//! This module provides RISC-V-specific implementations for the hypervisor.

use crate::{Result, Error};
use crate::arch::common::{CpuFeatures, Riscv64Context};

/// Initialize RISC-V-specific components
pub fn init() -> Result<()> {
    // Initialize H-extension if available
    init_hextension();

    // Initialize timer
    init_timer();

    // Initialize CLIC (Core Local Interrupt Controller)
    init_clic();

    Ok(())
}

/// Initialize RISC-V Hypervisor extension
fn init_hextension() {
    unsafe {
        // Check if H extension is available
        let misa: u64;
        core::arch::asm!("csrr {}, misa", out(reg) misa);

        let has_h_ext = (misa >> ('H' as u64 - 'A' as u64)) & 0x1 != 0;
        if !has_h_ext {
            crate::error!("RISC-V Hypervisor extension not available!");
            return;
        }

        // Configure HIDELEG to delegate some traps to HS mode
        let hideleg: u64 = (1 << 5) |  // Timer interrupt
                           (1 << 9) |  // External interrupt
                           (1 << 15);  // Store/AMO page fault
        core::arch::asm!("csrw hideleg, {}", in(reg) hideleg);

        // Configure HEDELEG to delegate some exceptions to HS mode
        let hedeleg: u64 = (1 << 0) |  // Instruction address misaligned
                           (1 << 1) |  // Instruction access fault
                           (1 << 3) |  // Breakpoint
                           (1 << 5) |  // Load access fault
                           (1 << 7) |  // Store/AMO access fault
                           (1 << 13);  // Instruction page fault
        core::arch::asm!("csrw hedeleg, {}", in(reg) hedeleg);

        // Enable virtualization in mstatus
        let mut mstatus: u64;
        core::arch::asm!("csrr {}, mstatus", out(reg) mstatus);
        mstatus |= (1 << 20);  // Set MPV bit
        mstatus &= !(1 << 17);  // Clear MPP to 0 (U-mode) for guest
        core::arch::asm!("csrw mstatus, {}", in(reg) mstatus);

        // Set VS mode to S-mode
        let mut hstatus: u64;
        core::arch::asm!("csrr {}, hstatus", out(reg) hstatus);
        hstatus |= (1 << 5);  // Set VTSR bit
        hstatus |= (1 << 6);  // Set VTVM bit
        hstatus &= !(1 << 7);  // Clear VGEIN (no guest external interrupts)
        core::arch::asm!("csrw hstatus, {}", in(reg) hstatus);
    }

    crate::info!("RISC-V H-extension initialized");
}

/// Initialize the timer
fn init_timer() {
    unsafe {
        // Get timer frequency
        #[cfg(target_pointer_width = "64")]
        let timebase_frequency = riscv::register::time::read();

        #[cfg(not(target_pointer_width = "64"))]
        let timebase_frequency = {
            // For 32-bit systems, need to read from device tree
            10000000 // Default 10MHz
        };

        crate::info!("Timer frequency: {} Hz", timebase_frequency);
    }
}

/// Initialize CLIC (Core Local Interrupt Controller)
fn init_clic() {
    // TODO: Initialize CLIC if available
    crate::info!("CLIC initialization skipped (TODO)");
}

/// Get RISC-V CPU features
pub fn get_cpu_features() -> crate::arch::common::CpuFeatures {
    let mut features = crate::arch::common::CpuFeatures::default();

    unsafe {
        // Check MISA for extensions
        let misa: u64;
        core::arch::asm!("csrr {}, misa", out(reg) misa);

        // Check for H extension (hypervisor)
        let has_h_ext = (misa >> ('H' as u64 - 'A' as u64)) & 0x1 != 0;
        features.virtualization = has_h_ext;

        // Check for G extension (floating point)
        let has_g_ext = (misa >> ('G' as u64 - 'A' as u64)) & 0x1 != 0;

        // SLAT requires G stage page tables
        features.slat = has_h_ext;

        // RISC-V supports large pages if Sv39 or Sv48 is implemented
        features.large_pages = true;

        // Performance counters require H extension
        features.perf_counters = has_h_ext;
    }

    features
}

/// Enter M-mode (machine mode) or HS-mode
pub fn enter_hypervisor_mode() {
    unsafe {
        // Check current privilege level
        let mstatus: u64;
        core::arch::asm!("csrr {}, mstatus", out(reg) mstatus);
        let mpp = (mstatus >> 11) & 0x3;

        if mpp == 3 {
            crate::info!("Already in M-mode");
        } else {
            crate::warn!("Not in M-mode, hypervisor may not work correctly");
        }
    }
}

/// Save current CPU context
pub fn save_context(context: &mut crate::arch::common::CpuContext) {
    unsafe {
        // Save general purpose registers
        core::arch::asm!(
            "sd x0, 0({})",
            "sd x1, 8({})",
            "sd x2, 16({})",
            "sd x3, 24({})",
            "sd x4, 32({})",
            "sd x5, 40({})",
            "sd x6, 48({})",
            "sd x7, 56({})",
            "sd x8, 64({})",
            "sd x9, 72({})",
            "sd x10, 80({})",
            "sd x11, 88({})",
            "sd x12, 96({})",
            "sd x13, 104({})",
            "sd x14, 112({})",
            "sd x15, 120({})",
            "sd x16, 128({})",
            "sd x17, 136({})",
            "sd x18, 144({})",
            "sd x19, 152({})",
            "sd x20, 160({})",
            "sd x21, 168({})",
            "sd x22, 176({})",
            "sd x23, 184({})",
            "sd x24, 192({})",
            "sd x25, 200({})",
            "sd x26, 208({})",
            "sd x27, 216({})",
            "sd x28, 224({})",
            "sd x29, 232({})",
            "sd x30, 240({})",
            "sd x31, 248({})",
            in(reg) &mut context.gpr[0],
        );

        // Save PC (stored in mepc or sepc depending on mode)
        let pc: u64;
        core::arch::asm!("csrr {}, mepc", out(reg) pc);
        context.pc = pc;

        // Save SP (x2)
        context.sp = context.gpr[2];

        // Save status register
        let status: u64;
        core::arch::asm!("csrr {}, mstatus", out(reg) status);
        context.psr = status;

        // Save RISC-V specific registers
        let riscv_context = unsafe { &mut context.arch_specific.riscv64 };
        core::arch::asm!("csrr {}, satp", out(reg) riscv_context.satp);
        core::arch::asm!("csrr {}, sstatus", out(reg) riscv_context.sstatus);
        core::arch::asm!("csrr {}, sie", out(reg) riscv_context.sie);
        core::arch::asm!("csrr {}, stvec", out(reg) riscv_context.stvec);
        core::arch::asm!("csrr {}, sscratch", out(reg) riscv_context.sscratch);
        core::arch::asm!("csrr {}, sepc", out(reg) riscv_context.sepc);
        core::arch::asm!("csrr {}, scause", out(reg) riscv_context.scause);
        core::arch::asm!("csrr {}, stval", out(reg) riscv_context.stval);
    }
}

/// Restore CPU context
pub fn restore_context(context: &crate::arch::common::CpuContext) {
    unsafe {
        // Restore RISC-V specific registers
        let riscv_context = &context.arch_specific.riscv64;
        core::arch::asm!("csrw satp, {}", in(reg) riscv_context.satp);
        core::arch::asm!("csrw sstatus, {}", in(reg) riscv_context.sstatus);
        core::arch::asm!("csrw sie, {}", in(reg) riscv_context.sie);
        core::arch::asm!("csrw stvec, {}", in(reg) riscv_context.stvec);
        core::arch::asm!("csrw sscratch, {}", in(reg) riscv_context.sscratch);
        core::arch::asm!("csrw sepc, {}", in(reg) riscv_context.sepc);
        core::arch::asm!("csrw scause, {}", in(reg) riscv_context.scause);
        core::arch::asm!("csrw stval, {}", in(reg) riscv_context.stval);

        // Restore status register
        core::arch::asm!("csrw mstatus, {}", in(reg) context.psr);

        // Restore general purpose registers
        core::arch::asm!(
            "ld x0, 0({})",
            "ld x1, 8({})",
            "ld x2, 16({})",
            "ld x3, 24({})",
            "ld x4, 32({})",
            "ld x5, 40({})",
            "ld x6, 48({})",
            "ld x7, 56({})",
            "ld x8, 64({})",
            "ld x9, 72({})",
            "ld x10, 80({})",
            "ld x11, 88({})",
            "ld x12, 96({})",
            "ld x13, 104({})",
            "ld x14, 112({})",
            "ld x15, 120({})",
            "ld x16, 128({})",
            "ld x17, 136({})",
            "ld x18, 144({})",
            "ld x19, 152({})",
            "ld x20, 160({})",
            "ld x21, 168({})",
            "ld x22, 176({})",
            "ld x23, 184({})",
            "ld x24, 192({})",
            "ld x25, 200({})",
            "ld x26, 208({})",
            "ld x27, 216({})",
            "ld x28, 224({})",
            "ld x29, 232({})",
            "ld x30, 240({})",
            "ld x31, 248({})",
            in(reg) &context.gpr[0],
        );

        // Restore PC and return
        core::arch::asm!("csrw mepc, {}", in(reg) context.pc);
        core::arch::asm!("mret");
    }
}

/// Architecture-specific main loop
pub fn run() -> ! {
    crate::info!("RISC-V hypervisor running");

    loop {
        // Main hypervisor loop
        riscv::asm::wfi();
    }
}

/// Panic handler for RISC-V
pub fn panic(info: &core::panic::PanicInfo) -> ! {
    crate::error!("RISC-V Panic: {}", info);

    // Output panic info via UART
    if let Some(location) = info.location() {
        crate::error!("  at {}:{}:{}", location.file(), location.line(), location.column());
    }

    if let Some(message) = info.message() {
        crate::error!("  message: {}", message);
    }

    // Halt the system
    loop {
        riscv::asm::wfi();
    }
}

/// Exception personality routine
pub extern "C" fn eh_personality() {
    // Rust exception handling - not used in bare metal
}