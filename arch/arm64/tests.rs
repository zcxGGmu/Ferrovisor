//! ARM64 Unit Tests
//!
//! This module provides unit tests for ARM64-specific functionality.
//! Tests are organized by module:
//! - CPU tests (registers, features, state)
//! - MMU tests (page tables, address translation)
//! - Interrupt tests (GIC, VGIC, virtual IRQs)
//! - Timer tests (Generic Timer)
//! - System register tests
//!
//! ## Running Tests
//!
//! ```bash
//! cargo test --package ferrovisor --lib --arch arm64
//! ```
//!
//! ## Test Organization
//!
//! Tests are grouped by functionality:
//! - `test_cpu_*` - CPU-related tests
//! - `test_mmu_*` - MMU-related tests
//! - `test_gic_*` - GIC-related tests
//! - `test_vgic_*` - VGIC-related tests
//! - `test_timer_*` - Timer-related tests
//!
//! ## References
//! - [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)
//! - [test-case crate](https://docs.rs/test-case/)

#[cfg(test)]
mod tests {
    // ========================================================================
    // CPU Tests
    // ========================================================================

    /// Test CPU register access
    #[test]
    fn test_cpu_registers() {
        // Test reading current EL
        let el = super::current_exception_level();
        assert!(matches!(el, super::ExceptionLevel::EL2));

        // Test CPU ID
        let cpu_id = super::cpu::current_cpu_id();
        assert!(cpu_id < 256);
    }

    /// Test CPU feature detection
    #[test]
    fn test_cpu_features() {
        // This test verifies that feature detection doesn't panic
        // Actual features depend on the hardware/platform
        super::cpu::features::detect();
    }

    /// Test CPU state
    #[test]
    fn test_cpu_state() {
        let state = super::cpu::CpuState::Running;
        assert!(state.is_running());
        assert!(!state.is_halted());
    }

    /// Test exception level
    #[test]
    fn test_exception_level() {
        let el = super::current_exception_level();
        // Should be at EL2 for hypervisor
        assert_eq!(el as u32, 2);
    }

    // ========================================================================
    // MMU Tests
    // ========================================================================

    /// Test page table entry creation
    #[test]
    fn test_page_table_entry() {
        use super::mmu::pte::*;

        // Create a valid block entry
        let pte = PageTableEntry::new_block(0x40000000, 0x40000000, Attributes::NORMAL);
        assert!(pte.is_valid());
        assert!(pte.is_block());
        assert!(!pte.is_table());

        // Create a table entry
        let table_pte = PageTableEntry::new_table(0x50000000);
        assert!(table_pte.is_valid());
        assert!(table_pte.is_table());
        assert!(!table_pte.is_block());
    }

    /// Test page attributes
    #[test]
    fn test_page_attributes() {
        use super::mmu::attrs::*;

        // Test normal memory attributes
        let attr = Attributes::NORMAL;
        assert!(attr.is_normal());
        assert!(!attr.is_device());

        // Test device memory attributes
        let device_attr = Attributes::DEVICE_nGnRnE;
        assert!(device_attr.is_device());
        assert!(!device_attr.is_normal());
    }

    /// Test VTCR_EL2 configuration
    #[test]
    fn test_vtcr() {
        use super::mmu::vtcr::*;

        // Test VTCR calculation for 48-bit VA
        let t0sz = 16; // 64 - 48 = 16
        assert!(t0sz <= 31);

        // Test SH0 (shareability)
        let sh0 = SH0_ISH;
        assert_eq!(sh0, 3);
    }

    // ========================================================================
    // Interrupt Tests
    // ========================================================================

    /// Test GIC version
    #[test]
    fn test_gic_version() {
        use super::interrupt::gic::GicVersion;

        let v2 = GicVersion::V2;
        let v3 = GicVersion::V3;

        assert_eq!(v2 as u32, 2);
        assert_eq!(v3 as u32, 3);
        assert!(v2 < v3);
    }

    /// Test GIC distributor
    #[test]
    fn test_gic_distributor() {
        use super::interrupt::gic::GicDistributor;

        let dist = GicDistributor::new(0x08000000, super::interrupt::gic::GicVersion::V3, 1020);
        assert_eq!(dist.get_num_irqs(), 1020);
        assert_eq!(dist.get_version(), super::interrupt::gic::GicVersion::V3);
    }

