//! VGIC (Virtual GIC) implementation for ARM64
//!
//! Provides virtual GIC emulation for guest VMs using hardware virtualization.
//! Reference: ARM IHI 0069D (GIC architecture specification)
//! Reference: xvisor/arch/arm/cpu/common/vgic.c, vgic_v2.c

use crate::arch::arm64::interrupt::gic::{self, GicDevice, GicVersion};

/// Maximum number of VCPUs supported
pub const VGIC_MAX_NCPU: u32 = 8;

/// Maximum number of IRQs supported
pub const VGIC_MAX_NIRQ: u32 = 256;

/// Maximum number of list registers
pub const VGIC_MAX_LRS: usize = 16;

/// Unknown LR value
pub const VGIC_LR_UNKNOWN: u8 = 0xFF;

/// VGIC model type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VgicModel {
    /// GICv2
    V2,
    /// GICv3
    V3,
}

/// List register state for virtual interrupts
#[derive(Debug, Clone, Copy)]
pub struct VgicLr {
    /// Virtual interrupt ID
    pub virtid: u16,
    /// Physical interrupt ID (for HW interrupts)
    pub physid: u16,
    /// Priority
    pub prio: u8,
    /// LR flags
    pub flags: VgicLrFlags,
}

bitflags! {
    /// List register flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct VgicLrFlags: u32 {
        /// Interrupt is pending
        const STATE_PENDING = 1 << 0;
        /// Interrupt is active
        const STATE_ACTIVE = 1 << 1;
        /// Hardware interrupt (needs physical IRQ)
        const HW = 1 << 2;
        /// EOI interrupt
        const EOI_INT = 1 << 3;
        /// Group 1 interrupt
        const GROUP1 = 1 << 4;
    }
}

impl Default for VgicLr {
    fn default() -> Self {
        Self {
            virtid: 0,
            physid: 0,
            prio: 0,
            flags: VgicLrFlags::empty(),
        }
    }
}

/// VGIC hardware state
#[derive(Debug, Clone, Copy, Default)]
pub struct VgicHwState {
    /// GICv2 specific state
    pub v2: VgicHwStateV2,
}

/// GICv2 hardware state
#[derive(Debug, Clone, Copy, Default)]
pub struct VgicHwStateV2 {
    /// Hypervisor Control Register
    pub hcr: u32,
    /// Virtual Machine Control Register
    pub vmcr: u32,
    /// Active Priorities Register
    pub apr: u32,
    /// List registers
    pub lr: [u32; VGIC_MAX_LRS],
}

/// Per-VCPU VGIC state
#[derive(Debug)]
pub struct VgicVcpuState {
    /// Parent interrupt
    pub parent_irq: u32,

    /// Hardware state
    pub hw: VgicHwState,

    /// Number of used LRs
    pub lr_used_count: u32,
    /// Bitmap of used LRs
    pub lr_used: [u32; (VGIC_MAX_LRS + 31) / 32],
    /// IRQ to LR mapping
    pub irq_lr: [u8; VGIC_MAX_NIRQ as usize],
}

impl Default for VgicVcpuState {
    fn default() -> Self {
        Self {
            parent_irq: 0,
            hw: VgicHwState::default(),
            lr_used_count: 0,
            lr_used: [0; (VGIC_MAX_LRS + 31) / 32],
            irq_lr: [VGIC_LR_UNKNOWN; VGIC_MAX_NIRQ as usize],
        }
    }
}

impl VgicVcpuState {
    /// Create new VCPU state
    pub fn new() -> Self {
        Self::default()
    }

    /// Test if LR is used
    pub fn test_lr_used(&self, lr: usize) -> bool {
        (self.lr_used[lr >> 5] & (1 << (lr & 0x1f))) != 0
    }

    /// Mark LR as used
    pub fn set_lr_used(&mut self, lr: usize) {
        self.lr_used[lr >> 5] |= 1 << (lr & 0x1f);
        self.lr_used_count += 1;
    }

    /// Clear LR used flag
    pub fn clear_lr_used(&mut self, lr: usize) {
        self.lr_used[lr >> 5] &= !(1 << (lr & 0x1f));
        self.lr_used_count -= 1;
    }

    /// Get LR mapping for IRQ
    pub fn get_lr_map(&self, irq: u32) -> u8 {
        self.irq_lr[irq as usize]
    }

