//! CPU initialization for ARM64
//!
//! Handles CPU initialization including:
//! - EL2 mode setup
//! - System register configuration
//! - Cache configuration
//! - MMU enablement

use crate::{ExceptionLevel, el2_regs};

/// HCR_EL2 (Hypervisor Configuration Register) bits
pub mod hcr_el2 {
    /// VMID mask bits (VMID in VTTBR_EL2)
    pub const VMID: u64 = 0xFF << 0;
    /// PTW (Page Table Walk) - stage 2 page table walk in progress
    pub const PTW: u64 = 1 << 8;
    /// FMO (Fetch Override) - fetches from EL1/EL0 are Stage-2
    pub const FMO: u64 = 1 << 9;
    /// IMO (Instruction Override) - instructions from EL1/EL0 are Stage-2
    pub const IMO: u64 = 1 << 10;
    /// AMO (Aligned Memory Override) - Align accesses from EL1/EL0 are Stage-2
    pub const AMO: u64 = 1 << 11;
    /// FW (Foreign Walk) - walks from EL1/EL0 are Stage-2
    pub const FW: u64 = 1 << 12;
    /// DC (Default Cacheable) - Stage-1 cacheable translates to cacheable
    pub const DC: u64 = 1 << 12;
    /// TGE (Trap General Exceptions) - EL2 handles all exceptions from EL0/EL1
    pub const TGE: u64 = 1 << 27;
    /// TSW (Trap WFI)
    pub const TSW: u64 = 1 << 28;
    /// TWE (Trap WFE)
    pub const TWE: u64 = 1 << 29;
    /// TIDCP (Trap Implementation Defined)
    pub const TIDCP: u64 = 1 << 30;
    /// TAC (Trap Access to ACTLR)
    pub const TAC: u64 = 1u64 << 31;
    /// RW (Routing of WFI/WFE)
    pub const RW: u64 = 1 << 3;
    /// TRVM (Trap VM)
    pub const TRVM: u64 = 1 << 30;
    /// HCD (Hypervisor Call Disable)
    pub const HCD: u64 = 1 << 29;
    /// APU (Action for Unimplemented SGI)
    pub const APU: u64 = 1 << 14;
    /// APK (Authenticate Privileged instructions)
    pub const APK: u64 = 1 << 15;
    /// AT (Address Translate)
    pub const AT: u64 = 1 << 12;
    /// BSU (Barrier Synchronization Updates)
    pub const BSU: u64 = 0b11 << 4;
    /// FB (Force Broadcast)
    pub const FB: u64 = 1 << 7;
    /// VSE (Virtual SError Enable)
    pub const VSE: u64 = 1 << 26;
    /// VI (Virtual IRQ)
    pub const VI: u64 = 1 << 25;
    /// VF (Virtual FIQ)
    pub const VF: u64 = 1 << 24;
    /// E2H (EL2 Host) - VHE mode
    pub const E2H: u64 = 1 << 34;
    /// ID (Illegal Execution)
    pub const ID: u64 = 1 << 33;
    /// TL (Trap Lock)
    pub const TL: u64 = 1 << 32;
    /// MIO (Management I/O)
    pub const MIO: u64 = 1 << 2;
    /// TA (Trap Advanced SIMD/FP)
    pub const TA: u64 = 1 << 16;
}

/// VTCR_EL2 (Virtualization Translation Control Register) bits
pub mod vtcr_el2 {
    /// T0SZ (Translation Table Size 0) - VA size = 64 - T0SZ
    pub const T0SZ_SHIFT: u64 = 0;
    /// SL0 (Starting Level for Stage 2)
    pub const SL0_SHIFT: u64 = 6;
    /// IRGN0 (Inner Region Normal Memory)
    pub const IRGN0_SHIFT: u64 = 8;
    /// ORGN0 (Outer Region Normal Memory)
    pub const ORGN0_SHIFT: u64 = 10;
    /// SH0 (Shareability)
    pub const SH0_SHIFT: u64 = 12;
    /// TG0 (Translation Granule)
    pub const TG0_SHIFT: u64 = 14;
    /// PS (Physical Size)
    pub const PS_SHIFT: u64 = 16;

