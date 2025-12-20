//! RISC-V Virtual Machine (VM) Management
//!
//! This module provides VM management functionality including:
//! - VM lifecycle management
/// - Guest physical memory management
/// - Virtual device management
/// - VM configuration

use crate::arch::riscv64::*;
use crate::arch::riscv64::mmu::*;
use crate::arch::riscv64::virtualization::vcpu::*;
use crate::arch::riscv64::virtualization::hextension::*;
use bitflags::bitflags;

/// VM state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmState {
    /// VM is not initialized
    Uninitialized,
    /// VM is created but not running
    Created,
    /// VM is running
    Running,
    /// VM is paused
    Paused,
    /// VM has stopped
    Stopped,
    /// VM has crashed
    Crashed,
}

/// VM configuration flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct VmFlags: u32 {
        /// Enable two-stage translation (GPA to HPA)
        const TWO_STAGE_TRANSLATION = 1 << 0;
        /// Enable virtual interrupts
        const VIRTUAL_INTERRUPTS = 1 << 1;
        /// Enable virtual timer
        const VIRTUAL_TIMER = 1 << 2;
        /// Enable nested virtualization
        const NESTED_VIRTUALIZATION = 1 << 3;
        /// Enable debug support
        const DEBUG_SUPPORT = 1 << 4;
        /// Enable IOMMU support
        const IOMMU_SUPPORT = 1 << 5;
        /// Enable hardware virtualization features
        const HW_VIRTUALIZATION = 1 << 6;
    }
}

/// Virtual Machine
pub struct VirtualMachine {
    /// VM ID (unique across the system)
    pub id: u16,
    /// VM name (for debugging)
    pub name: String,
    /// VM state
    pub state: VmState,
    /// VM flags
    pub flags: VmFlags,
    /// VMID for this VM
    pub vmid: u16,
    /// Guest physical address space
    pub guest_memory: GuestPhysicalMemory,
    /// Stage-2 page table (GPA -> HPA)
    pub stage2_ptable: RootPageTable,
    /// VCPU manager
    pub vcpu_manager: VcpuManager,
    /// Virtual devices
    pub devices: Vec<Box<dyn VirtualDevice>>,
    /// VM configuration
    pub config: VmConfig,
}

/// VM configuration
#[derive(Debug, Clone)]
pub struct VmConfig {
    /// Number of VCPUs
    pub num_vcpus: u8,
    /// Amount of guest physical memory (in bytes)
    pub memory_size: usize,
    /// Initial entry point
    pub entry_point: usize,
    /// Initial stack pointer
    pub stack_pointer: usize,
    /// Kernel command line
    pub kernel_cmdline: String,
    /// Device tree blob address
    pub dtb_address: usize,
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            num_vcpus: 1,
            memory_size: 128 * 1024 * 1024, // 128MB
            entry_point: 0x80000000,
            stack_pointer: 0x80100000,
            kernel_cmdline: String::new(),
            dtb_address: 0,
        }
    }
}

/// Guest physical memory management
pub struct GuestPhysicalMemory {
    /// Base physical address
    pub base_pa: usize,
    /// Size of guest memory
    pub size: usize,
    /// Memory regions
    pub regions: Vec<MemRegion>,
}

impl GuestPhysicalMemory {
    /// Create new guest physical memory
    pub fn new(base_pa: usize, size: usize) -> Self {
        Self {
            base_pa,
            size,
            regions: Vec::new(),
        }
    }

    /// Add a memory region
    pub fn add_region(&mut self, region: MemRegion) -> Result<(), &'static str> {
        // Check if region overlaps with existing regions
        for existing in &self.regions {
            if (region.va_start < existing.va_end() && region.va_end() > existing.va_start) ||
               (region.pa_start < existing.pa_end() && region.pa_end() > existing.pa_start) {
                return Err("Memory region overlaps with existing region");
            }
        }

        // Check if region is within bounds
        if region.va_start < self.base_pa || region.va_end() > self.base_pa + self.size {
            return Err("Memory region out of bounds");
        }

