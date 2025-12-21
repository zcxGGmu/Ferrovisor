//! Virtual CPU (VCPU) Management
//!
//! This module handles the lifecycle and management of virtual CPUs.

use crate::{Result, Error};
use crate::core::vmm::{VmId, VcpuId, VmExitInfo, VmExitReason, VmExitArchData, VcpuRegisters};
use crate::core::sched::{Thread, ThreadId, Priority};
use crate::core::sync::SpinLock;
use core::ptr::NonNull;

/// Maximum number of VCPUs
pub const MAX_VCPUS: usize = 256;

/// VCPU states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VcpuState {
    /// VCPU is not created
    Uninitialized,
    /// VCPU is created but not running
    Ready,
    /// VCPU is running guest code
    Running,
    /// VCPU is blocked (waiting for something)
    Blocked,
    /// VCPU has exited
    Exited,
}

/// VCPU priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VcpuPriority {
    /// Idle priority (lowest)
    Idle = 0,
    /// Low priority
    Low = 1,
    /// Normal priority
    Normal = 2,
    /// High priority
    High = 3,
    /// Real-time priority (highest)
    RealTime = 4,
}

impl Default for VcpuPriority {
    fn default() -> Self {
        VcpuPriority::Normal
    }
}

/// VCPU structure
pub struct VirtualCpu {
    /// VCPU ID
    id: VcpuId,
    /// Parent VM ID
    vm_id: VmId,
    /// Current VCPU state
    state: VcpuState,
    /// VCPU priority
    priority: VcpuPriority,
    /// VCPU registers
    registers: SpinLock<VcpuRegisters>,
    /// Host thread for this VCPU
    host_thread: SpinLock<Option<ThreadId>>,
    /// Exit information (populated on VM exit)
    exit_info: SpinLock<Option<VmExitInfo>>,
    /// Time slice quota
    time_slice: u32,
    /// Total execution time
    exec_time: u64,
    /// Number of exits
    exit_count: u64,
    /// Architecture-specific data
    arch_data: VcpuArchData,
}

/// Architecture-specific VCPU data
#[repr(C)]
pub union VcpuArchData {
    /// ARM64 specific data
    pub arm64: VcpuArm64Data,
    /// RISC-V specific data
    pub riscv64: VcpuRiscv64Data,
    /// x86_64 specific data
    pub x86_64: VcpuX86_64Data,
}

/// ARM64-specific VCPU data
#[derive(Debug, Clone)]
pub struct VcpuArm64Data {
    /// VCPU context
    pub context: crate::arch::common::Arm64Context,
    /// Virtualization Control Register
    pub vtcr_el2: u64,
    /// Hypervisor Configuration Register
    pub hcr_el2: u64,
    /// Virtualization Translation Control Register
    pub vttbr_el2: u64,
    /// Virtual ID register
    pub vmpidr_el2: u64,
    /// Virtual Auxillary Control Register
    pub vactlr_el2: u64,
    /// Virtual Processor ID Register
    pub vpidr_el2: u64,
    /// Virtual System Control Register
    pub vsctlr_el2: u64,
}

/// RISC-V-specific VCPU data
#[derive(Debug, Clone)]
pub struct VcpuRiscv64Data {
    /// VCPU context
    pub context: crate::arch::common::Riscv64Context,
    /// Hypervisor Status Register
    pub hstatus: u64,
    /// Hypervisor Delegation Register
    pub hedeleg: u64,
    /// Hypervisor Interrupt Delegation Register
    pub hideleg: u64,
    /// Virtual Supervisor Translation Register
    pub vstval: u64,
    /// Virtual Supervisor Address Translation and Protection
    pub vsatp: u64,
    /// Virtual Supervisor Status Register
    pub vsstatus: u64,
}

/// x86_64-specific VCPU data
#[derive(Debug, Clone)]
pub struct VcpuX86_64Data {
    /// VCPU context
    pub context: crate::arch::common::X86_64Context,
    /// VMCS region pointer
    pub vmcs_region: VirtAddr,
    /// VMCS revision identifier
    pub vmcs_revision: u32,
    /// VMX controls
    pub vmx_controls: VmxControls,
}

