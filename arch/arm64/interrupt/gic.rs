//! GIC (Generic Interrupt Controller) driver for ARM64
//!
//! Provides GICv2 and GICv3 interrupt controller support.
//! Reference: ARM IHI 0048B (GIC architecture specification)
//! Reference: xvisor/drivers/irqchip/irq-gic.c, irq-gic-v3.c

/// GIC version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GicVersion {
    /// GICv1
    V1,
    /// GICv2
    V2,
    /// GICv3
    V3,
    /// GICv4
    V4,
}

/// GIC Distributor register offsets
pub mod gicd {
    /// GICD_CTLR - Distributor Control Register
    pub const CTLR: u64 = 0x000;
    /// GICD_TYPER - Distributor Type Register
    pub const TYPER: u64 = 0x004;
    /// GICD_IIDR - Distributor Implementer ID Register
    pub const IIDR: u64 = 0x008;
    /// GICD_IGROUPR - Interrupt Group Registers
    pub const IGROUPR: u64 = 0x080;
    /// GICD_ISENABLER - Interrupt Set-Enable Registers
    pub const ISENABLER: u64 = 0x100;
    /// GICD_ICENABLER - Interrupt Clear-Enable Registers
    pub const ICENABLER: u64 = 0x180;
    /// GICD_ISPENDR - Interrupt Set-Pending Registers
    pub const ISPENDR: u64 = 0x200;
    /// GICD_ICPENDR - Interrupt Clear-Pending Registers
    pub const ICPENDR: u64 = 0x280;
    /// GICD_ISACTIVER - Interrupt Set-Active Registers
    pub const ISACTIVER: u64 = 0x300;
    /// GICD_ICACTIVER - Interrupt Clear-Active Registers
    pub const ICACTIVER: u64 = 0x380;
    /// GICD_IPRIORITYR - Interrupt Priority Registers
    pub const IPRIORITYR: u64 = 0x400;
    /// GICD_ITARGETSR - Interrupt Processor Targets Registers
    pub const ITARGETSR: u64 = 0x800;
    /// GICD_ICFGR - Interrupt Configuration Registers
    pub const ICFGR: u64 = 0xC00;
    /// GICD_SGIR - Software Generated Interrupt Register
    pub const SGIR: u64 = 0xF00;
    /// GICD_CPENDSGIR - Clear Pending Set-Group Interrupt Register
    pub const CPENDSGIR: u64 = 0xF10;
    /// GICD_SPENDSGIR - Set Pending Set-Group Interrupt Register
    pub const SPENDSGIR: u64 = 0xF20;
    /// GICD_PIDR4 - Peripheral ID4 Register
    pub const PIDR4: u64 = 0xFD0;
    /// GICD_PIDR5 - Peripheral ID5 Register
    pub const PIDR5: u64 = 0xFD4;
    /// GICD_PIDR6 - Peripheral ID6 Register
    pub const PIDR6: u64 = 0xFD8;
    /// GICD_PIDR7 - Peripheral ID7 Register
    pub const PIDR7: u64 = 0xFDC;
    /// GICD_PIDR0 - Peripheral ID0 Register
    pub const PIDR0: u64 = 0xFE0;
    /// GICD_PIDR1 - Peripheral ID1 Register
    pub const PIDR1: u64 = 0xFE4;
    /// GICD_PIDR2 - Peripheral ID2 Register
    pub const PIDR2: u64 = 0xFE8;
    /// GICD_PIDR3 - Peripheral ID3 Register
    pub const PIDR3: u64 = 0xFEC;
    /// GICD_CIDR0 - Component ID0 Register
    pub const CIDR0: u64 = 0xFF0;
    /// GICD_CIDR1 - Component ID1 Register
    pub const CIDR1: u64 = 0xFF4;
    /// GICD_CIDR2 - Component ID2 Register
    pub const CIDR2: u64 = 0xFF8;
    /// GICD_CIDR3 - Component ID3 Register
    pub const CIDR3: u64 = 0xFFC;

    /// GICv3 specific registers
    /// GICD_STATUSR - Status Register
    pub const STATUSR: u64 = 0x010;
    /// GICD_SETSPI_NSR - Set SPI Pending Register
    pub const SETSPI_NSR: u64 = 0x040;
    /// GICD_CLRSPI_NSR - Clear SPI Pending Register
    pub const CLRSPI_NSR: u64 = 0x048;
    /// GICD_SETSPI_SR - Set SPI Pending Register
    pub const SETSPI_SR: u64 = 0x050;
    /// GICD_CLRSPI_SR - Clear SPI Pending Register
    pub const CLRSPI_SR: u64 = 0x058;
    /// GICD_IROUTER - Interrupt Routing Registers
    pub const IROUTER: u64 = 0x6000;
    /// GICD_IDROUPS - Interrupt Group Registers
    pub const IDGROUPS: u64 = 0x0810;

    /// GICD_CTLR bit definitions
    /// Enable distributor
    pub const CTLR_ENABLE: u32 = 1;
}