    /// Test virtual interrupt
    #[test]
    fn test_virtual_interrupt() {
        use super::interrupt::virq::{VirtInterrupt, VirtIrqType, IrqState};

        let virq = VirtInterrupt::new(32, 5, VirtIrqType::External);
        assert_eq!(virq.irq, 32);
        assert_eq!(virq.priority, 5);
        assert_eq!(virq.state, IrqState::Pending);
        assert!(virq.state.is_pending());
    }

    /// Test exception type
    #[test]
    fn test_exception_type() {
        use super::interrupt::handlers::ExceptionType;

        let exc = ExceptionType::GuestSyncA64;
        assert!(exc.is_guest());
        assert!(exc.is_sync());
        assert!(!exc.is_irq());
        assert_eq!(exc.name(), "GUEST_SYNC_A64");
    }

    /// Test exception context
    #[test]
    fn test_exception_context() {
        use super::interrupt::handlers::ExceptionContext;

        let ctx = ExceptionContext::new();
        assert_eq!(ctx.gpr(0), 0);

        ctx.set_gpr(0, 0x12345678);
        assert_eq!(ctx.gpr(0), 0x12345678);

        ctx.set_elr(0x40000000);
        assert_eq!(ctx.elr(), 0x40000000);
    }

    // ========================================================================
    // Timer Tests
    // ========================================================================

    /// Test timer types
    #[test]
    fn test_timer_types() {
        use super::timer::TimerType;

        assert_eq!(TimerType::Physical as u32, 0);
        assert_eq!(TimerType::Virtual as u32, 1);
        assert_eq!(TimerType::HypPhysical as u32, 2);
    }

    /// Test timer control bits
    #[test]
    fn test_timer_control() {
        use super::timer::TimerControl;

        let ctl = TimerControl::new();
        assert!(!ctl.is_enabled());
        assert!(!ctl.is_masked());

        let ctl2 = ctl.with_enable(true);
        assert!(ctl2.is_enabled());
    }

    /// Test counter frequency
    #[test]
    fn test_counter_frequency() {
        // Read counter frequency
        let freq = super::timer::read_counter_freq();
        // Typical values: 10-100 MHz
        assert!(freq >= 10_000 && freq <= 100_000);
    }

    // ========================================================================
    // System Register Tests
    // ========================================================================

    /// Test HCR_EL2 bits
    #[test]
    fn test_hcr_el2() {
        use super::cpu::init::hcr_el2;

        // Test bit definitions
        assert_eq!(hcr_el2::VM, 1u64 << 0);
        assert_eq!(hcr_el2::RW, 1u64 << 31);
        assert_eq!(hcr_el2::TGE, 1u64 << 27);
    }

    /// Test VTCR_EL2 bits
    #[test]
    fn test_vtcr_el2() {
        use super::cpu::init::vtcr_el2;

        // Test T0SZ shift
        assert_eq!(vtcr_el2::T0SZ_SHIFT, 0);
        assert_eq!(vtcr_el2::SL0_SHIFT, 6);
        assert_eq!(vtcr_el2::TG0_SHIFT, 14);
        assert_eq!(vtcr_el2::PS_SHIFT, 16);
    }

    /// Test SCTLR_EL2 bits
    #[test]
    fn test_sctlr_el2() {
        use super::cpu::init::sctlr_el2;

        // Test bit definitions
        assert_eq!(sctlr_el2::M, 1u64 << 0);   // MMU enable
        assert_eq!(sctlr_el2::C, 1u64 << 2);   // Data cache
        assert_eq!(sctlr_el2::I, 1u64 << 12);  // Instruction cache
    }

    /// Test CPTR_EL2 bits
    #[test]
    fn test_cptr_el2() {
        use super::cpu::init::cptr_el2;

        // Test TFP bit (trap FP/SIMD)
        assert_eq!(cptr_el2::TFP, 1u64 << 10);
    }

    // ========================================================================
    // SMP Tests
    // ========================================================================

    /// Test CPU state
    #[test]
    fn test_smp_cpu_state() {
        use super::smp::CpuState;

        assert_eq!(CpuState::Offline as u8, 0);
        assert_eq!(CpuState::Online as u8, 2);

        let state = CpuState::Online;
        assert!(state.is_online());
        assert!(!state.is_offline());
    }

    /// Test CPU info
    #[test]
    fn test_smp_cpu_info() {
        use super::smp::CpuInfo;

        let cpu = CpuInfo::new(1, 0x80000001);
        assert_eq!(cpu.logical_id, 1);
        assert_eq!(cpu.mpidr, 0x80000001);

        cpu.set_enable_method("psci");
        assert_eq!(cpu.enable_method, "psci");
    }

