//! ARM64 G-Stage (Stage-2) Address Translation
//!
//! This module provides G-stage (Stage-2) address translation support for ARM64 virtualization,
//! implementing Intermediate Physical Address (IPA) to Physical Address (PA) translation.
//!
//! Reference: ARM DDI 0487I.a - ARM Architecture Reference Manual
//! - Chapter D5 - VMSAv8-64 Stage-2 Translation
//! - Chapter D13 - System Registers - VTTBR_EL2, VTCR_EL2

use crate::{Result, Error};
use crate::arch::arm64::mm::{stage2, vttbr, vtcr};
use crate::arch::arm64::mm::stage2::{PageTable, PageTableLevel, PageTableEntry};
use core::sync::atomic::{AtomicU32, Ordering};
use alloc::vec::Vec;

/// Guest Physical Address (IPA) type
pub type Ipa = u64;

/// Host Physical Address type
pub type Hpa = u64;

/// Virtual Machine ID type (8-bit in VTTBR_EL2)
pub type Vmid = u16;

/// Stage-2 translation modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GStageMode {
    /// No translation (bypass mode)
    None = 0,
    /// 40-bit IPA (4KB granule, 3 levels)
    Ip4k_40bit = 1,
    /// 42-bit IPA (4KB granule, 3 levels)
    Ip4k_42bit = 2,
    /// 44-bit IPA (4KB granule, 3 levels)
    Ip4k_44bit = 3,
    /// 48-bit IPA (4KB granule, 4 levels) - Standard
    Ip4k_48bit = 4,
    /// 52-bit IPA (4KB granule, 5 levels) - ARMv8.4+
    Ip4k_52bit = 5,
    /// 16KB granule variants (optional)
    Ip16k_36bit = 6,
    Ip16k_40bit = 7,
    /// 64KB granule variants (optional)
    Ip64k_36bit = 8,
    Ip64k_40bit = 9,
    Ip64k_42bit = 10,
    Ip64k_48bit = 11,
}

impl GStageMode {
    /// Get number of IPA bits for this mode
    pub const fn ipa_bits(&self) -> u32 {
        match self {
            GStageMode::None => 0,
            GStageMode::Ip4k_40bit => 40,
            GStageMode::Ip4k_42bit => 42,
            GStageMode::Ip4k_44bit => 44,
            GStageMode::Ip4k_48bit => 48,
            GStageMode::Ip4k_52bit => 52,
            GStageMode::Ip16k_36bit => 36,
            GStageMode::Ip16k_40bit => 40,
            GStageMode::Ip64k_36bit => 36,
            GStageMode::Ip64k_40bit => 40,
            GStageMode::Ip64k_42bit => 42,
            GStageMode::Ip64k_48bit => 48,
        }
    }

    /// Get number of page table levels for this mode
    pub const fn levels(&self) -> u32 {
        match self {
            GStageMode::None => 0,
            GStageMode::Ip4k_40bit => 3,
            GStageMode::Ip4k_42bit => 3,
            GStageMode::Ip4k_44bit => 3,
            GStageMode::Ip4k_48bit => 4,
            GStageMode::Ip4k_52bit => 5,
            GStageMode::Ip16k_36bit => 2,
            GStageMode::Ip16k_40bit => 3,
            GStageMode::Ip64k_36bit => 2,
            GStageMode::Ip64k_40bit => 3,
            GStageMode::Ip64k_42bit => 3,
            GStageMode::Ip64k_48bit => 4,
        }
    }

    /// Get granule size in bytes
    pub const fn granule_size(&self) -> u64 {
        match self {
            GStageMode::Ip4k_40bit | GStageMode::Ip4k_42bit | GStageMode::Ip4k_44bit |
            GStageMode::Ip4k_48bit | GStageMode::Ip4k_52bit => 4096,
            GStageMode::Ip16k_36bit | GStageMode::Ip16k_40bit => 16384,
            GStageMode::Ip64k_36bit | GStageMode::Ip64k_40bit |
            GStageMode::Ip64k_42bit | GStageMode::Ip64k_48bit => 65536,
            GStageMode::None => 4096,
        }
    }