/// GIC CPU Interface register offsets (GICv2)
pub mod gicc {
    /// GICC_CTLR - CPU Interface Control Register
    pub const CTLR: u64 = 0x00;
    /// GICC_PMR - Interrupt Priority Mask Register
    pub const PMR: u64 = 0x04;
    /// GICC_BPR - Binary Point Register
    pub const BPR: u64 = 0x08;
    /// GICC_IAR - Interrupt Acknowledge Register
    pub const IAR: u64 = 0x0C;
    /// GICC_EOIR - End of Interrupt Register
    pub const EOIR: u64 = 0x10;
    /// GICC_RPR - Running Priority Register
    pub const RPR: u64 = 0x14;
    /// GICC_HPPIR - Highest Priority Pending Interrupt Register
    pub const HPPIR: u64 = 0x18;
    /// GICC_ABPR - Aliased Binary Point Register
    pub const ABPR: u64 = 0x1C;
    /// GICC_AIAR - Aliased Interrupt Acknowledge Register
    pub const AIAR: u64 = 0x20;
    /// GICC_AEOIR - Aliased End of Interrupt Register
    pub const AEOIR: u64 = 0x24;
    /// GICC_AHPPIR - Aliased Highest Priority Pending Interrupt Register
    pub const AHPPIR: u64 = 0x28;
    /// GICC_APR - Active Priorities Register
    pub const APR: u64 = 0xD0;
    /// GICC_NSAPR - Non-secure Active Priorities Register
    pub const NSAPR: u64 = 0xE0;
    /// GICC_IIDR - CPU Interface Implementer ID Register
    pub const IIDR: u64 = 0xFC;
    /// GICC_DIR - Deactivate Interrupt Register
    pub const DIR: u64 = 0x1000;

    /// GICC_CTLR bit definitions
    pub const CTLR_ENABLE: u32 = 1;
}

/// GICv3 Hypervisor Interface system registers (ICH)
pub mod ich {
    /// System register encoding values for ICH registers
    /// Format: Op0=3, Op1=4, CRn=12

    /// ICH_VSEIR_EL2 - Virtual System Error Register
    pub const VSEIR_EL2: u32 = 0x24; // CRm=9, Op2=4
    /// ICH_HCR_EL2 - Hypervisor Control Register
    pub const HCR_EL2: u32 = 0x20; // CRm=11, Op2=0
    /// ICH_VTR_EL2 - VGIC Type Register
    pub const VTR_EL2: u32 = 0x21; // CRm=11, Op2=1
    /// ICH_MISR_EL2 - Maintenance Interrupt Status Register
    pub const MISR_EL2: u32 = 0x22; // CRm=11, Op2=2
    /// ICH_EISR_EL2 - End of Interrupt Status Register
    pub const EISR_EL2: u32 = 0x23; // CRm=11, Op2=3
    /// ICH_ELSR_EL2 - Empty List Register Status Register
    pub const ELSR_EL2: u32 = 0x25; // CRm=11, Op2=5
    /// ICH_VMCR_EL2 - Virtual Machine Control Register
    pub const VMCR_EL2: u32 = 0x27; // CRm=11, Op2=7

    /// ICH_LR0_EL2 through ICH_LR15_EL2 - List Registers
    /// LR0-LR7: CRm=12, Op2=0-7
    /// LR8-LR15: CRm=13, Op2=0-7
    pub const LR0_EL2: u32 = 0x30; // CRm=12, Op2=0
    pub const LR1_EL2: u32 = 0x31;
    pub const LR2_EL2: u32 = 0x32;
    pub const LR3_EL2: u32 = 0x33;
    pub const LR4_EL2: u32 = 0x34;
    pub const LR5_EL2: u32 = 0x35;
    pub const LR6_EL2: u32 = 0x36;
    pub const LR7_EL2: u32 = 0x37;
    pub const LR8_EL2: u32 = 0x38; // CRm=13, Op2=0
    pub const LR9_EL2: u32 = 0x39;
    pub const LR10_EL2: u32 = 0x3A;
    pub const LR11_EL2: u32 = 0x3B;
    pub const LR12_EL2: u32 = 0x3C;
    pub const LR13_EL2: u32 = 0x3D;
    pub const LR14_EL2: u32 = 0x3E;
    pub const LR15_EL2: u32 = 0x3F;

    /// Active Priorities Registers
    /// AP0R0-AP0R3: CRm=8, Op2=0-3
    /// AP1R0-AP1R3: CRm=9, Op2=0-3
    pub const AP0R0_EL2: u32 = 0x28; // CRm=8, Op2=0
    pub const AP0R1_EL2: u32 = 0x29;
    pub const AP0R2_EL2: u32 = 0x2A;
    pub const AP0R3_EL2: u32 = 0x2B;
    pub const AP1R0_EL2: u32 = 0x2C; // CRm=9, Op2=0
    pub const AP1R1_EL2: u32 = 0x2D;
    pub const AP1R2_EL2: u32 = 0x2E;
    pub const AP1R3_EL2: u32 = 0x2F;

    /// ICH_HCR_EL2 bit definitions
    /// Enable Group 0 interrupts
    pub const HCR_EN: u64 = 1 << 0;
    /// Enable Group 1 interrupts
    pub const HCR_En: u64 = 1 << 1;
    /// UIE - Underflow interrupt enable
    pub const HCR_UIE: u64 = 1 << 1;
    /// LRENPIE - List Register entry not present interrupt enable
    pub const HCR_LRENPIE: u64 = 1 << 2;
    /// NPIE - No pending interrupt enable
    pub const HCR_NPIE: u64 = 1 << 3;
    /// VGRP0EIE - Group 0 enable interrupt enable
    pub const HCR_VGRP0EIE: u64 = 1 << 4;
    /// VGRP1EIE - Group 1 enable interrupt enable
    pub const HCR_VGRP1EIE: u64 = 1 << 5;
    /// EOICount - EOI count field
    pub const HCR_EOICOUNT_SHIFT: u32 = 27;
    pub const HCR_EOICOUNT_MASK: u64 = 0x1F << HCR_EOICOUNT_SHIFT;

    /// ICH_VTR_EL2 bit definitions
    /// Number of list registers (bits [0:3] + 1)
    pub const VTR_NR_LR_MASK: u64 = 0x3F;
    pub const VTR_NR_LR_SHIFT: u32 = 0;
    /// Number of priority bits (bits [29:31] + 1)
    pub const VTR_PRIO_BITS_MASK: u64 = 0x7 << 29;
    pub const VTR_PRIO_BITS_SHIFT: u32 = 29;