        self.regions.push(region);
        Ok(())
    }

    /// Get memory region by guest physical address
    pub fn get_region(&self, gpa: usize) -> Option<&MemRegion> {
        self.regions.iter().find(|r| r.contains_pa(gpa))
    }

    /// Translate guest physical address to host physical address
    pub fn translate_gpa_to_hpa(&self, gpa: usize) -> Option<usize> {
        if let Some(region) = self.get_region(gpa) {
            let offset = gpa - region.pa_start;
            Some(region.va_start + offset)
        } else {
            None
        }
    }

    /// Check if guest physical address range is valid
    pub fn is_valid_range(&self, gpa: usize, size: usize) -> bool {
        gpa >= self.base_pa && gpa + size <= self.base_pa + self.size
    }
}

/// Virtual device trait
pub trait VirtualDevice {
    /// Get device ID
    fn device_id(&self) -> u32;

    /// Get device name
    fn device_name(&self) -> &str;

    /// Initialize the device
    fn init(&mut self, vm: &mut VirtualMachine) -> Result<(), &'static str>;

    /// Handle MMIO access
    fn handle_mmio(&mut self, gpa: usize, is_write: bool, value: u64) -> Result<u64, &'static str>;

    /// Handle interrupt (if device generates interrupts)
    fn handle_interrupt(&mut self) -> Result<(), &'static str>;

    /// Get device configuration
    fn get_config(&self) -> &VmDeviceConfig;
}

/// VM device configuration
#[derive(Debug, Clone)]
pub struct VmDeviceConfig {
    /// Device type
    pub device_type: String,
    /// Base address
    pub base_addr: usize,
    /// Size of MMIO region
    pub mmio_size: usize,
    /// Number of IRQs
    pub num_irqs: u32,
    /// Device-specific parameters
    pub params: std::collections::HashMap<String, String>,
}

impl VirtualMachine {
    /// Create a new virtual machine
    pub fn new(id: u16, name: String, config: VmConfig, flags: VmFlags) -> Result<Self, &'static str> {
        log::info!("Creating VM {}: {} ({} VCPUs, {}MB memory)",
                  id, name, config.num_vcpus, config.memory_size / (1024 * 1024));

        // Allocate VMID
        let vmid = 1; // This would be allocated by the H extension manager

        // Create guest physical memory
        let guest_memory = GuestPhysicalMemory::new(0x40000000, config.memory_size);

        // Create stage-2 page table
        let stage2_ptable = RootPageTable::new(8, vmid as Asid)?;

        // Create VCPU manager
        let vcpu_manager = VcpuManager::new();

        let vm = Self {
            id,
            name,
            state: VmState::Uninitialized,
            flags,
            vmid,
            guest_memory,
            stage2_ptable,
            vcpu_manager,
            devices: Vec::new(),
            config,
        };