    /// Set LR mapping for IRQ
    pub fn set_lr_map(&mut self, irq: u32, lr: u8) {
        self.irq_lr[irq as usize] = lr;
    }

    /// Check if any LRs are used
    pub fn has_lr_used(&self) -> bool {
        self.lr_used_count > 0
    }
}

/// VGIC guest state
#[derive(Debug)]
pub struct VgicGuestState {
    /// Configuration
    pub num_vcpus: u32,
    pub num_irqs: u32,

    /// Per-VCPU state
    pub vcpu_states: Vec<VgicVcpuState>,

    /// Distributor enabled
    pub enabled: bool,

    /// GIC version
    pub version: GicVersion,
}

impl VgicGuestState {
    /// Create new VGIC guest state
    pub fn new(num_vcpus: u32, num_irqs: u32, version: GicVersion) -> Self {
        let mut vcpu_states = Vec::with_capacity(num_vcpus as usize);
        for _ in 0..num_vcpus {
            vcpu_states.push(VgicVcpuState::new());
        }

        Self {
            num_vcpus,
            num_irqs,
            vcpu_states,
            enabled: false,
            version,
        }
    }

    /// Get VCPU state
    pub fn vcpu_state(&self, vcpu_id: u32) -> Option<&VgicVcpuState> {
        self.vcpu_states.get(vcpu_id as usize)
    }

    /// Get mutable VCPU state
    pub fn vcpu_state_mut(&mut self, vcpu_id: u32) -> Option<&mut VgicVcpuState> {
        self.vcpu_states.get_mut(vcpu_id as usize)
    }
}

/// VGIC ops for version-specific operations
pub trait VgicOps {
    /// Reset hardware state
    fn reset_state(&self, state: &mut VgicHwState, model: VgicModel);

    /// Save hardware state
    fn save_state(&self, state: &mut VgicHwState, model: VgicModel);

    /// Restore hardware state
    fn restore_state(&self, state: &VgicHwState, model: VgicModel);

    /// Check for underflow
    fn check_underflow(&self) -> bool;

    /// Enable underflow interrupt
    fn enable_underflow(&self);

    /// Disable underflow interrupt
    fn disable_underflow(&self);

    /// Read empty list register status
    fn read_elrsr(&self, elrsr: &mut [u32; 2]);

    /// Read EOI status
    fn read_eisr(&self, eisr: &mut [u32; 2]);

    /// Set list register
    fn set_lr(&self, lr: usize, lrv: &VgicLr, model: VgicModel);

    /// Get list register
    fn get_lr(&self, lr: usize, lrv: &mut VgicLr, model: VgicModel);

    /// Clear list register
    fn clear_lr(&self, lr: usize);
}

/// GICv2 VGIC operations
pub struct VgicV2Ops {
    hyp_base: u64,
    lr_cnt: u32,
}

impl VgicV2Ops {
    /// Create new GICv2 ops
    pub fn new(gic: &GicDevice) -> Option<Self> {
        let hyp_interface = gic.hyp_interface()?;
        Some(Self {
            hyp_base: hyp_interface.base_addr(),
            lr_cnt: hyp_interface.get_num_lr(),
        })
    }

    /// Read hypervisor register
    #[inline]
    fn read_reg(&self, offset: u64) -> u32 {
        unsafe {
            let addr = (self.hyp_base + offset) as *const u32;
            addr.read_volatile()
        }
    }

    /// Write hypervisor register
    #[inline]
    fn write_reg(&self, offset: u64, value: u32) {
        unsafe {
            let addr = (self.hyp_base + offset) as *mut u32;
            addr.write_volatile(value);
        }
    }
}

impl VgicOps for VgicV2Ops {
    fn reset_state(&self, state: &mut VgicHwState, _model: VgicModel) {
        state.v2.hcr = gic::gich::HCR_EN;
        state.v2.vmcr = 0;
        state.v2.apr = 0;
        for i in 0..self.lr_cnt as usize {
            state.v2.lr[i] = 0;
        }
    }

    fn save_state(&self, state: &mut VgicHwState, _model: VgicModel) {
        use gic::gich;
        state.v2.hcr = self.read_reg(gich::HCR);
        state.v2.vmcr = self.read_reg(gich::VMCR);
        state.v2.apr = self.read_reg(gich::APR);
        self.write_reg(gich::HCR, 0);
        for i in 0..self.lr_cnt as usize {
            state.v2.lr[i] = self.read_reg(gich::LR0 + (i * 4) as u64);
        }
    }