    /// ICH_LR_EL2 bit definitions
    /// Virtual interrupt ID [0:23] for GICv3
    pub const LR_VIRTUAL_ID_MASK: u64 = 0xFFFFFF;
    /// Physical interrupt ID
    pub const LR_PHYS_ID_MASK: u64 = 0x3FF << 32;
    pub const LR_PHYS_ID_SHIFT: u32 = 32;
    /// Priority field
    pub const LR_PRIORITY_MASK: u64 = 0xFF << 48;
    pub const LR_PRIORITY_SHIFT: u32 = 48;
    /// Group 1 interrupt
    pub const LR_GROUP: u64 = 1u64 << 62;
    /// Hardware interrupt
    pub const LR_HW: u64 = 1u64 << 61;
    /// State field
    pub const LR_STATE_PENDING: u64 = 1 << 0;
    pub const LR_STATE_ACTIVE: u64 = 1 << 1;
    pub const LR_STATE_PENDING_ACTIVE: u64 = 0x2;
    pub const LR_STATE_INVALID: u64 = 0x0;

    /// ICC_SRE_EL2 - System Register Enable register
    pub const SRE_EL2: u32 = 0x25; // Op0=3, Op1=4, CRn=12, CRm=9, Op2=5
    /// ICC_SRE_EL2 bit definitions
    /// SRE - System register access enable
    pub const SRE_EL2_SRE: u64 = 1 << 0;
    /// DFB - Disable FIQ bypass
    pub const SRE_EL2_DFB: u64 = 1 << 1;
    /// DIB - Disable IRQ bypass
    pub const SRE_EL2_DIB: u64 = 1 << 2;
    /// RM - Routing mode (0: IRQ/FIQ signals, 1: Stream protocol)
    pub const SRE_EL2_RM_SHIFT: u32 = 3;
    pub const SRE_EL2_RM_MASK: u64 = 0x3 << SRE_EL2_RM_SHIFT;
}

/// GICv3 System Register Access Functions
///
/// Provides safe access to GICv3 system registers used for interrupt
/// control in virtualized environments.
pub struct Gicv3SysRegs;

impl Gicv3SysRegs {
    /// Read ICC_IAR1_EL1 - Interrupt Acknowledge Register (Group 1)
    ///
    /// Returns the interrupt ID of the highest priority pending interrupt.
    /// # Safety
    /// Must be called at EL1 with ICC_SRE_EL1.SRE == 1
    #[inline]
    pub unsafe fn read_iar1() -> u64 {
        let mut value: u64;
        core::arch::asm!(
            "mrs {x}, ICC_IAR1_EL1",
            x = out(reg) value,
        );
        value
    }

    /// Write ICC_EOIR1_EL1 - End of Interrupt Register (Group 1)
    ///
    /// Signals completion of interrupt handling for the specified interrupt.
    /// # Safety
    /// Must be called at EL1 with ICC_SRE_EL1.SRE == 1
    #[inline]
    pub unsafe fn write_eoir1(irq: u64) {
        core::arch::asm!(
            "msr ICC_EOIR1_EL1, {x}",
            x = in(reg) irq,
        );
    }

    /// Write ICC_DIR_EL1 - Deactivate Interrupt Register
    ///
    /// Deactivates an interrupt without signaling completion.
    /// # Safety
    /// Must be called at EL1 with ICC_SRE_EL1.SRE == 1
    #[inline]
    pub unsafe fn write_dir(irq: u64) {
        core::arch::asm!(
            "msr ICC_DIR_EL1, {x}",
            x = in(reg) irq,
        );
    }

    /// Read ICC_SRE_EL2 - System Register Enable (EL2)
    ///
    /// # Safety
    /// Must be called at EL2
    #[inline]
    pub unsafe fn read_sre_el2() -> u64 {
        let mut value: u64;
        core::arch::asm!(
            "mrs {x}, ICC_SRE_EL2",
            x = out(reg) value,
        );
        value
    }

    /// Write ICC_SRE_EL2 - System Register Enable (EL2)
    ///
    /// # Safety
    /// Must be called at EL2
    #[inline]
    pub unsafe fn write_sre_el2(value: u64) {
        core::arch::asm!(
            "msr ICC_SRE_EL2, {x}",
            x = in(reg) value,
        );
    }

    /// Read ICH_HCR_EL2 - Hypervisor Control Register
    ///
    /// # Safety
    /// Must be called at EL2
    #[inline]
    pub unsafe fn read_hcr_el2() -> u64 {
        let mut value: u64;
        core::arch::asm!(
            "mrs {x}, ICH_HCR_EL2",
            x = out(reg) value,
        );
        value
    }

    /// Write ICH_HCR_EL2 - Hypervisor Control Register
    ///
    /// # Safety
    /// Must be called at EL2
    #[inline]
    pub unsafe fn write_hcr_el2(value: u64) {
        core::arch::asm!(
            "msr ICH_HCR_EL2, {x}",
            x = in(reg) value,
        );
    }

    /// Read ICH_VTR_EL2 - VGIC Type Register
    ///
    /// # Safety
    /// Must be called at EL2
    #[inline]
    pub unsafe fn read_vtr_el2() -> u64 {
        let mut value: u64;
        core::arch::asm!(
            "mrs {x}, ICH_VTR_EL2",
            x = out(reg) value,
        );
        value
    }