/// VMX controls
#[derive(Debug, Clone, Copy)]
pub struct VmxControls {
    /// Pin-based controls
    pub pin_controls: u32,
    /// Primary processor controls
    pub proc_controls: u32,
    /// Secondary processor controls
    pub proc_controls2: u32,
    /// Exit controls
    pub exit_controls: u32,
    /// Entry controls
    pub entry_controls: u32,
}

/// VCPU Manager
struct VcpuManager {
    /// Bitmap tracking allocated VCPU IDs
    vcpu_id_bitmap: crate::utils::bitmap::Bitmap,
    /// Array of VCPU references
    vcpus: [Option<NonNull<VirtualCpu>>; MAX_VCPUS],
    /// Number of active VCPUs
    active_vcpus: usize,
}

impl VirtualCpu {
    /// Create a new VCPU
    pub fn new(id: VcpuId, vm_id: VmId) -> Result<Self> {
        let arch_data = unsafe {
            #[cfg(target_arch = "aarch64")]
            {
                VcpuArchData {
                    arm64: VcpuArm64Data {
                        context: Default::default(),
                        vtcr_el2: 0,
                        hcr_el2: 0,
                        vttbr_el2: 0,
                        vmpidr_el2: id as u64,
                        vactlr_el2: 0,
                        vpidr_el2: 0,
                        vsctlr_el2: 0,
                    }
                }
            }

            #[cfg(target_arch = "riscv64")]
            {
                VcpuArchData {
                    riscv64: VcpuRiscv64Data {
                        context: Default::default(),
                        hstatus: 0,
                        hedeleg: 0,
                        hideleg: 0,
                        vstval: 0,
                        vsatp: 0,
                        vsstatus: 0,
                    }
                }
            }

            #[cfg(target_arch = "x86_64")]
            {
                VcpuArchData {
                    x86_64: VcpuX86_64Data {
                        context: Default::default(),
                        vmcs_region: 0,
                        vmcs_revision: 0,
                        vmx_controls: VmxControls {
                            pin_controls: 0,
                            proc_controls: 0,
                            proc_controls2: 0,
                            exit_controls: 0,
                            entry_controls: 0,
                        },
                    }
                }
            }
        };

        Ok(Self {
            id,
            vm_id,
            state: VcpuState::Ready,
            priority: VcpuPriority::default(),
            registers: SpinLock::new(VcpuRegisters {
                gpr: [0; 32],
                pc: 0,
                sp: 0,
                psr: 0,
                arch_regs: VcpuArchRegisters { arm64: VcpuArm64Registers {
                    sctlr_el1: 0,
                    tcr_el1: 0,
                    ttbr0_el1: 0,
                    ttbr1_el1: 0,
                    mair_el1: 0,
                    amair_el1: 0,
                    vbar_el1: 0,
                    cntvoff_el2: 0,
                    cntkctl_el1: 0,
                    fp_simd: None,
                }},
            }),
            host_thread: SpinLock::new(None),
            exit_info: SpinLock::new(None),
            time_slice: 10, // Default 10ms
            exec_time: 0,
            exit_count: 0,
            arch_data,
        })
    }

    /// Get VCPU ID
    pub fn id(&self) -> VcpuId {
        self.id
    }

    /// Get parent VM ID
    pub fn vm_id(&self) -> VmId {
        self.vm_id
    }

    /// Get VCPU state
    pub fn state(&self) -> VcpuState {
        self.state
    }

    /// Set VCPU state
    pub fn set_state(&mut self, state: VcpuState) {
        self.state = state;
    }

    /// Get VCPU priority
    pub fn priority(&self) -> VcpuPriority {
        self.priority
    }

    /// Set VCPU priority
    pub fn set_priority(&mut self, priority: VcpuPriority) {
        self.priority = priority;
    }

    /// Get VCPU registers
    pub fn get_registers(&self) -> VcpuRegisters {
        self.registers.lock().clone()
    }