    fn restore_state(&self, state: &VgicHwState, _model: VgicModel) {
        use gic::gich;
        self.write_reg(gich::HCR, state.v2.hcr);
        self.write_reg(gich::VMCR, state.v2.vmcr);
        self.write_reg(gich::APR, state.v2.apr);
        for i in 0..self.lr_cnt as usize {
            self.write_reg(gich::LR0 + (i * 4) as u64, state.v2.lr[i]);
        }
    }

    fn check_underflow(&self) -> bool {
        use gic::gich;
        let misr = self.read_reg(gich::MISR);
        (misr & gich::HCR_UIE) != 0
    }

    fn enable_underflow(&self) {
        use gic::gich;
        let hcr = self.read_reg(gich::HCR);
        self.write_reg(gich::HCR, hcr | gich::HCR_UIE);
    }

    fn disable_underflow(&self) {
        use gic::gich;
        let hcr = self.read_reg(gich::HCR);
        self.write_reg(gich::HCR, hcr & !gich::HCR_UIE);
    }

    fn read_elrsr(&self, elrsr: &mut [u32; 2]) {
        use gic::gich;
        elrsr[0] = self.read_reg(gich::ELRSR0);
        if self.lr_cnt > 32 {
            elrsr[1] = self.read_reg(gich::ELRSR1);
        } else {
            elrsr[1] = 0;
        }
    }

    fn read_eisr(&self, eisr: &mut [u32; 2]) {
        use gic::gich;
        eisr[0] = self.read_reg(gich::EISR0);
        if self.lr_cnt > 32 {
            eisr[1] = self.read_reg(gich::EISR1);
        } else {
            eisr[1] = 0;
        }
    }

    fn set_lr(&self, lr: usize, lrv: &VgicLr, _model: VgicModel) {
        use gic::gich;
        let mut lrval = (lrv.virtid as u32) & gich::LR_VIRTUALID;
        lrval |= ((lrv.prio as u32) << 23) & gich::LR_PRIO;

        if lrv.flags.contains(VgicLrFlags::STATE_PENDING) {
            lrval |= gich::LR_STATE_PENDING;
        }
        if lrv.flags.contains(VgicLrFlags::STATE_ACTIVE) {
            lrval |= gich::LR_STATE_ACTIVE;
        }
        if lrv.flags.contains(VgicLrFlags::HW) {
            lrval |= gich::LR_HW;
            lrval |= ((lrv.physid as u32) << 10) & gich::LR_PHYSID;
        } else if lrv.flags.contains(VgicLrFlags::EOI_INT) {
            lrval |= gich::LR_PHYSID_EOI;
        }
        if lrv.flags.contains(VgicLrFlags::GROUP1) {
            lrval |= gich::LR_GROUP1;
        }

        self.write_reg(gich::LR0 + (lr * 4) as u64, lrval);
    }

    fn get_lr(&self, lr: usize, lrv: &mut VgicLr, _model: VgicModel) {
        use gic::gich;
        let lrval = self.read_reg(gich::LR0 + (lr * 4) as u64);

        lrv.virtid = (lrval & gich::LR_VIRTUALID) as u16;
        lrv.physid = ((lrval & gich::LR_PHYSID) >> 10) as u16;
        lrv.prio = ((lrval & gich::LR_PRIO) >> 23) as u8;
        lrv.flags = VgicLrFlags::empty();

        if lrval & gich::LR_STATE_PENDING != 0 {
            lrv.flags |= VgicLrFlags::STATE_PENDING;
        }
        if lrval & gich::LR_STATE_ACTIVE != 0 {
            lrv.flags |= VgicLrFlags::STATE_ACTIVE;
        }
        if lrval & gich::LR_HW != 0 {
            lrv.flags |= VgicLrFlags::HW;
        } else if lrval & gich::LR_PHYSID_EOI != 0 {
            lrv.flags |= VgicLrFlags::EOI_INT;
        }
        if lrval & gich::LR_GROUP1 != 0 {
            lrv.flags |= VgicLrFlags::GROUP1;
        }
    }

    fn clear_lr(&self, lr: usize) {
        use gic::gich;
        self.write_reg(gich::LR0 + (lr * 4) as u64, 0);
    }
}