    /// Read ICH_LR_EL2 - List Register
    ///
    /// # Arguments
    /// * `index` - LR index (0-15)
    ///
    /// # Safety
    /// Must be called at EL2
    #[inline]
    pub unsafe fn read_lr_el2(index: u32) -> u64 {
        let mut value: u64;
        match index {
            0 => core::arch::asm!("mrs {x}, ICH_LR0_EL2", x = out(reg) value),
            1 => core::arch::asm!("mrs {x}, ICH_LR1_EL2", x = out(reg) value),
            2 => core::arch::asm!("mrs {x}, ICH_LR2_EL2", x = out(reg) value),
            3 => core::arch::asm!("mrs {x}, ICH_LR3_EL2", x = out(reg) value),
            4 => core::arch::asm!("mrs {x}, ICH_LR4_EL2", x = out(reg) value),
            5 => core::arch::asm!("mrs {x}, ICH_LR5_EL2", x = out(reg) value),
            6 => core::arch::asm!("mrs {x}, ICH_LR6_EL2", x = out(reg) value),
            7 => core::arch::asm!("mrs {x}, ICH_LR7_EL2", x = out(reg) value),
            8 => core::arch::asm!("mrs {x}, ICH_LR8_EL2", x = out(reg) value),
            9 => core::arch::asm!("mrs {x}, ICH_LR9_EL2", x = out(reg) value),
            10 => core::arch::asm!("mrs {x}, ICH_LR10_EL2", x = out(reg) value),
            11 => core::arch::asm!("mrs {x}, ICH_LR11_EL2", x = out(reg) value),
            12 => core::arch::asm!("mrs {x}, ICH_LR12_EL2", x = out(reg) value),
            13 => core::arch::asm!("mrs {x}, ICH_LR13_EL2", x = out(reg) value),
            14 => core::arch::asm!("mrs {x}, ICH_LR14_EL2", x = out(reg) value),
            15 => core::arch::asm!("mrs {x}, ICH_LR15_EL2", x = out(reg) value),
            _ => core::arch::asm!("mrs {x}, ICH_LR0_EL2", x = out(reg) value),
        };
        value
    }

    /// Write ICH_LR_EL2 - List Register
    ///
    /// # Arguments
    /// * `index` - LR index (0-15)
    /// * `value` - Value to write
    ///
    /// # Safety
    /// Must be called at EL2
    #[inline]
    pub unsafe fn write_lr_el2(index: u32, value: u64) {
        match index {
            0 => core::arch::asm!("msr ICH_LR0_EL2, {x}", x = in(reg) value),
            1 => core::arch::asm!("msr ICH_LR1_EL2, {x}", x = in(reg) value),
            2 => core::arch::asm!("msr ICH_LR2_EL2, {x}", x = in(reg) value),
            3 => core::arch::asm!("msr ICH_LR3_EL2, {x}", x = in(reg) value),
            4 => core::arch::asm!("msr ICH_LR4_EL2, {x}", x = in(reg) value),
            5 => core::arch::asm!("msr ICH_LR5_EL2, {x}", x = in(reg) value),
            6 => core::arch::asm!("msr ICH_LR6_EL2, {x}", x = in(reg) value),
            7 => core::arch::asm!("msr ICH_LR7_EL2, {x}", x = in(reg) value),
            8 => core::arch::asm!("msr ICH_LR8_EL2, {x}", x = in(reg) value),
            9 => core::arch::asm!("msr ICH_LR9_EL2, {x}", x = in(reg) value),
            10 => core::arch::asm!("msr ICH_LR10_EL2, {x}", x = in(reg) value),
            11 => core::arch::asm!("msr ICH_LR11_EL2, {x}", x = in(reg) value),
            12 => core::arch::asm!("msr ICH_LR12_EL2, {x}", x = in(reg) value),
            13 => core::arch::asm!("msr ICH_LR13_EL2, {x}", x = in(reg) value),
            14 => core::arch::asm!("msr ICH_LR14_EL2, {x}", x = in(reg) value),
            15 => core::arch::asm!("msr ICH_LR15_EL2, {x}", x = in(reg) value),
            _ => core::arch::asm!("msr ICH_LR0_EL2, {x}", x = in(reg) value),
        };
    }

    /// Read ICH_VMCR_EL2 - Virtual Machine Control Register
    ///
    /// # Safety
    /// Must be called at EL2
    #[inline]
    pub unsafe fn read_vmcr_el2() -> u64 {
        let mut value: u64;
        core::arch::asm!(
            "mrs {x}, ICH_VMCR_EL2",
            x = out(reg) value,
        );
        value
    }

    /// Write ICH_VMCR_EL2 - Virtual Machine Control Register
    ///
    /// # Safety
    /// Must be called at EL2
    #[inline]
    pub unsafe fn write_vmcr_el2(value: u64) {
        core::arch::asm!(
            "msr ICH_VMCR_EL2, {x}",
            x = in(reg) value,
        );
    }

    /// Read ICH_MISR_EL2 - Maintenance Interrupt Status Register
    ///
    /// # Safety
    /// Must be called at EL2
    #[inline]
    pub unsafe fn read_misr_el2() -> u64 {
        let mut value: u64;
        core::arch::asm!(
            "mrs {x}, ICH_MISR_EL2",
            x = out(reg) value,
        );
        value
    }

    /// Read ICH_AP0R0_EL2 through ICH_AP0R3_EL2 - Active Priorities (Group 0)
    ///
    /// # Safety
    /// Must be called at EL2
    #[inline]
    pub unsafe fn read_ap0r_el2(index: u32) -> u64 {
        let mut value: u64;
        match index {
            0 => core::arch::asm!("mrs {x}, ICH_AP0R0_EL2", x = out(reg) value),
            1 => core::arch::asm!("mrs {x}, ICH_AP0R1_EL2", x = out(reg) value),
            2 => core::arch::asm!("mrs {x}, ICH_AP0R2_EL2", x = out(reg) value),
            3 => core::arch::asm!("mrs {x}, ICH_AP0R3_EL2", x = out(reg) value),
            _ => core::arch::asm!("mrs {x}, ICH_AP0R0_EL2", x = out(reg) value),
        };
        value
    }