        log::info!("VM {} created with VMID {}", id, vmid);
        Ok(vm)
    }

    /// Initialize the VM
    pub fn init(&mut self) -> Result<(), &'static str> {
        log::info!("Initializing VM {}", self.id);

        // Map guest physical memory
        self.map_guest_memory()?;

        // Create VCPUs
        self.create_vcpus()?;

        // Initialize virtual devices
        self.init_devices()?;

        // Set up device tree
        self.setup_device_tree()?;

        // Set state to created
        self.state = VmState::Created;

        log::info!("VM {} initialized successfully", self.id);
        Ok(())
    }

    /// Map guest physical memory to host physical memory
    fn map_guest_memory(&mut self) -> Result<(), &'static str> {
        log::debug!("Mapping guest physical memory for VM {}", self.id);

        // Add guest memory region
        let memory_region = MemRegion::new(
            self.guest_memory.base_pa,    // GPA
            0x80000000,                    // HPA (assume identity mapping for now)
            self.guest_memory.size,
            MemFlags::READABLE | MemFlags::WRITABLE,
            "guest_memory",
        );

        self.guest_memory.add_region(memory_region)?;
        self.stage2_ptable.map_region(memory_region)?;

        // Add standard MMIO regions
        let uart_region = MemRegion::new(
            0x10000000,                    // UART base GPA
            0x10000000,                    // UART base HPA
            0x1000,                        // UART size
            MemFlags::READABLE | MemFlags::WRITABLE | MemFlags::DEVICE,
            "uart",
        );

        self.guest_memory.add_region(uart_region)?;
        self.stage2_ptable.map_region(uart_region)?;

        Ok(())
    }

    /// Create VCPUs for the VM
    fn create_vcpus(&mut self) -> Result<(), &'static str> {
        log::debug!("Creating {} VCPUs for VM {}", self.config.num_vcpus, self.id);

        for i in 0..self.config.num_vcpus {
            let vcpu_flags = if self.flags.contains(VmFlags::VIRTUAL_INTERRUPTS) {
                VcpuFlags::VIRTUAL_INTERRUPTS
            } else {
                VcpuFlags::empty()
            };

            let vcpu = self.vcpu_manager.allocate_vcpu(self.vmid, vcpu_flags)?;

            // Initialize VCPU with entry point and stack
            let entry_point = self.config.entry_point;
            let stack_size = 64 * 1024; // 64KB stack per VCPU
            let stack_top = self.guest_memory.base_pa + self.guest_memory.size - ((i as usize + 1) * stack_size);

            vcpu.init(entry_point, stack_top)?;

            log::debug!("Created VCPU {} for VM {}", i, self.id);
        }

        Ok(())
    }

    /// Initialize virtual devices
    fn init_devices(&mut self) -> Result<(), &'static str> {
        log::debug!("Initializing devices for VM {}", self.id);

        // Initialize each device
        for device in &mut self.devices {
            device.init(self)?;
            log::debug!("Initialized device: {}", device.device_name());
        }

        Ok(())
    }

    /// Set up device tree for the VM
    fn setup_device_tree(&mut self) -> Result<(), &'static str> {
        log::debug!("Setting up device tree for VM {}", self.id);

        // This would typically:
        // 1. Create a device tree blob
        // 2. Add CPU nodes
        // 3. Add memory nodes
        // 4. Add device nodes
        // 5. Add chosen node with boot arguments

        self.config.dtb_address = 0x41000000; // Placeholder

        Ok(())
    }

    /// Start the VM
    pub fn start(&mut self) -> Result<(), &'static str> {
        if self.state != VmState::Created {
            return Err("VM must be created before starting");
        }

        log::info!("Starting VM {}", self.id);

        // Activate stage-2 translation if enabled
        if self.flags.contains(VmFlags::TWO_STAGE_TRANSLATION) {
            self.activate_stage2_translation()?;
        }

        // Schedule first VCPU
        if let Some(vcpu) = self.vcpu_manager.get_next_ready_vcpu() {
            self.vcpu_manager.schedule_vcpu(vcpu.id)?;
        }

        self.state = VmState::Running;
        log::info!("VM {} started", self.id);
        Ok(())
    }

    /// Pause the VM
    pub fn pause(&mut self) -> Result<(), &'static str> {
        if self.state != VmState::Running {
            return Err("VM is not running");
        }

        log::info!("Pausing VM {}", self.id);

        // Save state of all VCPUs
        for vcpu in self.vcpu_manager.get_vcpus_mut() {
            if vcpu.is_running() {
                vcpu.save_state()?;
                vcpu.set_state(VcpuState::Blocked);
            }
        }

        self.state = VmState::Paused;
        Ok(())
    }

    /// Resume the VM
    pub fn resume(&mut self) -> Result<(), &'static str> {
        if self.state != VmState::Paused {
            return Err("VM is not paused");
        }

        log::info!("Resuming VM {}", self.id);

        // Resume all blocked VCPUs
        for vcpu in self.vcpu_manager.get_vcpus_mut() {
            if vcpu.state == VcpuState::Blocked {
                vcpu.set_state(VcpuState::Ready);
            }
        }

        // Schedule a VCPU
        if let Some(vcpu) = self.vcpu_manager.get_next_ready_vcpu() {
            self.vcpu_manager.schedule_vcpu(vcpu.id)?;
        }

        self.state = VmState::Running;
        Ok(())
    }

    /// Stop the VM
    pub fn stop(&mut self) -> Result<(), &'static str> {
        log::info!("Stopping VM {}", self.id);

        // Set all VCPUs to exited state
        for vcpu in self.vcpu_manager.get_vcpus_mut() {
            vcpu.set_state(VcpuState::Exited);
        }

        self.state = VmState::Stopped;
        Ok(())
    }

    /// Add a virtual device
    pub fn add_device(&mut self, device: Box<dyn VirtualDevice>) {
        log::debug!("Adding device {} to VM {}", device.device_name(), self.id);
        self.devices.push(device);
    }

    /// Handle MMIO access
    pub fn handle_mmio(&mut self, gpa: usize, is_write: bool, value: u64) -> Result<u64, &'static str> {
        // Find device that handles this GPA
        for device in &mut self.devices {
            let config = device.get_config();
            if gpa >= config.base_addr && gpa < config.base_addr + config.mmio_size {
                return device.handle_mmio(gpa, is_write, value);
            }
        }

        Err("No device handles this MMIO address")
    }

    /// Inject virtual interrupt into the VM
    pub fn inject_interrupt(&mut self, interrupt_id: u32) -> Result<(), &'static str> {
        if !self.flags.contains(VmFlags::VIRTUAL_INTERRUPTS) {
            return Err("Virtual interrupts not enabled");
        }

        self.vcpu_manager.inject_interrupt_to_vm(self.vmid, interrupt_id)
    }

    /// Activate stage-2 translation
    fn activate_stage2_translation(&self) -> Result<(), &'static str> {
        log::debug!("Activating stage-2 translation for VM {}", self.id);

        // Set HGATP with stage-2 page table
        let hgatp = crate::arch::riscv64::cpu::csr::virtualization::HGATP::make(
            self.stage2_ptable.root().ppn(),
            self.vmid as usize,
            8, // Sv39 mode
        );

        crate::arch::riscv64::cpu::csr::write_csr!(hcsr::HGATP, hgatp);

        // Invalidate all stage-2 TLB entries
        crate::arch::riscv64::cpu::asm::hfence_gvma();

        Ok(())
    }

    /// Get VM statistics
    pub fn get_stats(&self) -> VmStats {
        VmStats {
            id: self.id,
            state: self.state,
            num_vcpus: self.vcpu_manager.vcpu_count(),
            memory_size: self.guest_memory.size,
            vcpu_stats: self.vcpu_manager.get_all_stats(),
        }
    }
}

