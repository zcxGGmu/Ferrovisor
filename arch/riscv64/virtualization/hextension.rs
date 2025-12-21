//! RISC-V H Extension Implementation
//!
//! This module provides the RISC-V H extension implementation including:
//! - H extension CSR definitions and management
/// - Virtualization control structures
/// - Hypervisor trap handling
/// - Two-stage address translation support

use crate::arch::riscv64::*;
use crate::arch::riscv64::cpu::csr::*;
use bitflags::bitflags;

/// H extension registers
pub mod hcsr {
    use super::csr;

    /// Hypervisor status register
    pub const HSTATUS: usize = csr::HSTATUS;

    /// Hypervisor instruction delegation register
    pub const HEDELEG: usize = csr::HEDELEG;

    /// Hypervisor interrupt delegation register
    pub const HIDELEG: usize = csr::HIDELEG;

    /// Hypervisor interrupt enable register
    pub const HIE: usize = 0x604;

    /// Hypervisor counter enable register
    pub const HCOUNTEREN: usize = 0x606;

    /// Hypervisor guest external interrupt enable register
    pub const HGEIE: usize = 0x607;

    /// Hypervisor trap value register
    pub const HTVAL: usize = 0x643;

    /// Hypervisor interrupt pending register
    pub const HIP: usize = 0x644;

    /// Hypervisor virtual interrupt pending register
    pub const HVIP: usize = 0x645;

    /// Hypervisor trap instruction register
    pub const HTINST: usize = 0x64A;

    /// Hypervisor guest external interrupt pending register
    pub const HGEIP: usize = 0xE12;

    /// Hypervisor guest address translation and protection register
    pub const HGATP: usize = 0x680;

    /// Virtual supervisor status register
    pub const VSSTATUS: usize = csr::VSSTATUS;

    /// Virtual supervisor interrupt enable register
    pub const VSIE: usize = csr::VSIE;

    /// Virtual supervisor trap vector register
    pub const VSTVEC: usize = csr::VSTVEC;

    /// Virtual supervisor scratch register
    pub const VSSCRATCH: usize = csr::VSSCRATCH;

    /// Virtual supervisor exception program counter
    pub const VSEPC: usize = csr::VSEPC;

    /// Virtual supervisor cause register
    pub const VSCAUSE: usize = csr::VSCAUSE;

    /// Virtual supervisor trap value register
    pub const VSTVAL: usize = csr::VSTVAL;

    /// Virtual supervisor interrupt pending register
    pub const VSIP: usize = csr::VSIP;

    /// Virtual supervisor address translation and protection register
    pub const VSATP: usize = csr::VSATP;
}

/// HSTATUS register flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Hstatus: usize {
        const VTSR = 1 << 22;    // Virtual Trap SRET
        const VTW = 1 << 21;     // Virtual Timeout Wait
        const VTVM = 1 << 20;    // Virtual Trap Virtual Memory
        const HU = 1 << 17;      // Hypervisor in User mode
        const SPVP = 1 << 18;    // Supervisor Previous Virtual Privilege
        const SPV = 1 << 19;     // Supervisor Previous Virtualization
        const GVA = 1 << 6;      // Guest Virtual Access
        const VSBE = 1 << 5;     // Virtual SBE
    }
}

/// HEDELEG register flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hedeleg {
    value: usize,
}

impl Hedeleg {
    /// Create a new HEDELEG value
    pub const fn new() -> Self {
        Self { value: 0 }
    }

    /// Create from raw value
    pub const fn from_raw(value: usize) -> Self {
        Self { value }
    }

    /// Get raw value
    pub const fn raw(&self) -> usize {
        self.value
    }

    /// Delegate instruction page fault
    pub fn delegate_instruction_page_fault(&mut self) {
        self.value |= 1 << 12;
    }

    /// Delegate load page fault
    pub fn delegate_load_page_fault(&mut self) {
        self.value |= 1 << 13;
    }

    /// Delegate store page fault
    pub fn delegate_store_page_fault(&mut self) {
        self.value |= 1 << 15;
    }