    /// Write ICH_AP0R0_EL2 through ICH_AP0R3_EL2 - Active Priorities (Group 0)
    ///
    /// # Safety
    /// Must be called at EL2
    #[inline]
    pub unsafe fn write_ap0r_el2(index: u32, value: u64) {
        match index {
            0 => core::arch::asm!("msr ICH_AP0R0_EL2, {x}", x = in(reg) value),
            1 => core::arch::asm!("msr ICH_AP0R1_EL2, {x}", x = in(reg) value),
            2 => core::arch::asm!("msr ICH_AP0R2_EL2, {x}", x = in(reg) value),
            3 => core::arch::asm!("msr ICH_AP0R3_EL2, {x}", x = in(reg) value),
            _ => core::arch::asm!("msr ICH_AP0R0_EL2, {x}", x = in(reg) value),
        };
    }

    /// Read ICH_AP1R0_EL2 through ICH_AP1R3_EL2 - Active Priorities (Group 1)
    ///
    /// # Safety
    /// Must be called at EL2
    #[inline]
    pub unsafe fn read_ap1r_el2(index: u32) -> u64 {
        let mut value: u64;
        match index {
            0 => core::arch::asm!("mrs {x}, ICH_AP1R0_EL2", x = out(reg) value),
            1 => core::arch::asm!("mrs {x}, ICH_AP1R1_EL2", x = out(reg) value),
            2 => core::arch::asm!("mrs {x}, ICH_AP1R2_EL2", x = out(reg) value),
            3 => core::arch::asm!("mrs {x}, ICH_AP1R3_EL2", x = out(reg) value),
            _ => core::arch::asm!("mrs {x}, ICH_AP1R0_EL2", x = out(reg) value),
        };
        value
    }

    /// Write ICH_AP1R0_EL2 through ICH_AP1R3_EL2 - Active Priorities (Group 1)
    ///
    /// # Safety
    /// Must be called at EL2
    #[inline]
    pub unsafe fn write_ap1r_el2(index: u32, value: u64) {
        match index {
            0 => core::arch::asm!("msr ICH_AP1R0_EL2, {x}", x = in(reg) value),
            1 => core::arch::asm!("msr ICH_AP1R1_EL2, {x}", x = in(reg) value),
            2 => core::arch::asm!("msr ICH_AP1R2_EL2, {x}", x = in(reg) value),
            3 => core::arch::asm!("msr ICH_AP1R3_EL2, {x}", x = in(reg) value),
            _ => core::arch::asm!("msr ICH_AP1R0_EL2, {x}", x = in(reg) value),
        };
    }
}

/// GIC Hypervisor Interface register offsets (virtualization)
pub mod gich {
    /// GICH_HCR - Hypervisor Control Register
    pub const HCR: u64 = 0x0;
    /// GICH_VTR - VGIC Type Register
    pub const VTR: u64 = 0x4;
    /// GICH_VMCR - Virtual Machine Control Register
    pub const VMCR: u64 = 0x8;
    /// GICH_MISR - Maintenance Interrupt Status Register
    pub const MISR: u64 = 0x10;
    /// GICH_EISR0 - End of Interrupt Status Register 0
    pub const EISR0: u64 = 0x20;
    /// GICH_EISR1 - End of Interrupt Status Register 1
    pub const EISR1: u64 = 0x24;
    /// GICH_ELRSR0 - Empty List Register Status Register 0
    pub const ELRSR0: u64 = 0x30;
    /// GICH_ELRSR1 - Empty List Register Status Register 1
    pub const ELRSR1: u64 = 0x34;
    /// GICH_APR - Active Priorities Register
    pub const APR: u64 = 0xF0;
    /// GICH_LR0 - List Register 0
    pub const LR0: u64 = 0x100;

    /// GICH_HCR bit definitions
    /// Enable Group 0 interrupts
    pub const HCR_EN: u32 = 1 << 0;
    /// Enable Group 1 interrupts
    pub const HCR_En: u32 = 1 << 1;
    /// UIE - Underflow interrupt enable
    pub const HCR_UIE: u32 = 1 << 1;
    /// LRENPIIST - List Register entry not present interrupt
    pub const HCR_LRENPIE: u32 = 1 << 2;
    /// NPIE - No pending interrupt enable
    pub const HCR_NPIE: u32 = 1 << 3;
    /// VGRP0EIE - Group 0 enable interrupt enable
    pub const HCR_VGRP0EIE: u32 = 1 << 4;
    /// VGRP1EIE - Group 1 enable interrupt enable
    pub const HCR_VGRP1EIE: u32 = 1 << 5;
    /// EOICount - EOI count
    pub const HCR_EOICOUNT_SHIFT: u32 = 27;
    pub const HCR_EOICOUNT_MASK: u32 = 0x1F << HCR_EOICOUNT_SHIFT;

    /// GICH_VTR bit definitions
    /// Number of list registers (bits [0:3] + 1)
    pub const VTR_LRCNT_MASK: u32 = 0x3F;
    pub const VTR_LRCNT_SHIFT: u32 = 0;