    /// Get VTCR_T0SZ value for this mode
    pub const fn t0sz(&self) -> u32 {
        match self {
            GStageMode::None => 0,
            GStageMode::Ip4k_40bit => 24, // 64 - 40
            GStageMode::Ip4k_42bit => 22,
            GStageMode::Ip4k_44bit => 20,
            GStageMode::Ip4k_48bit => 16,
            GStageMode::Ip4k_52bit => 12,
            GStageMode::Ip16k_36bit => 28,
            GStageMode::Ip16k_40bit => 24,
            GStageMode::Ip64k_36bit => 28,
            GStageMode::Ip64k_40bit => 24,
            GStageMode::Ip64k_42bit => 22,
            GStageMode::Ip64k_48bit => 16,
        }
    }

    /// Get VTCR_SL0 value (starting level)
    pub const fn sl0(&self) -> u32 {
        match self {
            GStageMode::None => 0,
            GStageMode::Ip4k_40bit | GStageMode::Ip4k_42bit | GStageMode::Ip4k_44bit => 1,
            GStageMode::Ip4k_48bit => 1,
            GStageMode::Ip4k_52bit => 1,
            GStageMode::Ip16k_36bit => 1,
            GStageMode::Ip16k_40bit => 1,
            GStageMode::Ip64k_36bit => 1,
            GStageMode::Ip64k_40bit => 1,
            GStageMode::Ip64k_42bit => 1,
            GStageMode::Ip64k_48bit => 1,
        }
    }

    /// Check if IPA is valid for this mode
    pub fn is_valid_ipa(&self, ipa: Ipa) -> bool {
        let bits = self.ipa_bits();
        if bits == 0 {
            return false;
        }
        ipa < (1u64 << bits)
    }
}

/// Hardware capability information for Stage-2 translation
#[derive(Debug, Clone)]
pub struct GStageCapabilities {
    /// Supported translation modes
    pub supported_modes: Vec<GStageMode>,
    /// Maximum supported IPA bits
    pub max_ipa_bits: u32,
    /// Supported granule sizes
    pub supported_granules: Vec<u64>,
    /// Support for 16KB granule
    pub granule_16k: bool,
    /// Support for 64KB granule
    pub granule_64k: bool,
    /// Support for Stage-2 Page Table Walk
    pub hw_walk: bool,
    /// Support for virtualization
    pub virtualization: bool,
    /// Support for contiguous hint
    pub contiguous: bool,
    /// Support for execute-never control
    pub xn_control: bool,
    /// Support for Access Flag update
    pub af_update: bool,
}

impl GStageCapabilities {
    /// Detect hardware capabilities
    pub fn detect() -> Self {
        let mut supported_modes = Vec::new();
        let mut supported_granules = Vec::new();

        // 4KB granule is always supported
        supported_granules.push(4096);
        supported_modes.push(GStageMode::Ip4k_48bit); // Standard 48-bit IPA
        supported_modes.push(GStageMode::Ip4k_40bit);
        supported_modes.push(GStageMode::Ip4k_44bit);

        // Detect extended IPA support (ARMv8.4+)
        #[cfg(feature = "armv8_4")]
        {
            supported_modes.push(GStageMode::Ip4k_52bit);
        }

        // 16KB granule detection
        let granule_16k = Self::detect_16k_granule();
        if granule_16k {
            supported_granules.push(16384);
            supported_modes.push(GStageMode::Ip16k_40bit);
        }

        // 64KB granule detection
        let granule_64k = Self::detect_64k_granule();
        if granule_64k {
            supported_granules.push(65536);
            supported_modes.push(GStageMode::Ip64k_48bit);
            supported_modes.push(GStageMode::Ip64k_40bit);
        }

        let max_ipa_bits = supported_modes.iter()
            .map(|m| m.ipa_bits())
            .max()
            .unwrap_or(48);

        Self {
            supported_modes,
            max_ipa_bits,
            supported_granules,
            granule_16k,
            granule_64k,
            hw_walk: true, // ARM64 always has hardware page table walk
            virtualization: true, // We're in EL2, so virtualization is present
            contiguous: true, // Contiguous hint is supported
            xn_control: true, // XN control is supported
            af_update: true, // AF hardware update is supported
        }
    }