    /// Set VCPU registers
    pub fn set_registers(&self, regs: &VcpuRegisters) -> Result<()> {
        *self.registers.lock() = regs.clone();
        Ok(())
    }

    /// Initialize VCPU for first run
    pub fn initialize(&mut self) -> Result<()> {
        // Architecture-specific initialization
        #[cfg(target_arch = "aarch64")]
        {
            self.init_arm64()?;
        }

        #[cfg(target_arch = "riscv64")]
        {
            self.init_riscv64()?;
        }

        #[cfg(target_arch = "x86_64")]
        {
            self.init_x86_64()?;
        }

        self.state = VcpuState::Ready;
        Ok(())
    }

    #[cfg(target_arch = "aarch64")]
    fn init_arm64(&mut self) -> Result<()> {
        let arm64_data = unsafe { &mut self.arch_data.arm64 };

        // Configure VTCR_EL2
        arm64_data.vtcr_el2 = (0b0u64 << 31) |     // VS: 0
                               (0u64 << 30) |        // IRGN1: Normal, WB
                               (0u64 << 26) |        // ORGN1: Normal, WB
                               (0u64 << 24) |        // SH0: Non-shareable
                               (0b00u64 << 14) |     // TG0: 4KB granule
                               (0b0000u64 << 6) |    // SL0: 4KB level 0
                               (1u64 << 7) |         // HA: Hardware AF
                               (0u64);               // DS: Disabled

        // Configure HCR_EL2
        arm64_data.hcr_el2 = (1u64 << 31) |        // RW: 64-bit
                               (1u64 << 28) |        // IMO: IRQ to EL2
                               (1u64 << 27) |        // FMO: FIQ to EL2
                               (0u64 << 1);         // VM: Enable stage-2

        Ok(())
    }

    #[cfg(target_arch = "riscv64")]
    fn init_riscv64(&mut self) -> Result<()> {
        let riscv_data = unsafe { &mut self.arch_data.riscv64 };

        // Configure HSTATUS
        riscv_data.hstatus = 0;

        // Configure HEDELEG
        riscv_data.hedeleg = (1 << 0) |   // Instruction address misaligned
                             (1 << 1) |   // Instruction access fault
                             (1 << 3) |   // Breakpoint
                             (1 << 5) |   // Load access fault
                             (1 << 7) |   // Store/AMO access fault
                             (1 << 13);  // Instruction page fault

        // Configure HIDELEG
        riscv_data.hideleg = (1 << 5) |   // Timer interrupt
                             (1 << 9) |   // External interrupt
                             (1 << 15);  // Store/AMO page fault

        Ok(())
    }

    #[cfg(target_arch = "x86_64")]
    fn init_x86_64(&mut self) -> Result<()> {
        let x86_data = unsafe { &mut self.arch_data.x86_64 };

        // TODO: Initialize VMCS
        // This would involve:
        // - Allocating VMCS region
        // - Initializing VMCS fields
        // - Setting up VMCS controls
        // - Loading VMCS

        Ok(())
    }

    /// Run the VCPU (enter guest mode)
    pub fn run(&self) -> Result<VmExitInfo> {
        if self.state != VcpuState::Ready && self.state != VcpuState::Running {
            return Err(Error::InvalidState);
        }

        self.state = VcpuState::Running;

        // Save host context
        let mut host_context = crate::arch::common::CpuContext::default();
        unsafe {
            #[cfg(target_arch = "aarch64")]
            crate::arch::arm64::save_context(&mut host_context);

            #[cfg(target_arch = "riscv64")]
            crate::arch::riscv64::save_context(&mut host_context);

            #[cfg(target_arch = "x86_64")]
            crate::arch::x86_64::save_context(&mut host_context);
        }

        // Load guest context and enter guest
        let exit_info = unsafe {
            #[cfg(target_arch = "aarch64")]
            {
                self.run_arm64()?
            }

            #[cfg(target_arch = "riscv64")]
            {
                self.run_riscv64()?
            }

            #[cfg(target_arch = "x86_64")]
            {
                self.run_x86_64()?
            }

            #[cfg(not(any(target_arch = "aarch64", target_arch = "riscv64", target_arch = "x86_64")))]
            compile_error!("Unsupported architecture")
        };

        // Restore host context
        unsafe {
            #[cfg(target_arch = "aarch64")]
            crate::arch::arm64::restore_context(&host_context);

            #[cfg(target_arch = "riscv64")]
            crate::arch::riscv64::restore_context(&host_context);

            #[cfg(target_arch = "x86_64")]
            crate::arch::x86_64::restore_context(&host_context);
        }

        self.state = VcpuState::Exited;
        *self.exit_info.lock() = Some(exit_info.clone());

        Ok(exit_info)
    }