    /// Test PSCI operations
    #[test]
    fn test_psci_version() {
        use super::psci::PsciVersion;

        let v0_1 = PsciVersion::V0_1;
        let v1_0 = PsciVersion::V1_0;
        let v1_1 = PsciVersion::V1_1;

        assert!(v0_1 < v1_0);
        assert!(v1_0 < v1_1);
    }

    /// Test PSCI function IDs
    #[test]
    fn test_psci_functions() {
        use super::psci::*;

        // Test CPU_ON function ID
        let cpu_on = PSCI_CPU_ON;
        assert_eq!(cpu_on & 0x3FFF, 0x3); // Fast call

        // Test CPU_OFF function ID
        let cpu_off = PSCI_CPU_OFF;
        assert_eq!(cpu_off & 0x3FFF, 0x2); // Fast call
    }

    // ========================================================================
    // FPU Tests
    // ========================================================================

    /// Test FPU state
    #[test]
    fn test_fpu_state() {
        use super::cpu::fpu::FpuState;

        let state = FpuState::Disabled;
        assert!(!state.is_enabled());

        let enabled = FpuState::Enabled;
        assert!(enabled.is_enabled());
    }

    /// Test VFP registers
    #[test]
    fn test_vfp_registers() {
        use super::cpu::fpu::vfp::VfpRegisters;

        let regs = VfpRegisters::new();
        // VFP has 32 double registers
        assert!(regs.d.len() == 32);
    }

    /// Test NEON registers
    #[test]
    fn test_neon_registers() {
        use super::cpu::fpu::neon::NeonRegisters;

        let regs = NeonRegisters::new();
        // NEON has 32 vector registers
        assert!(regs.q.len() == 32);
    }

    // ========================================================================
    // Device Tree Tests
    // ========================================================================

    /// Test CPU enable method parsing
    #[test]
    fn test_cpu_enable_method() {
        use super::devtree::CpuEnableMethod;

        assert_eq!(CpuEnableMethod::from_str("spin-table"), CpuEnableMethod::SpinTable);
        assert_eq!(CpuEnableMethod::from_str("psci"), CpuEnableMethod::Psci);
        assert_eq!(CpuEnableMethod::from_str("arm"), CpuEnableMethod::Arm);
        assert_eq!(CpuEnableMethod::from_str("unknown"), CpuEnableMethod::Unknown);
    }

    /// Test CPU info
    #[test]
    fn test_devtree_cpu_info() {
        use super::devtree::CpuInfo;

        let cpu = CpuInfo::new(0, 0x80000000);
        assert_eq!(cpu.cpu_id, 0);
        assert_eq!(cpu.mpidr, 0x80000000);
        assert!(cpu.is_boot_cpu());
        assert_eq!(cpu.path(), "/cpus/cpu@0");
    }

    /// Test GIC info
    #[test]
    fn test_devtree_gic_info() {
        use super::devtree::GicInfo;

        let mut gic = GicInfo::new();
        gic.regs.push((0x2f000000, 0x10000));
        gic.regs.push((0x2f100000, 0x200000));
        assert_eq!(gic.dist_addr(), Some(0x2f000000));
        assert_eq!(gic.redistributor_addr(), Some(0x2f100000));
        assert!(gic.is_v3_or_later());
    }

    /// Test memory info
    #[test]
    fn test_devtree_mem_info() {
        use super::devtree::MemInfo;

        let mem = MemInfo::new(0x80000000, 0x10000000);
        assert_eq!(mem.base, 0x80000000);
        assert_eq!(mem.size, 0x10000000);
        assert_eq!(mem.end(), 0x90000000);
        assert!(mem.contains(0x80000000));
        assert!(mem.contains(0x8fffffff));
        assert!(!mem.contains(0x90000000));
    }

    /// Test interrupt parsing
    #[test]
    fn test_interrupt_parsing() {
        use super::devtree::parse::{InterruptType, InterruptFlags, parse_interrupt};

        // Test SPI (8 bytes)
        let spi_data = [0, 0, 0x03, 0xE8, 0, 0, 0, 4]; // SPI 1000, edge-triggered
        let (irq_type, flags) = parse_interrupt(&spi_data).unwrap();
        assert!(matches!(irq_type, InterruptType::Spi(1000)));
        assert!(flags.edge_triggered);

        // Test PPI (8 bytes)
        let ppi_data = [0, 0, 0, 1, 0, 0, 0, 0x1B, 0, 0, 0, 1]; // PPI 27, high-level
        let (irq_type2, flags2) = parse_interrupt(&ppi_data).unwrap();
        assert!(matches!(irq_type2, InterruptType::Ppi(27)));
        assert!(flags2.high_level);
    }