    /// Delegate illegal instruction
    pub fn delegate_illegal_instruction(&mut self) {
        self.value |= 1 << 2;
    }

    /// Delegate user environment call
    pub fn delegate_user_ecall(&mut self) {
        self.value |= 1 << 8;
    }

    /// Delegate supervisor environment call
    pub fn delegate_supervisor_ecall(&mut self) {
        self.value |= 1 << 9;
    }

    /// Delegate breakpoint
    pub fn delegate_breakpoint(&mut self) {
        self.value |= 1 << 3;
    }

    /// Delegate all standard exceptions to supervisor
    pub fn delegate_all_standard_exceptions(&mut self) {
        self.value |=
            (1 << ExceptionCode::InstructionMisaligned as usize) |
            (1 << ExceptionCode::InstructionAccessFault as usize) |
            (1 << ExceptionCode::IllegalInstruction as usize) |
            (1 << ExceptionCode::Breakpoint as usize) |
            (1 << ExceptionCode::LoadMisaligned as usize) |
            (1 << ExceptionCode::LoadAccessFault as usize) |
            (1 << ExceptionCode::StoreMisaligned as usize) |
            (1 << ExceptionCode::StoreAccessFault as usize) |
            (1 << ExceptionCode::ECallFromUMode as usize) |
            (1 << ExceptionCode::ECallFromSMode as usize) |
            (1 << ExceptionCode::InstructionPageFault as usize) |
            (1 << ExceptionCode::LoadPageFault as usize) |
            (1 << ExceptionCode::StorePageFault as usize);
    }

    /// Check if an exception is delegated
    pub fn is_delegated(&self, exception_code: ExceptionCode) -> bool {
        (self.value & (1 << exception_code as usize)) != 0
    }
}

impl Default for Hedeleg {
    fn default() -> Self {
        let mut hedeleg = Self::new();
        hedeleg.delegate_all_standard_exceptions();
        hedeleg
    }
}

/// HIDELEG register flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hideleg {
    value: usize,
}

impl Hideleg {
    /// Create a new HIDELEG value
    pub const fn new() -> Self {
        Self { value: 0 }
    }

    /// Create from raw value
    pub const fn from_raw(value: usize) -> Self {
        Self { value }
    }

    /// Get raw value
    pub const fn raw(&self) -> usize {
        self.value
    }

    /// Delegate supervisor software interrupt
    pub fn delegate_supervisor_software(&mut self) {
        self.value |= 1 << InterruptCause::SupervisorSoftware as usize;
    }

    /// Delegate supervisor timer interrupt
    pub fn delegate_supervisor_timer(&mut self) {
        self.value |= 1 << InterruptCause::SupervisorTimer as usize;
    }

    /// Delegate supervisor external interrupt
    pub fn delegate_supervisor_external(&mut self) {
        self.value |= 1 << InterruptCause::SupervisorExternal as usize;
    }

    /// Delegate all supervisor interrupts
    pub fn delegate_all_supervisor_interrupts(&mut self) {
        self.value |=
            (1 << InterruptCause::SupervisorSoftware as usize) |
            (1 << InterruptCause::SupervisorTimer as usize) |
            (1 << InterruptCause::SupervisorExternal as usize);
    }

    /// Check if an interrupt is delegated
    pub fn is_delegated(&self, interrupt_cause: InterruptCause) -> bool {
        (self.value & (1 << interrupt_cause as usize)) != 0
    }
}

impl Default for Hideleg {
    fn default() -> Self {
        let mut hideleg = Self::new();
        hideleg.delegate_all_supervisor_interrupts();
        hideleg
    }
}

/// H Extension Configuration
#[derive(Debug, Clone)]
pub struct HExtensionConfig {
    /// Enable two-stage address translation
    pub enable_two_stage_translation: bool,
    /// Enable virtual interrupts
    pub enable_virtual_interrupts: bool,
    /// Support nested virtualization
    pub support_nested_virtualization: bool,
    /// Maximum number of VMIDs
    pub max_vmid: u16,
    /// Maximum number of VCPUs per VM
    pub max_vcpus_per_vm: u8,
}