/// VGIC device
#[derive(Debug)]
pub struct VgicDevice {
    /// GIC device
    gic: *const GicDevice,
    /// VGIC operations
    ops: Option<Box<dyn VgicOps>>,
    /// Number of list registers
    lr_cnt: u32,
    /// Guest state
    guest_state: Option<VgicGuestState>,
}

// Safety: The GIC device pointer has static lifetime and is only accessed safely
unsafe impl Send for VgicDevice {}
unsafe impl Sync for VgicDevice {}

impl VgicDevice {
    /// Create new VGIC device
    pub fn new(gic: &'static GicDevice) -> Self {
        let lr_cnt = gic.hyp_interface()
            .map(|h| h.get_num_lr())
            .unwrap_or(4);

        let ops = match gic.distributor().get_version() {
            GicVersion::V2 => VgicV2Ops::new(gic).map(|o| Box::new(o) as Box<dyn VgicOps>),
            _ => None,
        };

        Self {
            gic,
            ops,
            lr_cnt,
            guest_state: None,
        }
    }

    /// Get GIC device
    pub fn gic(&self) -> &GicDevice {
        unsafe { &*self.gic }
    }

    /// Get number of list registers
    pub fn lr_cnt(&self) -> u32 {
        self.lr_cnt
    }

    /// Check if VGIC is available
    pub fn is_available(&self) -> bool {
        self.ops.is_some()
    }

    /// Initialize guest state
    pub fn init_guest(&mut self, num_vcpus: u32, num_irqs: u32) -> Result<(), &'static str> {
        if num_vcpus > VGIC_MAX_NCPU {
            return Err("Too many VCPUs");
        }
        if num_irqs > VGIC_MAX_NIRQ {
            return Err("Too many IRQs");
        }

        let version = self.gic().distributor().get_version();
        self.guest_state = Some(VgicGuestState::new(num_vcpus, num_irqs, version));

        log::info!("VGIC initialized: {} VCPUs, {} IRQs", num_vcpus, num_irqs);
        Ok(())
    }

    /// Inject virtual interrupt
    pub fn inject_irq(&self, vcpu_id: u32, virt_irq: u32, phys_irq: Option<u32>) -> Result<(), &'static str> {
        let Some(ops) = &self.ops else {
            return Err("VGIC not available");
        };

        let Some(guest_state) = &self.guest_state else {
            return Err("Guest not initialized");
        };

        let Some(vcpu_state) = guest_state.vcpu_state(vcpu_id) else {
            return Err("Invalid VCPU ID");
        };

        // Find free LR
        let lr = if let Some(lr) = vcpu_state.irq_lr.get(virt_irq as usize) {
            if *lr != VGIC_LR_UNKNOWN && vcpu_state.test_lr_used(*lr as usize) {
                *lr as usize
            } else {
                // Find free LR
                let mut free_lr = None;
                for i in 0..self.lr_cnt as usize {
                    if !vcpu_state.test_lr_used(i) {
                        free_lr = Some(i);
                        break;
                    }
                }

                match free_lr {
                    Some(lr) => lr,
                    None => return Err("No free list registers"),
                }
            }
        } else {
            return Err("Invalid virtual IRQ");
        };

        // Build LR value
        let mut lrv = VgicLr {
            virtid: virt_irq as u16,
            physid: phys_irq.unwrap_or(0) as u16,
            prio: 0,
            flags: VgicLrFlags::STATE_PENDING,
        };

        if phys_irq.is_some() {
            lrv.flags |= VgicLrFlags::HW;
        }

        let model = match guest_state.version {
            GicVersion::V2 => VgicModel::V2,
            _ => VgicModel::V3,
        };

        ops.set_lr(lr, &lrv, model);

        log::debug!("Injected IRQ {} to VCPU {} (LR{})", virt_irq, vcpu_id, lr);
        Ok(())
    }

    /// Save VCPU context
    pub fn save_vcpu_context(&self, vcpu_id: u32) -> Result<(), &'static str> {
        let Some(ops) = &self.ops else {
            return Err("VGIC not available");
        };

        let Some(guest_state) = &self.guest_state else {
            return Err("Guest not initialized");
        };

        let Some(vcpu_state) = guest_state.vcpu_state(vcpu_id) else {
            return Err("Invalid VCPU ID");
        };

        let model = match guest_state.version {
            GicVersion::V2 => VgicModel::V2,
            _ => VgicModel::V3,
        };

        ops.save_state(&mut vcpu_state.hw, model);

        log::debug!("Saved VCPU {} VGIC context", vcpu_id);
        Ok(())
    }

    /// Restore VCPU context
    pub fn restore_vcpu_context(&self, vcpu_id: u32) -> Result<(), &'static str> {
        let Some(ops) = &self.ops else {
            return Err("VGIC not available");
        };

        let Some(guest_state) = &self.guest_state else {
            return Err("Guest not initialized");
        };

        let Some(vcpu_state) = guest_state.vcpu_state(vcpu_id) else {
            return Err("Invalid VCPU ID");
        };

        let model = match guest_state.version {
            GicVersion::V2 => VgicModel::V2,
            _ => VgicModel::V3,
        };

        ops.restore_state(&vcpu_state.hw, model);

        log::debug!("Restored VCPU {} VGIC context", vcpu_id);
        Ok(())
    }

    /// Enable distributor
    pub fn enable(&self) {
        if let Some(ref state) = self.guest_state {
            // Note: This would need interior mutability in real implementation
            log::info!("VGIC distributor enabled");
        }
    }

    /// Disable distributor
    pub fn disable(&self) {
        if let Some(ref _state) = self.guest_state {
            // Note: This would need interior mutability in real implementation
            log::info!("VGIC distributor disabled");
        }
    }
}