    /// GICH_LR bit definitions
    /// Virtual interrupt ID
    pub const LR_VIRTUALID: u32 = 0x3FF;
    /// Physical interrupt ID
    pub const LR_PHYSID: u32 = 0x3FF << 10;
    pub const LR_PHYSID_SHIFT: u32 = 10;
    pub const LR_PHYSID_EOI: u32 = 0x3FF << 10;
    /// Priority
    pub const LR_PRIO: u32 = 0x1F << 23;
    /// State
    pub const LR_STATE_PENDING: u32 = 1 << 28;
    pub const LR_STATE_ACTIVE: u32 = 1 << 29;
    pub const LR_STATE_INVALID: u32 = 0 << 28;
    pub const LR_STATE_MASK: u32 = 0x3 << 28;
    /// Hardware interrupt
    pub const LR_HW: u32 = 1 << 31;
    /// Group 1 interrupt
    pub const LR_GROUP1: u32 = 1 << 30;

    /// Maximum number of list registers (implementation defined)
    pub const LR_MAX_COUNT: usize = 4;
    pub const LR_MAX_COUNT_GICV3: usize = 16;
}

/// GIC Redistributor register offsets (GICv3)
pub mod gicr {
    /// GICR_CTLR - Redistributor Control Register
    pub const CTLR: u64 = 0x000;
    /// GICR_IIDR - Redistributor Implementer ID Register
    pub const IIDR: u64 = 0x004;
    /// GICR_TYPER - Redistributor Type Register
    pub const TYPER: u64 = 0x008;
    /// GICR_WAKER - Redistributor Wake Register
    pub const WAKER: u64 = 0x014;
    /// GICR_SETLPIR - Set LPI Pending Register
    pub const SETLPIR: u64 = 0x040;
    /// GICR_CLRLPIR - Clear LPI Pending Register
    pub const CLRLPIR: u64 = 0x048;
    /// GICR_PROPBASER - Redistributor Properties Base Address Register
    pub const PROPBASER: u64 = 0x070;
    /// GICR_PENDBASER - Redistributor Pending Table Base Address Register
    pub const PENDBASER: u64 = 0x078;
    /// GICR_INVLPIR - Invalidate LPI Pending Register
    pub const INVLPIR: u64 = 0x0A0;
    /// GICR_INVALLR - Invalidate All Register
    pub const INVALLR: u64 = 0x0B0;
    /// GICR_SYNCR - Synchronize Register
    pub const SYNCR: u64 = 0x0C0;
    /// GICR_IDROUPS - Interrupt Group Registers
    pub const IDGROUPS: u64 = 0x084;
}

/// GIC System Register access (GICv3)
pub mod icc {
    /// ICC_IAR0_EL1 - Interrupt Acknowledge Group 0
    pub const IAR0_EL1: u32 = 0xC0; // Op0=3, Op1=0, CRn=12, CRm=8, Op2=0
    /// ICC_IAR1_EL1 - Interrupt Acknowledge Group 1
    pub const IAR1_EL1: u32 = 0xC4; // Op0=3, Op1=0, CRn=12, CRm=12, Op2=0
    /// ICC_EOIR0_EL1 - End of Interrupt Group 0
    pub const EOIR0_EL1: u32 = 0xC1; // Op0=3, Op1=0, CRn=12, CRm=8, Op2=1
    /// ICC_EOIR1_EL1 - End of Interrupt Group 1
    pub const EOIR1_EL1: u32 = 0xC5; // Op0=3, Op1=0, CRn=12, CRm=12, Op2=1
    /// ICC_IGRPEN0_EL1 - Interrupt Group Enable Group 0
    pub const IGRPEN0_EL1: u32 = 0xC6; // Op0=3, Op1=0, CRn=12, CRm=12, Op2=6
    /// ICC_IGRPEN1_EL1 - Interrupt Group Enable Group 1
    pub const IGRPEN1_EL1: u32 = 0xC7; // Op0=3, Op1=0, CRn=12, CRm=12, Op2=6
    /// ICC_PMR_EL1 - Interrupt Priority Mask
    pub const PMR_EL1: u32 = 0x63; // Op0=3, Op1=0, CRn=4, CRm=6, Op2=0
    /// ICC_BPR0_EL1 - Binary Point Group 0
    pub const BPR0_EL1: u32 = 0xC8; // Op0=3, Op1=0, CRn=12, CRm=8, Op2=3
    /// ICC_BPR1_EL1 - Binary Point Group 1
    pub const BPR1_EL1: u32 = 0xC9; // Op0=3, Op1=0, CRn=12, Crm=12, Op2=3
    /// ICC_CTLR_EL1 - CPU Interface Control
    pub const CTLR_EL1: u32 = 0x62; // Op0=3, Op1=0, CRn=4, CRm=6, Op2=0
}

/// GIC Distributor state
#[derive(Debug)]
pub struct GicDistributor {
    base_addr: u64,
    version: GicVersion,
    num_irqs: u32,
    max_num_lr: u32,
    cpus: u32,
    it_lines_number: u32,
}

impl GicDistributor {
    /// Create new GIC distributor
    pub fn new(base_addr: u64, version: GicVersion, num_irqs: u32) -> Self {
        Self {
            base_addr,
            version,
            num_irqs,
            max_num_lr: 4, // Default for GICv2
            cpus: 1,
            it_lines_number: 0,
        }
    }

    /// Read distributor register
    #[inline]
    fn read_reg(&self, offset: u64) -> u32 {
        unsafe {
            let addr = (self.base_addr + offset) as *const u32;
            addr.read_volatile()
        }
    }

    /// Write distributor register
    #[inline]
    fn write_reg(&self, offset: u64, value: u32) {
        unsafe {
            let addr = (self.base_addr + offset) as *mut u32;
            addr.write_volatile(value);
        }
    }

    /// Enable the distributor
    pub fn enable(&self) {
        log::debug!("Enabling GIC distributor at {:#x}", self.base_addr);
        let ctlr = self.read_reg(gicd::CTLR);
        self.write_reg(gicd::CTLR, ctlr | gicd::CTLR_ENABLE);
    }