impl Default for HExtensionConfig {
    fn default() -> Self {
        Self {
            enable_two_stage_translation: true,
            enable_virtual_interrupts: true,
            support_nested_virtualization: false,
            max_vmid: 4095, // 12-bit VMID
            max_vcpus_per_vm: 16,
        }
    }
}

/// H Extension Manager
pub struct HExtensionManager {
    /// Configuration
    config: HExtensionConfig,
    /// Is H extension enabled?
    enabled: bool,
    /// VMID allocator
    vmid_allocator: VmidAllocator,
}

impl HExtensionManager {
    /// Create a new H extension manager
    pub fn new(config: HExtensionConfig) -> Self {
        Self {
            config,
            enabled: false,
            vmid_allocator: VmidAllocator::new(config.max_vmid),
        }
    }

    /// Check if H extension is available
    pub fn is_available() -> bool {
        // Check if H extension is present in ISA string
        crate::arch::riscv64::cpu::features::has_extension(
            crate::arch::riscv64::cpu::features::IsaExtension::H,
        )
    }

    /// Initialize H extension
    pub fn init(&mut self) -> Result<(), &'static str> {
        if !Self::is_available() {
            return Err("H extension not supported");
        }

        log::info!("Initializing RISC-V H extension");

        // Configure HSTATUS
        self.configure_hstatus()?;

        // Set up exception and interrupt delegation
        self.configure_delegation()?;

        // Configure counter enable for virtualization
        self.configure_counter_enable()?;

        // Enable H extension
        self.enabled = true;

        log::info!("RISC-V H extension initialized successfully");
        Ok(())
    }

    /// Configure HSTATUS register
    fn configure_hstatus(&self) -> Result<(), &'static str> {
        let mut hstatus = HSTATUS::read();

        // Clear virtualization bits
        hstatus &= !(Hstatus::VTSR | Hstatus::VTW | Hstatus::VTVM);

        // Enable hypervisor features based on configuration
        if self.config.enable_two_stage_translation {
            // Configure for virtualization
        }

        HSTATUS::write(hstatus);
        Ok(())
    }

    /// Configure exception and interrupt delegation
    fn configure_delegation(&self) -> Result<(), &'static str> {
        // Use delegation module to configure registers
        if let Some(deleg_manager) = crate::arch::riscv64::virtualization::delegation::get_manager() {
            log::debug!("Using delegation module for configuration");
            // The delegation module already configured during init()
            Ok(())
        } else {
            // Fallback to basic configuration
            HEDELEG::delegate_all_standard();
            HIDELEG::delegate_all_standard();

            log::debug!("H extension delegation configured (fallback)");
            Ok(())
        }
    }

    /// Configure counter enable for virtualization
    fn configure_counter_enable(&self) -> Result<(), &'static str> {
        // Enable counters for guest access
        let mut hcounteren = 0usize;
        hcounteren |= 1 << 0; // Cycle
        hcounteren |= 1 << 1; // Time
        hcounteren |= 1 << 2; // Instret

        crate::arch::riscv64::cpu::csr::write_csr!(hcsr::HCOUNTEREN, hcounteren);

        log::debug!("H extension counter enable configured");
        Ok(())
    }

    /// Allocate a VMID
    pub fn allocate_vmid(&mut self) -> Result<u16, &'static str> {
        self.vmid_allocator.allocate()
    }

    /// Free a VMID
    pub fn free_vmid(&mut self, vmid: u16) {
        self.vmid_allocator.free(vmid);
    }

    /// Check if H extension is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get configuration
    pub fn config(&self) -> &HExtensionConfig {
        &self.config
    }

    /// Enable virtualization mode for entering guest
    pub fn enter_virtualization(&self, guest_csr: &GuestCsrState) -> Result<(), &'static str> {
        if !self.enabled {
            return Err("H extension not enabled");
        }

        // Save current hypervisor state if needed
        // Load guest CSR state
        guest_csr.load();

        // Update HSTATUS to indicate we're in guest mode
        let mut hstatus = HSTATUS::read();
        hstatus |= Hstatus::SPV; // Set Previous Virtualization bit
        HSTATUS::write(hstatus);

        log::debug!("Entered virtualization mode");
        Ok(())
    }

    /// Exit virtualization mode
    pub fn exit_virtualization(&self) -> Result<HypervisorTrapInfo, &'static str> {
        if !self.enabled {
            return Err("H extension not enabled");
        }

        // Save guest CSR state
        let guest_csr = GuestCsrState::save();

        // Read trap information
        let trap_info = HypervisorTrapInfo {
            guest_csr,
            cause: crate::arch::riscv64::cpu::csr::read_csr!(crate::arch::riscv64::cpu::csr::HGATP), // This is just placeholder
            tval: crate::arch::riscv64::cpu::csr::read_csr!(crate::arch::riscv64::cpu::csr::HTVAL),
            htinst: crate::arch::riscv64::cpu::csr::read_csr!(hcsr::HTINST),
        };

        // Clear virtualization bits
        let mut hstatus = HSTATUS::read();
        hstatus &= !Hstatus::SPV;
        HSTATUS::write(hstatus);

        log::debug!("Exited virtualization mode");
        Ok(trap_info)
    }
}