    #[cfg(target_arch = "aarch64")]
    unsafe fn run_arm64(&self) -> Result<VmExitInfo> {
        // Save guest registers
        let mut guest_regs = self.registers.lock();

        // Load guest context
        // TODO: Load guest registers to CPU

        // Enter guest
        // TODO: Use ERET to EL1

        // Handle VM exit
        let exit_info = self.handle_arm64_exit()?;

        // Save guest registers
        // TODO: Save registers from CPU

        Ok(exit_info)
    }

    #[cfg(target_arch = "riscv64")]
    unsafe fn run_riscv64(&self) -> Result<VmExitInfo> {
        // Similar to ARM64 but RISC-V specific
        todo!("RISC-V VCPU run not implemented")
    }

    #[cfg(target_arch = "x86_64")]
    unsafe fn run_x86_64(&self) -> Result<VmExitInfo> {
        // Use VMLAUNCH/VMRESUME
        todo!("x86_64 VCPU run not implemented")
    }

    #[cfg(target_arch = "aarch64")]
    fn handle_arm64_exit(&self) -> Result<VmExitInfo> {
        let arm64_data = unsafe { &self.arch_data.arm64 };

        // Read exit syndrome
        let esr_el2: u64;
        unsafe {
            core::arch::asm!("mrs {}, esr_el2", out(reg) esr_el2);
        }

        let exit_class = (esr_el2 >> 26) & 0x3F;

        let reason = match exit_class {
            0x20 => VmExitReason::Exception,
            0x21 => VmExitReason::Hypercall,
            0x24 => VmExitReason::MmioAccess,
            _ => VmExitReason::Unknown,
        };

        let exit_info = VmExitInfo {
            reason,
            qualification: esr_el2,
            guest_rip: { let regs = self.registers.lock(); regs.pc },
            instruction_length: 4,
            instruction_info: 0,
            arch_data: VmExitArchData {
                arm64: VmExitArm64Data {
                    esr_el2,
                    far_el2: { let far: u64; core::arch::asm!("mrs {}, far_el2", out(reg) far); far },
                    hpfar_el2: { let hpfar: u64; core::arch::asm!("mrs {}, hpfar_el2", out(reg) hpfar); hpfar },
                }
            },
        };

        Ok(exit_info)
    }

    /// Inject an interrupt into the VCPU
    pub fn inject_interrupt(&self, vector: u32) -> Result<()> {
        #[cfg(target_arch = "aarch64")]
        {
            // Set HCR_EL2.VI to inject virtual IRQ
            let arm64_data = unsafe { &self.arch_data.arm64 };
            // TODO: Set pending interrupt
        }

        #[cfg(target_arch = "riscv64")]
        {
            // Set VSIE pending bits
        }

        #[cfg(target_arch = "x86_64")]
        {
            // Set VMCS interrupt field
        }

        Ok(())
    }

    /// Inject an exception into the VCPU
    pub fn inject_exception(&self, exception: u32, error_code: u32) -> Result<()> {
        // Architecture-specific exception injection
        #[cfg(target_arch = "aarch64")]
        {
            // Set VSESR_EL2
            // Set VTCR_EL2.VSE
        }

        #[cfg(target_arch = "riscv64")]
        {
            // Set VSTVEC and VSEPC
        }

        #[cfg(target_arch = "x86_64")]
        {
            // Set VMCS VM-exit injection fields
        }

        Ok(())
    }
}