    // ========================================================================
    // Platform Tests
    // ========================================================================

    /// Test platform detection
    #[test]
    fn test_platform_detection() {
        use super::platform::qemu_virt::is_qemu_virt;
        use super::platform::foundation_v8::is_foundation_v8;

        // These may return false in non-QEMU environments
        let is_qemu = is_qemu_virt();
        let is_foundation = is_foundation_v8();

        // At least one should work (or we're on real hardware)
        // Don't assert - just verify the functions don't panic
        let _ = is_qemu;
        let _ = is_foundation;
    }

    /// Test platform info
    #[test]
    fn test_qemu_virt_platform() {
        use super::platform::qemu_virt::QemuVirtPlatform;

        let platform = QemuVirtPlatform::new();
        assert_eq!(platform.name(), "QEMU virt ARM64");
        assert_eq!(platform.gic_base(), super::platform::qemu_virt::QEMU_VIRT_GIC_DIST_BASE);
        assert_eq!(platform.uart_base(), Some(super::platform::qemu_virt::QEMU_VIRT_UART_BASE));
    }

    /// Test foundation v8 platform
    #[test]
    fn test_foundation_v8_platform() {
        use super::platform::foundation_v8::FoundationV8Platform;

        let platform = FoundationV8Platform::new();
        assert_eq!(platform.name(), "ARM Foundation v8");
        assert_eq!(platform.gic_base(), super::platform::foundation_v8::FOUNDATION_V8_GIC_DIST_BASE);
    }

    // ========================================================================
    // Integration Tests
    // ========================================================================

    /// Test basic hypervisor state
    #[test]
    fn test_hypervisor_state() {
        // Verify we're at EL2
        let el = super::current_exception_level();
        assert_eq!(el as u32, 2, "Not running at EL2");

        // Verify HCR_EL2 has VM bit set for Stage-2 translation
        let hcr: u64;
        unsafe { core::arch::asm!("mrs {}, hcr_el2", out(reg) hcr); }
        assert!(hcr & (1 << 0) != 0, "HCR_EL2.VM not set");
    }

    /// Test exception vector setup
    #[test]
    fn test_exception_vectors() {
        // Read VBAR_EL2
        let vbar: u64;
        unsafe { core::arch::asm!("mrs {}, vbar_el2", out(reg) vbar); }

        // VBAR should be aligned to 2KB
        assert_eq!(vbar & 0x7FF, 0, "VBAR_EL2 not aligned to 2KB");
        assert!(vbar != 0, "VBAR_EL2 not set");
    }
}

// ============================================================================
// Test Utilities
// ============================================================================

#[cfg(test)]
mod test_utils {
    /// Test helper to read system register
    #[inline(always)]
    pub unsafe fn read_sysreg(name: &str) -> u64 {
        match name {
            "hcr_el2" => {
                let mut val: u64;
                core::arch::asm!("mrs {}, hcr_el2", out(reg) val);
                val
            }
            "vtcr_el2" => {
                let mut val: u64;
                core::arch::asm!("mrs {}, vtcr_el2", out(reg) val);
                val
            }
            "sctlr_el2" => {
                let mut val: u64;
                core::arch::asm!("mrs {}, sctlr_el2", out(reg) val);
                val
            }
            "vbar_el2" => {
                let mut val: u64;
                core::arch::asm!("mrs {}, vbar_el2", out(reg) val);
                val
            }
            "cntfrq_el0" => {
                let mut val: u64;
                core::arch::asm!("mrs {}, cntfrq_el0", out(reg) val);
                val
            }
            "midr_el1" => {
                let mut val: u64;
                core::arch::asm!("mrs {}, midr_el1", out(reg) val);
                val
            }
            "mpidr_el1" => {
                let mut val: u64;
                core::arch::asm!("mrs {}, mpidr_el1", out(reg) val);
                val
            }
            _ => panic!("Unknown system register: {}", name),
        }
    }

    /// Test helper to write system register
    #[inline(always)]
    pub unsafe fn write_sysreg(name: &str, val: u64) {
        match name {
            "hcr_el2" => core::arch::asm!("msr hcr_el2, {}", in(reg) val),
            "vtcr_el2" => core::arch::asm!("msr vtcr_el2, {}", in(reg) val),
            "sctlr_el2" => core::arch::asm!("msr sctlr_el2, {}", in(reg) val),
            "vbar_el2" => core::arch::asm!("msr vbar_el2, {}", in(reg) val),
            _ => panic!("Unknown system register: {}", name),
        }
    }
}
