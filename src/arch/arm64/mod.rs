//! ARM64 architecture support
//!
//! This module provides ARM64-specific implementations for the hypervisor.

use crate::{Result, Error};
use crate::arch::common::{CpuFeatures, Arm64Context};

/// Initialize ARM64-specific components
pub fn init() -> Result<()> {
    // Initialize VTCR_EL2 for stage-2 translation
    init_vtcr();

    // Initialize HCR_EL2 for hypervisor configuration
    init_hcr();

    // Initialize timer
    init_timer();

    // Initialize GIC
    init_gic();

    Ok(())
}

/// Initialize the virtualization control register
fn init_vtcr() {
    unsafe {
        // Configure VTCR_EL2 for 4KB pages, 48-bit VA/PA
        let vtcr = (0b0u64 << 31) |     // VS: 0 = not using 16-bit VMID
                   (0u64 << 30) |        // IRGN1: 0 = Normal, Inner Write-Back
                   (0u64 << 26) |        // ORGN1: 0 = Normal, Outer Write-Back
                   (0u64 << 24) |        // SH0: 0 = Non-shareable
                   (0b00u64 << 14) |     // TG0: 00 = 4KB granule
                   (0b0000u64 << 6) |    // SL0: 0000 = 4KB level 0
                   (0u64 << 4) |         // T0SZ: 0 = 64-bit VA (we use 48-bit in practice)
                   (1u64 << 7) |         // HA: Hardware Access flag update
                   (0u64 << 8) |         // HD: Hardware Dirty flag update
                   (0u64);               // DS: 0 = Disable stage-1 translation for EL0/EL1

        core::arch::asm!("msr vtcr_el2, {}", in(reg) vtcr);
    }
}

/// Initialize the hypervisor configuration register
fn init_hcr() {
    unsafe {
        // Configure HCR_EL2
        let hcr = (1u64 << 31) |        // RW: 1 = 64-bit EL2
                  (1u64 << 28) |        // IMO: 1 = IRQs routed to EL2
                  (1u64 << 27) |        // FMO: 1 = FIQs routed to EL2
                  (1u64 << 1) |         // TGE: 1 = Trap general exceptions
                  (0u64 << 0);          // VM: 0 = Stage-2 disabled initially

        core::arch::asm!("msr hcr_el2, {}", in(reg) hcr);
    }
}

/// Initialize the timer
fn init_timer() {
    unsafe {
        // Enable CNTP timer access at EL1
        let cntkctl: u64;
        core::arch::asm!("mrs {}, cntkctl_el1", out(reg) cntkctl);
        core::arch::asm!("msr cntkctl_el1, {}", in(reg) cntkctl | 0x3);

        // Configure timer frequency
        let cntfrq: u64;
        core::arch::asm!("mrs {}, cntfrq_el0", out(reg) cntfrq);
        crate::info!("Timer frequency: {} Hz", cntfrq);
    }
}

/// Initialize the GIC (Generic Interrupt Controller)
fn init_gic() {
    // TODO: Initialize GICv2 or GICv3 based on what's available
    crate::info!("GIC initialization skipped (TODO)");
}

/// Get ARM64 CPU features
pub fn get_cpu_features() -> crate::arch::common::CpuFeatures {
    let mut features = crate::arch::common::CpuFeatures::default();

    unsafe {
        // Check ID_AA64PFR0_EL1 for EL2 (virtualization)
        let pfr0: u64;
        core::arch::asm!("mrs {}, id_aa64pfr0_el1", out(reg) pfr0);
        let el2 = (pfr0 >> 12) & 0xF;
        features.virtualization = el2 == 0b0001;

        // Check for VHE (Virtualization Host Extensions)
        let pfr1: u64;
        core::arch::asm!("mrs {}, id_aa64pfr0_el1", out(reg) pfr1);
        let vhe = (pfr1 >> 8) & 0xF;
        features.vhe = vhe == 0b0001;

        // Check ID_AA64MMFR0_EL1 for physical address size
        let mmfr0: u64;
        core::arch::asm!("mrs {}, id_aa64mmfr0_el1", out(reg) mmfr0);
        let pa_range = (mmfr0 >> 0) & 0xF;
        features.large_pages = true; // ARM64 supports various page sizes

        // SLAT is always present with virtualization
        features.slat = features.virtualization;
    }

    features
}