    /// Disable the distributor
    pub fn disable(&self) {
        log::debug!("Disabling GIC distributor at {:#x}", self.base_addr);
        let ctlr = self.read_reg(gicd::CTLR);
        self.write_reg(gicd::CTLR, ctlr & !gicd::CTLR_ENABLE);
    }

    /// Get distributor type information
    pub fn read_typer(&self) -> u32 {
        self.read_reg(gicd::TYPER)
    }

    /// Enable interrupt
    pub fn enable_irq(&self, irq: u32) {
        let reg_offset = gicd::ISENABLER + ((irq / 32) * 4) as u64;
        let bit = irq % 32;
        let mut val = self.read_reg(reg_offset);
        val |= 1 << bit;
        self.write_reg(reg_offset, val);
    }

    /// Disable interrupt
    pub fn disable_irq(&self, irq: u32) {
        let reg_offset = gicd::ICENABLER + ((irq / 32) * 4) as u64;
        let bit = irq % 32;
        let mut val = self.read_reg(reg_offset);
        val |= 1 << bit;
        self.write_reg(reg_offset, val);
    }

    /// Set interrupt priority
    pub fn set_priority(&self, irq: u32, priority: u8) {
        let reg_offset = gicd::IPRIORITYR + (irq as u64);
        self.write_reg(reg_offset, priority as u32);
    }

    /// Get interrupt priority
    pub fn get_priority(&self, irq: u32) -> u8 {
        let reg_offset = gicd::IPRIORITYR + (irq as u64);
        (self.read_reg(reg_offset) & 0xFF) as u8
    }

    /// Set interrupt target (GICv2)
    pub fn set_target(&self, irq: u32, cpu_mask: u8) {
        let reg_offset = gicd::ITARGETSR + (irq as u64);
        self.write_reg(reg_offset, cpu_mask as u32);
    }

    /// Configure interrupt as level-sensitive (0) or edge-triggered (1)
    pub fn set_config(&self, irq: u32, is_edge: bool) {
        let reg_offset = gicd::ICFGR + ((irq / 16) * 4) as u64;
        let bit = (irq % 16) * 2 + 1;
        let mut val = self.read_reg(reg_offset);
        if is_edge {
            val |= 1 << bit;
        } else {
            val &= !(1 << bit);
        }
        self.write_reg(reg_offset, val);
    }

    /// Generate software interrupt (SGI)
    pub fn generate_sgi(&self, sgi: u8, cpu_mask: u8) {
        let target_list = cpu_mask;
        let sgi_id = sgi & 0xF;
        let val = ((target_list as u32) << 16) | (sgi_id as u32);
        self.write_reg(gicd::SGIR, val);
    }

    /// Get the number of implemented interrupts
    pub fn get_num_irqs(&self) -> u32 {
        self.num_irqs
    }

    /// Get the GIC version
    pub fn get_version(&self) -> GicVersion {
        self.version
    }
}

/// GIC CPU Interface state (GICv2)
#[derive(Debug)]
pub struct GicCpuInterface {
    base_addr: u64,
    cpu_id: u32,
}

impl GicCpuInterface {
    /// Create new GIC CPU interface
    pub fn new(base_addr: u64, cpu_id: u32) -> Self {
        Self {
            base_addr,
            cpu_id,
        }
    }

    /// Read CPU interface register
    #[inline]
    fn read_reg(&self, offset: u64) -> u32 {
        unsafe {
            let addr = (self.base_addr + offset) as *const u32;
            addr.read_volatile()
        }
    }

    /// Write CPU interface register
    #[inline]
    fn write_reg(&self, offset: u64, value: u32) {
        unsafe {
            let addr = (self.base_addr + offset) as *mut u32;
            addr.write_volatile(value);
        }
    }

    /// Enable CPU interface
    pub fn enable(&self) {
        log::debug!("Enabling GIC CPU interface at {:#x}", self.base_addr);
        let mut ctlr = self.read_reg(gicc::CTLR);
        ctlr |= gicc::CTLR_ENABLE;
        self.write_reg(gicc::CTLR, ctlr);
    }

    /// Disable CPU interface
    pub fn disable(&self) {
        log::debug!("Disabling GIC CPU interface at {:#x}", self.base_addr);
        let mut ctlr = self.read_reg(gicc::CTLR);
        ctlr &= !gicc::CTLR_ENABLE;
        self.write_reg(gicc::CTLR, ctlr);
    }

    /// Set priority mask
    pub fn set_priority_mask(&self, mask: u8) {
        self.write_reg(gicc::PMR, mask as u32);
    }

    /// Get priority mask
    pub fn get_priority_mask(&self) -> u8 {
        (self.read_reg(gicc::PMR) & 0xFF) as u8
    }

    /// Acknowledge interrupt
    pub fn acknowledge_interrupt(&self) -> u32 {
        self.read_reg(gicc::IAR)
    }

    /// End of interrupt
    pub fn end_of_interrupt(&self, irq: u32) {
        self.write_reg(gicc::EOIR, irq);
    }

    /// Deactivate interrupt (GICv3)
    pub fn deactivate_interrupt(&self, irq: u32) {
        self.write_reg(gicc::DIR, irq);
    }

    /// Get highest priority pending interrupt
    pub fn get_hppir(&self) -> u32 {
        self.read_reg(gicc::HPPIR)
    }

    /// Set binary point
    pub fn set_binary_point(&self, bpr: u8) {
        self.write_reg(gicc::BPR, bpr as u32);
    }

    /// Get binary point
    pub fn get_binary_point(&self) -> u8 {
        (self.read_reg(gicc::BPR) & 0x7) as u8
    }
}

/// GIC Hypervisor Interface state (virtualization)
#[derive(Debug)]
pub struct GicHypInterface {
    base_addr: u64,
    num_lr: u32,
}