/// VMID Allocator
pub struct VmidAllocator {
    next_vmid: u16,
    max_vmid: u16,
    free_vmid: Vec<u16>,
}

impl VmidAllocator {
    /// Create a new VMID allocator
    pub fn new(max_vmid: u16) -> Self {
        Self {
            next_vmid: 1, // VMID 0 is reserved
            max_vmid,
            free_vmid: Vec::new(),
        }
    }

    /// Allocate a VMID
    pub fn allocate(&mut self) -> Result<u16, &'static str> {
        // Try to reuse a freed VMID
        if let Some(vmid) = self.free_vmid.pop() {
            return Ok(vmid);
        }

        // Allocate a new VMID
        if self.next_vmid <= self.max_vmid {
            let vmid = self.next_vmid;
            self.next_vmid += 1;
            Ok(vmid)
        } else {
            Err("No available VMID")
        }
    }

    /// Free a VMID
    pub fn free(&mut self, vmid: u16) {
        if vmid != 0 && vmid < self.next_vmid {
            self.free_vmid.push(vmid);
        }
    }
}

/// Guest CSR state
#[derive(Debug, Clone)]
pub struct GuestCsrState {
    /// Guest VSSTATUS
    pub vsstatus: usize,
    /// Guest VSIE
    pub vsie: usize,
    /// Guest VSTVEC
    pub vstvec: usize,
    /// Guest VSSCRATCH
    pub vsscratch: usize,
    /// Guest VSEPC
    pub vsepc: usize,
    /// Guest VSCAUSE
    pub vscause: usize,
    /// Guest VSTVAL
    pub vstval: usize,
    /// Guest VSIP
    pub vsip: usize,
    /// Guest VSATP
    pub vsatp: usize,
}

impl GuestCsrState {
    /// Save current guest CSR state
    pub fn save() -> Self {
        Self {
            vsstatus: crate::arch::riscv64::cpu::csr::read_csr!(hcsr::VSSTATUS),
            vsie: crate::arch::riscv64::cpu::csr::read_csr!(hcsr::VSIE),
            vstvec: crate::arch::riscv64::cpu::csr::read_csr!(hcsr::VSTVEC),
            vsscratch: crate::arch::riscv64::cpu::csr::read_csr!(hcsr::VSSCRATCH),
            vsepc: crate::arch::riscv64::cpu::csr::read_csr!(hcsr::VSEPC),
            vscause: crate::arch::riscv64::cpu::csr::read_csr!(hcsr::VSCAUSE),
            vstval: crate::arch::riscv64::cpu::csr::read_csr!(hcsr::VSTVAL),
            vsip: crate::arch::riscv64::cpu::csr::read_csr!(hcsr::VSIP),
            vsatp: crate::arch::riscv64::cpu::csr::read_csr!(hcsr::VSATP),
        }
    }