/// VM statistics
#[derive(Debug, Clone)]
pub struct VmStats {
    pub id: u16,
    pub state: VmState,
    pub num_vcpus: usize,
    pub memory_size: usize,
    pub vcpu_stats: Vec<(u8, VcpuStats)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_creation() {
        let config = VmConfig::default();
        let vm = VirtualMachine::new(
            1,
            "test_vm".to_string(),
            config,
            VmFlags::VIRTUAL_INTERRUPTS,
        ).unwrap();

        assert_eq!(vm.id, 1);
        assert_eq!(vm.name, "test_vm");
        assert_eq!(vm.state, VmState::Uninitialized);
        assert_eq!(vm.vmid, 1);
    }

    #[test]
    fn test_guest_physical_memory() {
        let mut gpm = GuestPhysicalMemory::new(0x40000000, 0x10000000);

        let region = MemRegion::new(
            0x40000000,
            0x80000000,
            0x1000,
            MemFlags::READABLE | MemFlags::WRITABLE,
            "test_region",
        );

        gpm.add_region(region).unwrap();

        assert!(gpm.is_valid_range(0x40000000, 0x1000));
        assert!(!gpm.is_valid_range(0x50000000, 0x1000));

        let hpa = gpm.translate_gpa_to_hpa(0x40000100);
        assert_eq!(hpa, Some(0x80000100));
    }

    #[test]
    fn test_vm_lifecycle() {
        let mut vm = VirtualMachine::new(
            1,
            "test_vm".to_string(),
            VmConfig::default(),
            VmFlags::empty(),
        ).unwrap();

        // Initialize VM
        vm.init().unwrap();
        assert_eq!(vm.state, VmState::Created);

        // Start VM
        vm.start().unwrap();
        assert_eq!(vm.state, VmState::Running);

        // Pause VM
        vm.pause().unwrap();
        assert_eq!(vm.state, VmState::Paused);

        // Resume VM
        vm.resume().unwrap();
        assert_eq!(vm.state, VmState::Running);

        // Stop VM
        vm.stop().unwrap();
        assert_eq!(vm.state, VmState::Stopped);
    }
}