    /// Detect 16KB granule support
    fn detect_16k_granule() -> bool {
        // Read ID_AA64MMFR0_EL1 to check TGRAN16 field
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let mut mmfr0: u64;
            core::arch::asm!("mrs {}, id_aa64mmfr0_el1", out(reg) mmfr0);
            let tgran16 = (mmfr0 >> 20) & 0xF;
            tgran16 != 0xF // Not "impl not defined"
        }
        #[cfg(not(target_arch = "aarch64"))]
        false
    }

    /// Detect 64KB granule support
    fn detect_64k_granule() -> bool {
        // Read ID_AA64MMFR0_EL1 to check TGRAN64 field
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let mut mmfr0: u64;
            core::arch::asm!("mrs {}, id_aa64mmfr0_el1", out(reg) mmfr0);
            let tgran64 = (mmfr0 >> 24) & 0xF;
            tgran64 != 0xF // Not "impl not defined"
        }
        #[cfg(not(target_arch = "aarch64"))]
        false
    }

    /// Check if a mode is supported
    pub fn supports_mode(&self, mode: GStageMode) -> bool {
        self.supported_modes.contains(&mode)
    }

    /// Get the best supported mode
    pub fn best_mode(&self) -> GStageMode {
        // Return the mode with maximum IPA bits
        self.supported_modes.iter()
            .max_by_key(|m| m.ipa_bits())
            .copied()
            .unwrap_or(GStageMode::Ip4k_48bit)
    }

    /// Check if granule size is supported
    pub fn supports_granule(&self, granule: u64) -> bool {
        self.supported_granules.contains(&granule)
    }
}

/// Stage-2 translation result
#[derive(Debug, Clone, Copy)]
pub struct TranslationResult {
    /// Host physical address
    pub hpa: Hpa,
    /// Access permissions
    pub permissions: TranslationPermissions,
    /// Page/block size
    pub page_size: u64,
    /// Translation level where mapping was found
    pub level: u32,
}

/// Translation permissions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranslationPermissions {
    /// Read permission
    pub readable: bool,
    /// Write permission
    pub writable: bool,
    /// Execute permission
    pub executable: bool,
}

impl TranslationPermissions {
    /// Create new permissions
    pub fn new(readable: bool, writable: bool, executable: bool) -> Self {
        Self { readable, writable, executable }
    }

    /// Read-write permissions
    pub const RW: Self = Self { readable: true, writable: true, executable: false };

    /// Read-execute permissions
    pub const RX: Self = Self { readable: true, writable: false, executable: true };

    /// Read-write-execute permissions
    pub const RWX: Self = Self { readable: true, writable: true, executable: true };

    /// No permissions
    pub const NONE: Self = Self { readable: false, writable: false, executable: false };
}

/// Stage-2 translation fault
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslationFault {
    /// Translation fault (page not mapped)
    Translation,
    /// Access fault (permission denied)
    Access,
    /// Permission fault
    Permission,
    /// Address size fault (IPA too large)
    AddressSize,
    /// Invalid PTE
    InvalidPte,
}

/// Stage-2 translation context (per-VM)
pub struct GStageContext {
    /// VMID for this context
    pub vmid: Vmid,
    /// Translation mode
    pub mode: GStageMode,
    /// Root page table physical address
    pub root_pa: Hpa,
    /// Root page table virtual address
    pub root_va: u64,
    /// VTTBR_EL2 value
    pub vttbr: u64,
    /// VTCR_EL2 value
    pub vtcr: u64,
    /// Hardware capabilities
    pub capabilities: GStageCapabilities,
    /// Translation statistics
    pub stats: TranslationStats,
}

/// Translation statistics
#[derive(Debug, Clone, Default)]
pub struct TranslationStats {
    /// Number of translations performed
    pub translations: u64,
    /// Number of translation misses
    pub misses: u64,
    /// Number of page faults
    pub page_faults: u64,
    /// Number of TLB flushes
    pub tlb_flushes: u64,
}