impl GicHypInterface {
    /// Create new GIC hypervisor interface
    pub fn new(base_addr: u64) -> Self {
        Self {
            base_addr,
            num_lr: 4,
        }
    }

    /// Get base address
    pub fn base_addr(&self) -> u64 {
        self.base_addr
    }

    /// Read hypervisor interface register
    #[inline]
    fn read_reg(&self, offset: u64) -> u32 {
        unsafe {
            let addr = (self.base_addr + offset) as *const u32;
            addr.read_volatile()
        }
    }

    /// Write hypervisor interface register
    #[inline]
    fn write_reg(&self, offset: u64, value: u32) {
        unsafe {
            let addr = (self.base_addr + offset) as *mut u32;
            addr.write_volatile(value);
        }
    }

    /// Enable virtual interface control
    pub fn enable(&self) {
        log::debug!("Enabling GIC hypervisor interface at {:#x}", self.base_addr);
        let mut hcr = self.read_reg(gich::HCR);
        hcr |= gich::HCR_EN;
        self.write_reg(gich::HCR, hcr);
    }

    /// Read VGIC type register
    pub fn read_vtr(&self) -> u32 {
        self.read_reg(gich::VTR)
    }

    /// Get number of list registers
    pub fn get_num_lr(&self) -> u32 {
        let vtr = self.read_vtr();
        ((vtr & gich::VTR_LRCNT_MASK) + 1) as u32
    }

    /// Read list register
    pub fn read_lr(&self, index: usize) -> u32 {
        if index >= self.num_lr as usize {
            0
        } else {
            self.read_reg(gich::LR0 + (index * 4) as u64)
        }
    }

    /// Write list register
    pub fn write_lr(&self, index: usize, value: u32) {
        if index < self.num_lr as usize {
            self.write_reg(gich::LR0 + (index * 4) as u64, value);
        }
    }

    /// Read maintenance interrupt status
    pub fn read_misr(&self) -> u32 {
        self.read_reg(gich::MISR)
    }
}

/// GIC device - combines distributor, CPU interface, and hypervisor interface
#[derive(Debug)]
pub struct GicDevice {
    distributor: GicDistributor,
    cpu_interface: GicCpuInterface,
    hyp_interface: Option<GicHypInterface>,
}

impl GicDevice {
    /// Create new GIC device
    pub fn new(
        dist_base: u64,
        cpu_base: u64,
        hyp_base: Option<u64>,
        version: GicVersion,
        num_irqs: u32,
        cpu_id: u32,
    ) -> Self {
        let distributor = GicDistributor::new(dist_base, version, num_irqs);
        let cpu_interface = GicCpuInterface::new(cpu_base, cpu_id);
        let hyp_interface = hyp_base.map(|base| GicHypInterface::new(base));

        Self {
            distributor,
            cpu_interface,
            hyp_interface,
        }
    }

    /// Enable GIC
    pub fn enable(&self) {
        self.distributor.enable();
        self.cpu_interface.enable();
        if let Some(ref hyp) = self.hyp_interface {
            hyp.enable();
        }
    }

    /// Disable GIC
    pub fn disable(&self) {
        self.cpu_interface.disable();
        self.distributor.disable();
    }

    /// Get distributor reference
    pub fn distributor(&self) -> &GicDistributor {
        &self.distributor
    }

    /// Get CPU interface reference
    pub fn cpu_interface(&self) -> &GicCpuInterface {
        &self.cpu_interface
    }

    /// Get hypervisor interface reference
    pub fn hyp_interface(&self) -> Option<&GicHypInterface> {
        self.hyp_interface.as_ref()
    }

    /// Initialize GIC for a VM
    pub fn init_vm(&self, vmid: u32) {
        log::info!("Initializing GIC for VMID {}", vmid);
        // TODO: Set up VM-specific interrupt routing
    }
}

/// Global GIC instance
static mut GIC_INSTANCE: Option<GicDevice> = None;

/// Initialize GIC
pub fn init(
    dist_base: u64,
    cpu_base: u64,
    hyp_base: Option<u64>,
    version: GicVersion,
    num_irqs: u32,
    cpu_id: u32,
) -> Result<(), &'static str> {
    log::info!("Initializing GIC v{:?} with {} IRQs", version, num_irqs);
    log::info!("  Distributor base: {:#x}", dist_base);
    log::info!("  CPU interface base: {:#x}", cpu_base);
    if let Some(base) = hyp_base {
        log::info!("  Hypervisor interface base: {:#x}", base);
    }

    let gic = GicDevice::new(dist_base, cpu_base, hyp_base, version, num_irqs, cpu_id);
    gic.enable();

    unsafe {
        GIC_INSTANCE = Some(gic);
    }

    log::info!("GIC initialized successfully");
    Ok(())
}

/// Get the global GIC instance
pub fn get() -> Option<&'static GicDevice> {
    unsafe { GIC_INSTANCE.as_ref() }
}

/// Get the global GIC instance (panic if not initialized)
pub fn get_expect() -> &'static GicDevice {
    get().expect("GIC not initialized")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gic_version() {
        assert_eq!(GicVersion::V2, GicVersion::V2);
        assert_eq!(GicVersion::V3, GicVersion::V3);
    }

    #[test]
    fn test_gic_distributor() {
        let dist = GicDistributor::new(0x08000000, GicVersion::V3, 1020);
        assert_eq!(dist.get_num_irqs(), 1020);
        assert_eq!(dist.get_version(), GicVersion::V3);
    }

    #[test]
    fn test_gic_cpu_interface() {
        let cpu = GicCpuInterface::new(0x08010000, 0);
        assert_eq!(cpu.cpu_id, 0);
    }
}