/// Enter EL2 (hypervisor mode)
pub fn enter_el2() {
    // This would typically be called from bootloader
    unsafe {
        // Check current EL
        let current_el: u64;
        core::arch::asm!("mrs {}, currentel", out(reg) current_el);
        let el = (current_el >> 2) & 0x3;

        if el == 2 {
            crate::info!("Already in EL2");
            return;
        }

        // TODO: Implement EL2 entry from lower EL
        crate::warn!("EL2 entry not implemented");
    }
}

/// Save current CPU context
pub fn save_context(context: &mut crate::arch::common::CpuContext) {
    unsafe {
        // Save general purpose registers
        core::arch::asm!(
            "str x0, [{}]",
            "str x1, [{}]",
            "str x2, [{}]",
            "str x3, [{}]",
            "str x4, [{}]",
            "str x5, [{}]",
            "str x6, [{}]",
            "str x7, [{}]",
            "str x8, [{}]",
            "str x9, [{}]",
            "str x10, [{}]",
            "str x11, [{}]",
            "str x12, [{}]",
            "str x13, [{}]",
            "str x14, [{}]",
            "str x15, [{}]",
            "str x16, [{}]",
            "str x17, [{}]",
            "str x18, [{}]",
            "str x19, [{}]",
            "str x20, [{}]",
            "str x21, [{}]",
            "str x22, [{}]",
            "str x23, [{}]",
            "str x24, [{}]",
            "str x25, [{}]",
            "str x26, [{}]",
            "str x27, [{}]",
            "str x28, [{}]",
            "str x29, [{}]",
            "str x30, [{}]",
            in(reg) &mut context.gpr[0],
            in(reg) &mut context.gpr[1],
            in(reg) &mut context.gpr[2],
            in(reg) &mut context.gpr[3],
            in(reg) &mut context.gpr[4],
            in(reg) &mut context.gpr[5],
            in(reg) &mut context.gpr[6],
            in(reg) &mut context.gpr[7],
            in(reg) &mut context.gpr[8],
            in(reg) &mut context.gpr[9],
            in(reg) &mut context.gpr[10],
            in(reg) &mut context.gpr[11],
            in(reg) &mut context.gpr[12],
            in(reg) &mut context.gpr[13],
            in(reg) &mut context.gpr[14],
            in(reg) &mut context.gpr[15],
            in(reg) &mut context.gpr[16],
            in(reg) &mut context.gpr[17],
            in(reg) &mut context.gpr[18],
            in(reg) &mut context.gpr[19],
            in(reg) &mut context.gpr[20],
            in(reg) &mut context.gpr[21],
            in(reg) &mut context.gpr[22],
            in(reg) &mut context.gpr[23],
            in(reg) &mut context.gpr[24],
            in(reg) &mut context.gpr[25],
            in(reg) &mut context.gpr[26],
            in(reg) &mut context.gpr[27],
            in(reg) &mut context.gpr[28],
            in(reg) &mut context.gpr[29],
            in(reg) &mut context.gpr[30],
        );

        // Save PC and SP
        core::arch::asm!(
            "adr {}, 1f",
            "mov {}, sp",
            "b 2f",
            "1:",
            "2:",
            out(reg) context.pc,
            out(reg) context.sp,
        );

        // Save PSTATE
        let pstate: u64;
        core::arch::asm!("mrs {}, daif", out(reg) pstate);
        context.psr = pstate;

        // Save ARM64 specific registers
        let arm64_context = unsafe { &mut context.arch_specific.arm64 };
        core::arch::asm!("mrs {}, tpidr_el0", out(reg) arm64_context.tpidr_el0);
        core::arch::asm!("mrs {}, tpidr_el1", out(reg) arm64_context.tpidr_el1);
        core::arch::asm!("mrs {}, vbar_el1", out(reg) arm64_context.vbar_el1);
        core::arch::asm!("mrs {}, sctlr_el1", out(reg) arm64_context.sctlr_el1);
        core::arch::asm!("mrs {}, tcr_el1", out(reg) arm64_context.tcr_el1);
        core::arch::asm!("mrs {}, ttbr0_el1", out(reg) arm64_context.ttbr0_el1);
        core::arch::asm!("mrs {}, ttbr1_el1", out(reg) arm64_context.ttbr1_el1);
    }
}