impl GStageContext {
    /// Create a new Stage-2 translation context
    pub fn new(vmid: Vmid, mode: GStageMode) -> Result<Self> {
        let capabilities = GStageCapabilities::detect();

        if !capabilities.supports_mode(mode) {
            return Err(Error::InvalidArgument);
        }

        Ok(Self {
            vmid,
            mode,
            root_pa: 0,
            root_va: 0,
            vttbr: 0,
            vtcr: 0,
            capabilities,
            stats: TranslationStats::default(),
        })
    }

    /// Create a new context with automatic mode detection
    pub fn new_with_auto_detection(vmid: Vmid) -> Result<Self> {
        let capabilities = GStageCapabilities::detect();
        let mode = capabilities.best_mode();
        Self::new(vmid, mode)
    }

    /// Initialize the context with a root page table
    pub fn init(&mut self, root_pa: Hpa, root_va: u64) -> Result<()> {
        self.root_pa = root_pa;
        self.root_va = root_va;

        // Create VTTBR_EL2 value
        self.vttbr = vttbr::make_vttbr(self.vmid, root_pa);

        // Create VTCR_EL2 value
        self.vtcr = vtcr::VtcrConfig::new_for_mode(self.mode).encode();

        Ok(())
    }

    /// Get current VTTBR_EL2 value
    pub fn get_vttbr(&self) -> u64 {
        self.vttbr
    }

    /// Get current VTCR_EL2 value
    pub fn get_vtcr(&self) -> u64 {
        self.vtcr
    }

    /// Translate IPA to HPA
    pub fn translate(&mut self, ipa: Ipa) -> Result<TranslationResult, TranslationFault> {
        self.stats.translations += 1;

        // Check if IPA is valid for this mode
        if !self.mode.is_valid_ipa(ipa) {
            self.stats.page_faults += 1;
            return Err(TranslationFault::AddressSize);
        }

        // Walk the page table
        match self.walk_page_table(ipa) {
            Ok(result) => Ok(result),
            Err(fault) => {
                self.stats.misses += 1;
                self.stats.page_faults += 1;
                Err(fault)
            }
        }
    }

    /// Walk the Stage-2 page table
    fn walk_page_table(&mut self, ipa: Ipa) -> Result<TranslationResult, TranslationFault> {
        let mut current_pa = self.root_pa;
        let mut current_level = self.mode.sl0() as usize;

        // Get starting level based on SL0
        let start_level = self.mode.sl0() as usize;

        for level_idx in start_level..self.mode.levels() as usize {
            let level = match level_idx {
                0 => PageTableLevel::L0,
                1 => PageTableLevel::L1,
                2 => PageTableLevel::L2,
                3 => PageTableLevel::L3,
                _ => break,
            };

            // Get the page table
            let pt_va = crate::core::mm::frame::phys_to_virt(current_pa);
            let pt = unsafe { &*(pt_va as *const PageTable) };

            // Get index at this level
            let index = stage2::level_index(level, ipa);

            // Get PTE
            let pte = pt.entries[index];

            // Check if PTE is valid
            if !pte.is_valid() {
                return Err(TranslationFault::Translation);
            }

            // Check if it's a block/page descriptor
            if pte.is_block() || pte.is_page() {
                // Found the translation
                let block_size = stage2::block_size_at_level(level);
                let offset = ipa & (block_size - 1);
                let hpa = (pte.output_addr() & stage2::pte::OUTADDR_MASK) + offset;

                // Get permissions
                let permissions = self.pte_to_permissions(&pte);

                return Ok(TranslationResult {
                    hpa,
                    permissions,
                    page_size: block_size,
                    level: level_idx as u32,
                });
            }

            // It's a table descriptor, continue walking
            current_pa = pte.output_addr() & stage2::pte::OUTADDR_MASK;
        }

        Err(TranslationFault::InvalidPte)
    }