    /// Default values
    pub const T0SZ: u64 = 16; // 48-bit VA
    pub const SL0: u64 = 1;   // Starting at level 2
    pub const IRGN0_WBWA: u64 = 1; // Write-Back Write-Allocate
    pub const ORGN0_WBWA: u64 = 1;
    pub const SH0_ISH: u64 = 3; // Inner Shareable
    pub const TG0_4K: u64 = 0;  // 4KB granule
    pub const PS_48BIT: u64 = 2; // 48-bit PA
}

/// SCTLR_EL2 (System Control Register) bits
pub mod sctlr_el2 {
    /// M (MMU Enable)
    pub const M: u64 = 1 << 0;
    /// A (Alignment Check Enable)
    pub const A: u64 = 1 << 1;
    /// C (Cache Enable)
    pub const C: u64 = 1 << 2;
    /// SA (Stack Alignment Check Enable)
    pub const SA: u64 = 1 << 3;
    /// I (Instruction Cache Enable)
    pub const I: u64 = 1 << 12;
    /// WXN (Write Implies Execute-Not)
    pub const WXN: u64 = 1 << 19;
    /// EE (Exception Endianness)
    pub const EE: u64 = 1 << 25;
    /// EOS (Exception Entry is Context Synchronization)
    pub const EOS: u64 = 1 << 24;
    /// EIS (Exception Exit is Context Synchronization)
    pub const EIS: u64 = 1 << 23;
}

/// CPTR_EL2 (Architectural Feature Trap Register) bits
pub mod cptr_el2 {
    /// TFP (Trap FP/ASIMD)
    pub const TFP: u64 = 1 << 10;
    /// TCPAC (Trap EL2 and EL3 accesses to CPACR)
    pub const TCPAC: u64 = 1 << 31;
    /// TTA (Trap Access to Trace registers)
    pub const TTA: u64 = 1 << 20;
    /// TSM (Trap SVE)
    pub const TSM: u64 = 1 << 12;
}

/// HSTR_EL2 (Hypervisor System Trap Register) bits
pub mod hstr_el2 {
    /// Trap accesses to EL1/EL0
    pub const T_ELR: u64 = 1 << 3;
    pub const T_SP_EL1: u64 = 1 << 1;
    pub const T_SPSR_EL1: u64 = 1 << 2;
    pub const T_VBAR_EL1: u64 = 1 << 4;
}

/// Initialize EL2 mode
///
/// This function sets up the CPU for EL2 operation including:
/// - Configuring HCR_EL2 for virtualization
/// - Setting up VTCR_EL2 for Stage-2 translation
/// - Configuring SCTLR_EL2 for EL2
/// - Enabling caches
///
/// # Safety
/// Must be called at EL2 before entering guest OS
pub unsafe fn init_el2_mode() -> Result<(), &'static str> {
    log::info!("Initializing EL2 mode");

    // Read current EL to verify we're at EL2
    let el = super::super::current_exception_level();
    if el != ExceptionLevel::EL2 {
        return Err("Not running at EL2");
    }

    // Step 1: Configure HCR_EL2
    // Enable Stage-2 translation for instruction, data, and peripheral accesses
    let mut hcr = crate::cpu::regs::el2::read_hcr_el2();

    // Set VM, FMO, IMO, AMO for Stage-2 translation
    hcr |= hcr_el2::FMO | hcr_el2::IMO | hcr_el2::AMO;

    // Clear reserved bits
    hcr &= !(hcr_el2::RW | hcr_el2::CD); // Clear RW and CD for ARMv8

    crate::cpu::regs::el2::write_hcr_el2(hcr);

    // Step 2: Configure VTCR_EL2 for Stage-2 translation
    let vtcr = (vtcr_el2::T0SZ << vtcr_el2::T0SZ_SHIFT) |
               (vtcr_el2::SL0 << vtcr_el2::SL0_SHIFT) |
               (vtcr_el2::IRGN0_WBWA << vtcr_el2::IRGN0_SHIFT) |
               (vtcr_el2::ORGN0_WBWA << vtcr_el2::ORGN0_SHIFT) |
               (vtcr_el2::SH0_ISH << vtcr_el2::SH0_SHIFT) |
               (vtcr_el2::TG0_4K << vtcr_el2::TG0_SHIFT) |
               (vtcr_el2::PS_48BIT << vtcr_el2::PS_SHIFT);

    crate::cpu::regs::el2::write_vtcr_el2(vtcr);

    // Step 3: Configure SCTLR_EL2
    let mut sctlr = crate::cpu::regs::el2::read_sctlr_el2();

    // Enable MMU, caches, and alignment checks
    sctlr |= sctlr_el2::M | sctlr_el2::C | sctlr_el2::I;

    // Clear reserved bits and set safe defaults
    sctlr &= !(sctlr_el2::A | sctlr_el2::SA | sctlr_el2::WXN);

    crate::cpu::regs::el2::write_sctlr_el2(sctlr);

    // Step 4: Configure CPTR_EL2 to trap FP/ASIMD (lazy switching)
    let cptr = cptr_el2::TFP; // Trap FP/ASIMD for lazy switching
    crate::cpu::regs::el2::write_cptr_el2(cptr);

    // Step 5: Configure HSTR_EL2 (clear all traps by default)
    crate::cpu::regs::el2::write_hstr_el2(0);

    log::debug!("EL2 mode initialized: HCR={:#x}, VTCR={:#x}, SCTLR={:#x}",
                hcr, vtcr, sctlr);

    Ok(())
}