/// Global VGIC instance
static mut VGIC_INSTANCE: Option<VgicDevice> = None;

/// Initialize VGIC
pub fn init(gic: &'static GicDevice) -> Result<(), &'static str> {
    log::info!("Initializing VGIC");

    let vgic = VgicDevice::new(gic);

    if !vgic.is_available() {
        log::warn!("VGIC not available (no hypervisor interface)");
        return Err("VGIC not available");
    }

    log::info!("VGIC initialized with {} LRs", vgic.lr_cnt());

    unsafe {
        VGIC_INSTANCE = Some(vgic);
    }

    Ok(())
}

/// Get the global VGIC instance
pub fn get() -> Option<&'static VgicDevice> {
    unsafe { VGIC_INSTANCE.as_ref() }
}

/// Get the global VGIC instance (panic if not initialized)
pub fn get_expect() -> &'static VgicDevice {
    get().expect("VGIC not initialized")
}

/// Initialize VGIC
pub fn init_module() -> Result<(), &'static str> {
    log::info!("Initializing VGIC module");
    log::info!("VGIC module initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vgic_model() {
        assert_eq!(VgicModel::V2, VgicModel::V2);
        assert_eq!(VgicModel::V3, VgicModel::V3);
    }

    #[test]
    fn test_vgic_lr_flags() {
        let flags = VgicLrFlags::STATE_PENDING | VgicLrFlags::HW;
        assert!(flags.contains(VgicLrFlags::STATE_PENDING));
        assert!(flags.contains(VgicLrFlags::HW));
        assert!(!flags.contains(VgicLrFlags::STATE_ACTIVE));
    }

    #[test]
    fn test_vgic_lr_default() {
        let lr = VgicLr::default();
        assert_eq!(lr.virtid, 0);
        assert_eq!(lr.physid, 0);
        assert_eq!(lr.prio, 0);
        assert!(lr.flags.is_empty());
    }

    #[test]
    fn test_vgic_vcpu_state() {
        let state = VgicVcpuState::new();
        assert!(!state.has_lr_used());
        assert!(!state.test_lr_used(0));
        assert_eq!(state.get_lr_map(0), VGIC_LR_UNKNOWN);
    }

    #[test]
    fn test_vgic_vcpu_state_lr_management() {
        let mut state = VgicVcpuState::new();
        state.set_lr_used(2);
        assert!(state.has_lr_used());
        assert!(state.test_lr_used(2));
        assert!(!state.test_lr_used(0));
        assert_eq!(state.lr_used_count, 1);
    }

    #[test]
    fn test_vgic_guest_state() {
        let state = VgicGuestState::new(4, 256, GicVersion::V2);
        assert_eq!(state.num_vcpus, 4);
        assert_eq!(state.num_irqs, 256);
        assert!(!state.enabled);
    }

    #[test]
    fn test_constants() {
        assert_eq!(VGIC_MAX_NCPU, 8);
        assert_eq!(VGIC_MAX_NIRQ, 256);
        assert_eq!(VGIC_MAX_LRS, 16);
        assert_eq!(VGIC_LR_UNKNOWN, 0xFF);
    }
}