    /// Convert PTE to translation permissions
    fn pte_to_permissions(&self, pte: &PageTableEntry) -> TranslationPermissions {
        let hap = (pte.raw >> stage2::pte::HAP_SHIFT) & 0x3;
        let xn = (pte.raw >> stage2::pte::XN_SHIFT) & 0x1;

        match hap {
            0 => TranslationPermissions::NONE,
            1 => TranslationPermissions::new(true, false, false), // Read-only
            2 => TranslationPermissions::new(false, true, false), // Write-only (unusual)
            3 => TranslationPermissions::new(true, true, xn == 0), // RW, X based on XN
            _ => TranslationPermissions::NONE,
        }
    }

    /// Flush TLB for this VM
    pub fn flush_tlb(&mut self) {
        self.stats.tlb_flushes += 1;

        #[cfg(target_arch = "aarch64")]
        unsafe {
            // TLBI IPAS2E1IS - Stage-2 IPA invalidate
            core::arch::asm!("tlbi ipas2e1is, {}", in(reg) self.vmid);
        }
    }

    /// Flush TLB for specific IPA range
    pub fn flush_tlb_ipa(&mut self, ipa: Ipa, size: u64) {
        self.stats.tlb_flushes += 1;

        #[cfg(target_arch = "aarch64")]
        unsafe {
            // Flush each page in range
            let mut addr = ipa;
            while addr < ipa + size {
                core::arch::asm!("tlbi ipas2e1is, {}", in(reg) (addr | (self.vmid as u64)));
                addr += 4096; // Flush by 4KB pages
            }
        }
    }

    /// Get translation statistics
    pub fn get_stats(&self) -> &TranslationStats {
        &self.stats
    }

    /// Reset translation statistics
    pub fn reset_stats(&mut self) {
        self.stats = TranslationStats::default();
    }
}

/// G-stage manager for managing multiple VM contexts
pub struct GStageManager {
    /// Maximum VMID value
    max_vmid: Vmid,
    /// G-stage contexts (indexed by VMID)
    contexts: alloc::collections::BTreeMap<Vmid, GStageContext>,
    /// Current active VMID
    active_vmid: Option<Vmid>,
}

impl GStageManager {
    /// Create a new G-stage manager
    pub fn new(max_vmid: Vmid) -> Self {
        Self {
            max_vmid,
            contexts: alloc::collections::BTreeMap::new(),
            active_vmid: None,
        }
    }

    /// Allocate a VMID and create a new context
    pub fn create_context(&mut self, mode: GStageMode) -> Result<Vmid> {
        let vmid = vttbr::allocate_vmid()?;
        let mut context = GStageContext::new(vmid, mode)?;
        self.contexts.insert(vmid, context);
        Ok(vmid)
    }

    /// Create a new context with automatic mode detection
    pub fn create_context_auto(&mut self) -> Result<Vmid> {
        let capabilities = GStageCapabilities::detect();
        let mode = capabilities.best_mode();
        self.create_context(mode)
    }

    /// Destroy a context and free its VMID
    pub fn destroy_context(&mut self, vmid: Vmid) -> Result<()> {
        if self.contexts.remove(&vmid).is_some() {
            vttbr::free_vmid(vmid);
            if self.active_vmid == Some(vmid) {
                self.active_vmid = None;
            }
            Ok(())
        } else {
            Err(Error::NotFound)
        }
    }

    /// Get a mutable reference to a context
    pub fn get_context_mut(&mut self, vmid: Vmid) -> Option<&mut GStageContext> {
        self.contexts.get_mut(&vmid)
    }

    /// Get a reference to a context
    pub fn get_context(&self, vmid: Vmid) -> Option<&GStageContext> {
        self.contexts.get(&vmid)
    }

    /// Set active VMID (for context switch)
    pub fn set_active_vmid(&mut self, vmid: Vmid) -> Result<()> {
        if self.contexts.contains_key(&vmid) {
            self.active_vmid = Some(vmid);

            // Write VTTBR_EL2 register
            if let Some(context) = self.get_context(vmid) {
                #[cfg(target_arch = "aarch64")]
                unsafe {
                    core::arch::asm!("msr vttbr_el2, {}", in(reg) context.get_vttcr());
                }
            }

            Ok(())
        } else {
            Err(Error::NotFound)
        }
    }

    /// Get active VMID
    pub fn get_active_vmid(&self) -> Option<Vmid> {
        self.active_vmid
    }