// VCPU Manager implementation
static mut VCPU_MANAGER: Option<VcpuManager> = None;
static VCPU_MANAGER_INIT: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

impl VcpuManager {
    /// Create a new VCPU manager
    const fn new() -> Self {
        Self {
            vcpu_id_bitmap: crate::utils::bitmap::Bitmap::new(core::ptr::null_mut(), MAX_VCPUS),
            vcpus: [None; MAX_VCPUS],
            active_vcpus: 0,
        }
    }

    /// Initialize the VCPU manager
    fn init() -> Result<()> {
        unsafe {
            if VCPU_MANAGER.is_none() {
                // TODO: Allocate memory for VCPU ID bitmap
                let bitmap_data = [0u64; (MAX_VCPUS + 63) / 64];
                VCPU_MANAGER = Some(VcpuManager {
                    vcpu_id_bitmap: crate::utils::bitmap::Bitmap::new(bitmap_data.as_ptr() as *mut u64, MAX_VCPUS),
                    vcpus: [None; MAX_VCPUS],
                    active_vcpus: 0,
                });
                VCPU_MANAGER_INIT.store(true, core::sync::atomic::Ordering::Release);
            }
        }
        Ok(())
    }

    /// Get the VCPU manager instance
    fn get() -> &'static mut VcpuManager {
        unsafe {
            VCPU_MANAGER.as_mut().unwrap()
        }
    }

    /// Allocate a VCPU ID
    fn allocate_vcpu_id(&mut self) -> Result<VcpuId> {
        if let Some(index) = self.vcpu_id_bitmap.find_and_set() {
            Ok(index as VcpuId)
        } else {
            Err(Error::ResourceUnavailable)
        }
    }

    /// Free a VCPU ID
    fn free_vcpu_id(&mut self, vcpu_id: VcpuId) -> Result<()> {
        if vcpu_id as usize >= MAX_VCPUS {
            return Err(Error::InvalidArgument);
        }

        if self.vcpu_id_bitmap.clear_bit(vcpu_id as usize) {
            Ok(())
        } else {
            Err(Error::InvalidArgument)
        }
    }
}

/// Initialize VCPU management
pub fn init() -> Result<()> {
    VcpuManager::init()
}

/// Create a VCPU for a VM
pub fn create_vcpu(vm_id: VmId, vcpu_id: VcpuId) -> Result<()> {
    let manager = VcpuManager::get();

    if vcpu_id as usize >= MAX_VCPUS {
        return Err(Error::InvalidArgument);
    }

    if manager.vcpus[vcpu_id as usize].is_some() {
        return Err(Error::ResourceUnavailable);
    }

    // Create VCPU
    let mut vcpu = VirtualCpu::new(vcpu_id, vm_id)?;

    // Initialize VCPU
    vcpu.initialize()?;

    // Store VCPU in manager
    let vcpu_ptr = NonNull::new(Box::into_raw(Box::new(vcpu)) as *mut VirtualCpu)
        .ok_or(Error::OutOfMemory)?;

    manager.vcpus[vcpu_id as usize] = Some(vcpu_ptr);
    manager.active_vcpus += 1;

    // Add VCPU to VM
    crate::core::vmm::vm::add_vcpu(vm_id, vcpu_id)?;

    crate::info!("Created VCPU {} for VM {}", vcpu_id, vm_id);

    Ok(())
}

/// Destroy a VCPU
pub fn destroy_vcpu(vm_id: VmId, vcpu_id: VcpuId) -> Result<()> {
    let manager = VcpuManager::get();

    if vcpu_id as usize >= MAX_VCPUS {
        return Err(Error::InvalidArgument);
    }

    // Get VCPU reference
    let vcpu_ptr = manager.vcpus[vcpu_id as usize]
        .ok_or(Error::NotFound)?;

    let vcpu = unsafe { vcpu_ptr.as_ref() };

    // Check if VCPU can be destroyed
    match vcpu.state() {
        VcpuState::Running => return Err(Error::ResourceBusy),
        _ => {},
    }

    // Remove VCPU from VM
    crate::core::vmm::vm::remove_vcpu(vm_id, vcpu_id)?;

    // Free VCPU
    let _ = unsafe { Box::from_raw(vcpu_ptr.as_ptr()) };
    manager.vcpus[vcpu_id as usize] = None;
    manager.active_vcpus -= 1;

    // Free VCPU ID
    manager.free_vcpu_id(vcpu_id)?;

    crate::info!("Destroyed VCPU {} from VM {}", vcpu_id, vm_id);

    Ok(())
}