/// Restore CPU context
pub fn restore_context(context: &crate::arch::common::CpuContext) {
    unsafe {
        // Restore ARM64 specific registers
        let arm64_context = &context.arch_specific.arm64;
        core::arch::asm!("msr tpidr_el0, {}", in(reg) arm64_context.tpidr_el0);
        core::arch::asm!("msr tpidr_el1, {}", in(reg) arm64_context.tpidr_el1);
        core::arch::asm!("msr vbar_el1, {}", in(reg) arm64_context.vbar_el1);
        core::arch::asm!("msr sctlr_el1, {}", in(reg) arm64_context.sctlr_el1);
        core::arch::asm!("msr tcr_el1, {}", in(reg) arm64_context.tcr_el1);
        core::arch::asm!("msr ttbr0_el1, {}", in(reg) arm64_context.ttbr0_el1);
        core::arch::asm!("msr ttbr1_el1, {}", in(reg) arm64_context.ttbr1_el1);

        // Restore general purpose registers
        core::arch::asm!(
            "ldr x0, [{}]",
            "ldr x1, [{}]",
            "ldr x2, [{}]",
            "ldr x3, [{}]",
            "ldr x4, [{}]",
            "ldr x5, [{}]",
            "ldr x6, [{}]",
            "ldr x7, [{}]",
            "ldr x8, [{}]",
            "ldr x9, [{}]",
            "ldr x10, [{}]",
            "ldr x11, [{}]",
            "ldr x12, [{}]",
            "ldr x13, [{}]",
            "ldr x14, [{}]",
            "ldr x15, [{}]",
            "ldr x16, [{}]",
            "ldr x17, [{}]",
            "ldr x18, [{}]",
            "ldr x19, [{}]",
            "ldr x20, [{}]",
            "ldr x21, [{}]",
            "ldr x22, [{}]",
            "ldr x23, [{}]",
            "ldr x24, [{}]",
            "ldr x25, [{}]",
            "ldr x26, [{}]",
            "ldr x27, [{}]",
            "ldr x28, [{}]",
            "ldr x29, [{}]",
            "ldr x30, [{}]",
            in(reg) &context.gpr[0],
            in(reg) &context.gpr[1],
            in(reg) &context.gpr[2],
            in(reg) &context.gpr[3],
            in(reg) &context.gpr[4],
            in(reg) &context.gpr[5],
            in(reg) &context.gpr[6],
            in(reg) &context.gpr[7],
            in(reg) &context.gpr[8],
            in(reg) &context.gpr[9],
            in(reg) &context.gpr[10],
            in(reg) &context.gpr[11],
            in(reg) &context.gpr[12],
            in(reg) &context.gpr[13],
            in(reg) &context.gpr[14],
            in(reg) &context.gpr[15],
            in(reg) &context.gpr[16],
            in(reg) &context.gpr[17],
            in(reg) &context.gpr[18],
            in(reg) &context.gpr[19],
            in(reg) &context.gpr[20],
            in(reg) &context.gpr[21],
            in(reg) &context.gpr[22],
            in(reg) &context.gpr[23],
            in(reg) &context.gpr[24],
            in(reg) &context.gpr[25],
            in(reg) &context.gpr[26],
            in(reg) &context.gpr[27],
            in(reg) &context.gpr[28],
            in(reg) &context.gpr[29],
            in(reg) &context.gpr[30],
        );

        // Restore SP and jump to PC
        core::arch::asm!(
            "mov sp, {}",
            "msr daif, {}",
            "br {}",
            in(reg) context.sp,
            in(reg) context.psr,
            in(reg) context.pc,
        );
    }
}

/// Architecture-specific main loop
pub fn run() -> ! {
    crate::info!("ARM64 hypervisor running");

    loop {
        // Main hypervisor loop
        cortex_a::asm::wfi();
    }
}

/// Panic handler for ARM64
pub fn panic(info: &core::panic::PanicInfo) -> ! {
    crate::error!("ARM64 Panic: {}", info);

    // Output panic info via UART
    if let Some(location) = info.location() {
        crate::error!("  at {}:{}:{}", location.file(), location.line(), location.column());
    }

    if let Some(message) = info.message() {
        crate::error!("  message: {}", message);
    }

    // Halt the system
    loop {
        cortex_a::asm::wfe();
    }
}

/// Exception personality routine
pub extern "C" fn eh_personality() {
    // Rust exception handling - not used in bare metal
}