    /// Translate IPA for active VM
    pub fn translate_active(&mut self, ipa: Ipa) -> Result<TranslationResult, TranslationFault> {
        if let Some(vmid) = self.active_vmid {
            if let Some(context) = self.get_context_mut(vmid) {
                context.translate(ipa)
            } else {
                Err(TranslationFault::Translation)
            }
        } else {
            Err(TranslationFault::Translation)
        }
    }

    /// Get hardware capabilities
    pub fn get_capabilities(&self) -> GStageCapabilities {
        GStageCapabilities::detect()
    }
}

/// Global G-stage manager
static mut G_STAGE_MANAGER: Option<GStageManager> = None;
static G_STAGE_MANAGER_INIT: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// Initialize the global G-stage manager
pub fn init(max_vmid: Vmid) -> Result<()> {
    if G_STAGE_MANAGER_INIT.load(core::sync::atomic::Ordering::Relaxed) {
        return Ok(());
    }

    let manager = GStageManager::new(max_vmid);
    unsafe {
        G_STAGE_MANAGER = Some(manager);
    }

    G_STAGE_MANAGER_INIT.store(true, core::sync::atomic::Ordering::Release);
    log::info!("Global G-stage manager initialized with max VMID {}", max_vmid);
    Ok(())
}

/// Get the global G-stage manager
pub fn get() -> Option<&'static GStageManager> {
    unsafe { G_STAGE_MANAGER.as_ref() }
}

/// Get mutable reference to global G-stage manager
pub fn get_mut() -> Option<&'static mut GStageManager> {
    unsafe { G_STAGE_MANAGER.as_mut() }
}

/// Get the global G-stage manager (panic if not initialized)
pub fn get_expect() -> &'static GStageManager {
    get().expect("G-stage manager not initialized")
}

/// Get mutable global G-stage manager (panic if not initialized)
pub fn get_expect_mut() -> &'static mut GStageManager {
    get_mut().expect("G-stage manager not initialized")
}

/// Get global hardware capabilities
pub fn get_capabilities() -> GStageCapabilities {
    GStageCapabilities::detect()
}

/// Check if a specific mode is supported
pub fn supports_mode(mode: GStageMode) -> bool {
    get_capabilities().supports_mode(mode)
}

/// Get the best supported mode
pub fn get_best_mode() -> GStageMode {
    get_capabilities().best_mode()
}

/// Create a G-stage context with automatic mode selection
pub fn create_context_auto() -> Result<Vmid> {
    if let Some(manager) = get_mut() {
        manager.create_context_auto()
    } else {
        Err(Error::InvalidState)
    }
}

/// Create a G-stage context with specific mode
pub fn create_context_with_mode(mode: GStageMode) -> Result<Vmid> {
    if let Some(manager) = get_mut() {
        manager.create_context(mode)
    } else {
        Err(Error::InvalidState)
    }
}

/// Translate IPA for active VM
pub fn translate_active(ipa: Ipa) -> Result<TranslationResult, TranslationFault> {
    if let Some(manager) = get_mut() {
        manager.translate_active(ipa)
    } else {
        Err(TranslationFault::Translation)
    }
}

/// Module initialization
pub fn init_module() -> Result<()> {
    init(vttbr::MAX_VMID)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gstage_mode() {
        assert_eq!(GStageMode::Ip4k_48bit.ipa_bits(), 48);
        assert_eq!(GStageMode::Ip4k_48bit.levels(), 4);
        assert_eq!(GStageMode::Ip4k_40bit.ipa_bits(), 40);
        assert_eq!(GStageMode::Ip4k_40bit.levels(), 3);
    }

    #[test]
    fn test_t0sz_values() {
        assert_eq!(GStageMode::Ip4k_48bit.t0sz(), 16); // 64 - 48
        assert_eq!(GStageMode::Ip4k_40bit.t0sz(), 24); // 64 - 40
    }

    #[test]
    fn test_permissions() {
        let rw = TranslationPermissions::RW;
        assert!(rw.readable);
        assert!(rw.writable);
        assert!(!rw.executable);
    }
}