/// Run a VCPU
pub fn run_vcpu(vm_id: VmId, vcpu_id: VcpuId) -> Result<VmExitInfo> {
    let manager = VcpuManager::get();

    if vcpu_id as usize >= MAX_VCPUS {
        return Err(Error::InvalidArgument);
    }

    let vcpu_ptr = manager.vcpus[vcpu_id as usize]
        .ok_or(Error::NotFound)?;

    let vcpu = unsafe { vcpu_ptr.as_ref() };

    if vcpu.vm_id() != vm_id {
        return Err(Error::InvalidArgument);
    }

    vcpu.run()
}

/// Inject an interrupt into a VCPU
pub fn inject_interrupt(vm_id: VmId, vcpu_id: VcpuId, vector: u32) -> Result<()> {
    let manager = VcpuManager::get();

    if vcpu_id as usize >= MAX_VCPUS {
        return Err(Error::InvalidArgument);
    }

    let vcpu_ptr = manager.vcpus[vcpu_id as usize]
        .ok_or(Error::NotFound)?;

    let vcpu = unsafe { vcpu_ptr.as_ref() };

    if vcpu.vm_id() != vm_id {
        return Err(Error::InvalidArgument);
    }

    vcpu.inject_interrupt(vector)
}

/// Inject an exception into a VCPU
pub fn inject_exception(vm_id: VmId, vcpu_id: VcpuId, exception: u32, error_code: u32) -> Result<()> {
    let manager = VcpuManager::get();

    if vcpu_id as usize >= MAX_VCPUS {
        return Err(Error::InvalidArgument);
    }

    let vcpu_ptr = manager.vcpus[vcpu_id as usize]
        .ok_or(Error::NotFound)?;

    let vcpu = unsafe { vcpu_ptr.as_ref() };

    if vcpu.vm_id() != vm_id {
        return Err(Error::InvalidArgument);
    }

    vcpu.inject_exception(exception, error_code)
}

/// Get VCPU registers
pub fn get_vcpu_regs(vm_id: VmId, vcpu_id: VcpuId) -> Option<VcpuRegisters> {
    let manager = VcpuManager::get();

    if vcpu_id as usize >= MAX_VCPUS {
        return None;
    }

    let vcpu_ptr = manager.vcpus[vcpu_id as usize]?;
    let vcpu = unsafe { vcpu_ptr.as_ref() };

    if vcpu.vm_id() != vm_id {
        return None;
    }

    Some(vcpu.get_registers())
}

/// Set VCPU registers
pub fn set_vcpu_regs(vm_id: VmId, vcpu_id: VcpuId, regs: &VcpuRegisters) -> Result<()> {
    let manager = VcpuManager::get();

    if vcpu_id as usize >= MAX_VCPUS {
        return Err(Error::InvalidArgument);
    }

    let vcpu_ptr = manager.vcpus[vcpu_id as usize]
        .ok_or(Error::NotFound)?;

    let vcpu = unsafe { vcpu_ptr.as_ref() };

    if vcpu.vm_id() != vm_id {
        return Err(Error::InvalidArgument);
    }

    vcpu.set_registers(regs)
}

/// Get number of VCPUs
pub fn get_vcpu_count() -> usize {
    let manager = VcpuManager::get();
    manager.active_vcpus
}

/// Get number of running VCPUs
pub fn get_running_vcpu_count() -> usize {
    let manager = VcpuManager::get();
    let mut count = 0;

    for vcpu_ptr in manager.vcpus.iter().flatten() {
        let vcpu = unsafe { vcpu_ptr.as_ref() };
        if vcpu.state() == VcpuState::Running {
            count += 1;
        }
    }

    count
}