/// Setup exception vector table at EL2
///
/// # Safety
/// Must be called from EL2
pub unsafe fn setup_exception_vectors(vectors: *const u8) -> Result<(), &'static str> {
    // Write VBAR_EL2 with the vector table address
    let vbar_el2 = vectors as u64;

    core::arch::asm!(
        "msr vbar_el2, {vbar}",
        vbar = in(reg) vbar_el2,
    );

    log::debug!("Exception vectors set to {:#x}", vbar_el2);

    Ok(())
}

/// Initialize CPU for ARM64
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing ARM64 CPU");

    // Detect CPU features first
    super::features::detect();

    // Check if we're at EL2
    let el = super::super::current_exception_level();
    log::info!("Current exception level: {:?}", el);

    if el == ExceptionLevel::EL2 {
        // Initialize EL2 mode
        unsafe {
            init_el2_mode()?;
        }
    } else {
        log::warn!("Not running at EL2, virtualization will not work properly");
    }

    log::info!("ARM64 CPU initialized successfully");
    Ok(())
}

/// CPU initialization early boot code
///
/// This is typically called from assembly entry point
#[no_mangle]
pub extern "C" fn cpu_init_early() {
    log::info!("CPU early initialization");

    // Very early initialization before any Rust code
    // This would typically be in assembly

    log::debug!("CPU early init complete");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hcr_el2_bits() {
        assert_eq!(hcr_el2::VMID, 0xFF);
        assert_eq!(hcr_el2::FMO, 1 << 9);
        assert_eq!(hcr_el2::IMO, 1 << 10);
        assert_eq!(hcr_el2::AMO, 1 << 11);
    }

    #[test]
    fn test_vtcr_el2_bits() {
        assert_eq!(vtcr_el2::T0SZ_SHIFT, 0);
        assert_eq!(vtcr_el2::SL0_SHIFT, 6);
        assert_eq!(vtcr_el2::IRGN0_SHIFT, 8);
        assert_eq!(vtcr_el2::TG0_4K, 0);
        assert_eq!(vtcr_el2::TG0_SHIFT, 14);
    }

    #[test]
    fn test_sctlr_el2_bits() {
        assert_eq!(sctlr_el2::M, 1 << 0);
        assert_eq!(sctlr_el2::A, 1 << 1);
        assert_eq!(sctlr_el2::C, 1 << 2);
        assert_eq!(sctlr_el2::I, 1 << 12);
    }

    #[test]
    fn test_cptr_el2_bits() {
        assert_eq!(cptr_el2::TFP, 1 << 10);
        assert_eq!(cptr_el2::TCPAC, 1 << 31);
        assert_eq!(cptr_el2::TTA, 1 << 20);
    }
}