    /// Load guest CSR state
    pub fn load(&self) {
        crate::arch::riscv64::cpu::csr::write_csr!(hcsr::VSSTATUS, self.vsstatus);
        crate::arch::riscv64::cpu::csr::write_csr!(hcsr::VSIE, self.vsie);
        crate::arch::riscv64::cpu::csr::write_csr!(hcsr::VSTVEC, self.vstvec);
        crate::arch::riscv64::cpu::csr::write_csr!(hcsr::VSSCRATCH, self.vsscratch);
        crate::arch::riscv64::cpu::csr::write_csr!(hcsr::VSEPC, self.vsepc);
        crate::arch::riscv64::cpu::csr::write_csr!(hcsr::VSCAUSE, self.vscause);
        crate::arch::riscv64::cpu::csr::write_csr!(hcsr::VSTVAL, self.vstval);
        crate::arch::riscv64::cpu::csr::write_csr!(hcsr::VSIP, self.vsip);
        crate::arch::riscv64::cpu::csr::write_csr!(hcsr::VSATP, self.vsatp);
    }

    /// Create a new guest CSR state with default values
    pub fn new() -> Self {
        Self {
            vsstatus: 0,
            vsie: 0,
            vstvec: 0,
            vsscratch: 0,
            vsepc: 0,
            vscause: 0,
            vstval: 0,
            vsip: 0,
            vsatp: 0,
        }
    }
}

impl Default for GuestCsrState {
    fn default() -> Self {
        Self::new()
    }
}

/// Hypervisor trap information
#[derive(Debug, Clone)]
pub struct HypervisorTrapInfo {
    /// Guest CSR state at trap
    pub guest_csr: GuestCsrState,
    /// Trap cause
    pub cause: usize,
    /// Trap value
    pub tval: usize,
    /// Trap instruction
    pub htinst: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_h_extension_availability() {
        // Test whether H extension is available
        let has_h = HExtensionManager::is_available();
        println!("H extension available: {}", has_h);
    }

    #[test]
    fn test_hedeleg() {
        let mut hedeleg = Hedeleg::new();

        hedeleg.delegate_instruction_page_fault();
        assert!(hedeleg.is_delegated(ExceptionCode::InstructionPageFault));
        assert!(!hedeleg.is_delegated(ExceptionCode::LoadPageFault));

        hedeleg.delegate_all_standard_exceptions();
        assert!(hedeleg.is_delegated(ExceptionCode::LoadPageFault));
        assert!(hedeleg.is_delegated(ExceptionCode::IllegalInstruction));
    }

    #[test]
    fn test_hideleg() {
        let mut hideleg = Hideleg::new();

        hideleg.delegate_supervisor_timer();
        assert!(hideleg.is_delegated(InterruptCause::SupervisorTimer));
        assert!(!hideleg.is_delegated(InterruptCause::SupervisorSoftware));

        hideleg.delegate_all_supervisor_interrupts();
        assert!(hideleg.is_delegated(InterruptCause::SupervisorSoftware));
        assert!(hideleg.is_delegated(InterruptCause::SupervisorExternal));
    }

    #[test]
    fn test_vmid_allocator() {
        let mut allocator = VmidAllocator::new(10);

        let vmid1 = allocator.allocate().unwrap();
        let vmid2 = allocator.allocate().unwrap();

        assert_ne!(vmid1, vmid2);
        assert_eq!(vmid1, 1);
        assert_eq!(vmid2, 2);

        allocator.free(vmid1);
        let vmid3 = allocator.allocate().unwrap();
        assert_eq!(vmid3, vmid1);
    }

    #[test]
    fn test_guest_csr_state() {
        let state = GuestCsrState::new();
        assert_eq!(state.vsstatus, 0);
        assert_eq!(state.vsatp, 0);

        let saved_state = GuestCsrState::save();
        // In a real system, this would read actual CSR values
        println!("Saved guest CSR state: {:?}", saved_state);
    }

    #[test]
    fn test_h_extension_config() {
        let config = HExtensionConfig::default();
        assert!(config.enable_two_stage_translation);
        assert!(config.enable_virtual_interrupts);
        assert!(!config.support_nested_virtualization);
        assert_eq!(config.max_vmid, 4095);
        assert_eq!(config.max_vcpus_per_vm, 16);
    }
}