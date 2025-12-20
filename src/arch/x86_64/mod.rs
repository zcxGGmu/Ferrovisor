//! x86_64 architecture support
//!
//! This module provides x86_64-specific implementations for the hypervisor.

use crate::{Result, Error};
use crate::arch::common::{CpuFeatures, X86_64Context};

/// Initialize x86_64-specific components
pub fn init() -> Result<()> {
    // Check for virtualization support
    if !check_vt_support() {
        return Err(Error::Unsupported);
    }

    // Initialize VMX
    init_vmx();

    // Initialize APIC
    init_apic();

    // Initialize timer
    init_timer();

    Ok(())
}

/// Check for Intel VT-x or AMD-V support
fn check_vt_support() -> bool {
    unsafe {
        // Check CPUID.1:ECX.VMX for Intel VT-x
        let mut cpuid_result = [0u32; 4];
        x86_64::instructions::cpuid::cpuid(1, 0, &mut cpuid_result);

        let has_vmx = (cpuid_result[2] >> 5) & 0x1 != 0;
        if has_vmx {
            crate::info!("Intel VT-x detected");
            return true;
        }

        // Check CPUID.8000_0001:ECX.SVM for AMD-V
        x86_64::instructions::cpuid::cpuid(0x8000_0001, 0, &mut cpuid_result);
        let has_svm = (cpuid_result[2] >> 2) & 0x1 != 0;
        if has_svm {
            crate::info!("AMD-V detected");
            return true;
        }

        false
    }
}

/// Initialize VMX (Intel VT-x)
fn init_vmx() {
    unsafe {
        // Check if VMX is already enabled
        let cr4 = x86_64::registers::control::Cr4::read();
        if !cr4.contains(x86_64::registers::control::Cr4Flags::VIRTUAL_MACHINE_EXTENSIONS) {
            // Enable VMX in CR4
            let mut new_cr4 = cr4;
            new_cr4.insert(x86_64::registers::control::Cr4Flags::VIRTUAL_MACHINE_EXTENSIONS);
            x86_64::registers::control::Cr4::write(new_cr4);
        }

        // Get VMX basic information
        let mut vmx_basic = x86_64::registers::model_specific::Msr::new(0x480);
        let vmx_basic_msr = vmx_basic.read();

        let vmcs_revision_id = vmx_basic_msr & 0x7FFFFFFF;
        crate::info!("VMCS revision ID: 0x{:x}", vmcs_revision_id);

        // Adjust VMXON region size based on dual-monitor support
        let dual_monitor = (vmx_basic_msr >> 48) & 0x1 != 0;
        let vmxon_size = if dual_monitor { 0 } else { 0 };

        // TODO: Allocate and initialize VMXON region
        crate::info!("VMX initialization skipped (TODO)");
    }
}

/// Initialize the APIC
fn init_apic() {
    unsafe {
        // Check for APIC support
        let mut cpuid_result = [0u32; 4];
        x86_64::instructions::cpuid::cpuid(1, 0, &mut cpuid_result);

        let has_apic = (cpuid_result[3] >> 9) & 0x1 != 0;
        if has_apic {
            // Enable APIC
            let mut apic_base = x86_64::registers::model_specific::Msr::new(0x1B);
            let mut apic_base_value = apic_base.read();
            apic_base_value |= 0x800; // Enable APIC
            apic_base.write(apic_base_value);

            crate::info!("APIC enabled at 0x{:x}", apic_base_value & 0xFFFFF000);
        } else {
            crate::error!("APIC not supported!");
        }
    }
}

/// Initialize the timer
fn init_timer() {
    // Initialize APIC timer
    unsafe {
        let apic_base = (x86_64::registers::model_specific::Msr::new(0x1B).read() & 0xFFFFF000) as *mut u8;

        // Configure timer (divide by 16)
        apic_base.add(0x3E0).write_volatile(0x3u32);

        // Set initial count
        apic_base.add(0x380).write_volatile(0x100000u32);

        // Unmask timer interrupt
        let lvt_timer = apic_base.add(0x320).read_volatile::<u32>();
        apic_base.add(0x320).write_volatile(lvt_timer & !0x10000);
    }

    crate::info!("APIC timer initialized");
}

/// Get x86_64 CPU features
pub fn get_cpu_features() -> crate::arch::common::CpuFeatures {
    let mut features = crate::arch::common::CpuFeatures::default();

    unsafe {
        let mut cpuid_result = [0u32; 4];

        // Check virtualization support
        x86_64::instructions::cpuid::cpuid(1, 0, &mut cpuid_result);
        let has_vmx = (cpuid_result[2] >> 5) & 0x1 != 0;
        x86_64::instructions::cpuid::cpuid(0x8000_0001, 0, &mut cpuid_result);
        let has_svm = (cpuid_result[2] >> 2) & 0x1 != 0;
        features.virtualization = has_vmx || has_svm;

        // Check for EPT/NPT (Second Level Address Translation)
        if has_vmx {
            x86_64::instructions::cpuid::cpuid(1, 0, &mut cpuid_result);
            let has_ept = (cpuid_result[2] >> 6) & 0x1 != 0;
            features.slat = has_ept;
        } else if has_svm {
            // AMD-V always has NPT
            features.slat = true;
        }

        // Check for large pages (2MB/1GB)
        features.large_pages = true; // x86_64 supports various page sizes

        // Check for nested virtualization
        if has_vmx {
            let vmx_basic = x86_64::registers::model_specific::Msr::new(0x480).read();
            features.nested_virt = (vmx_basic >> 55) & 0x1 != 0;
        }

        // Performance counters are always available on x86_64
        features.perf_counters = true;
    }

    features
}

/// Enter VMX operation
pub fn enter_vmx() {
    unsafe {
        // TODO: VMXON
        crate::info!("VMX entry skipped (TODO)");
    }
}

/// Save current CPU context
pub fn save_context(context: &mut crate::arch::common::CpuContext) {
    unsafe {
        // Save general purpose registers
        core::arch::asm!(
            "mov [{} + 0], rax",
            "mov [{} + 8], rbx",
            "mov [{} + 16], rcx",
            "mov [{} + 24], rdx",
            "mov [{} + 32], rsi",
            "mov [{} + 40], rdi",
            "mov [{} + 48], rbp",
            "mov [{} + 56], r8",
            "mov [{} + 64], r9",
            "mov [{} + 72], r10",
            "mov [{} + 80], r11",
            "mov [{} + 88], r12",
            "mov [{} + 96], r13",
            "mov [{} + 104], r14",
            "mov [{} + 112], r15",
            in(reg) &mut context.gpr[0],
        );

        // Save RIP, RSP, and RFLAGS
        let rip: u64;
        let rsp: u64;
        let rflags: u64;
        core::arch::asm!(
            "lea {}, [rip + 1]",
            "mov {}, rsp",
            "pushf",
            "pop {}",
            out(reg) rip,
            out(reg) rsp,
            out(reg) rflags,
        );

        context.pc = rip;
        context.sp = rsp;
        context.psr = rflags;

        // Save x86_64 specific registers
        let x86_context = unsafe { &mut context.arch_specific.x86_64 };
        x86_context.cr0 = x86_64::registers::control::Cr0::read().bits();
        x86_context.cr2 = x86_64::registers::control::Cr2::read().bits();
        x86_context.cr3 = x86_64::registers::control::Cr3::read().bits();
        x86_context.cr4 = x86_64::registers::control::Cr4::read().bits();
        x86_context.rflags = rflags;

        // FS and GS bases
        let fs_base: u64;
        let gs_base: u64;
        let kernel_gs_base: u64;
        core::arch::asm!("rdfsbase {}", out(reg) fs_base);
        core::arch::asm!("rdgsbase {}", out(reg) gs_base);
        core::arch::asm!("swapgs; rdgsbase {}; swapgs", out(reg) kernel_gs_base);

        x86_context.fs_base = fs_base;
        x86_context.gs_base = gs_base;
        x86_context.kernel_gs_base = kernel_gs_base;
    }
}

/// Restore CPU context
pub fn restore_context(context: &crate::arch::common::CpuContext) {
    unsafe {
        // Restore x86_64 specific registers
        let x86_context = &context.arch_specific.x86_64;
        x86_64::registers::control::Cr0::write(x86_64::registers::control::Cr0::from_bits_truncate(x86_context.cr0));
        x86_64::registers::control::Cr2::write(x86_context.cr2);
        x86_64::registers::control::Cr3::write(x86_64::registers::control::Cr3::from_bits_truncate(x86_context.cr3));
        x86_64::registers::control::Cr4::write(x86_64::registers::control::Cr4::from_bits_truncate(x86_context.cr4));

        // Restore FS and GS bases
        core::arch::asm!("wrfsbase {}", in(reg) x86_context.fs_base);
        core::arch::asm!("wrgsbase {}", in(reg) x86_context.gs_base);
        core::arch::asm!("swapgs; wrgsbase {}; swapgs", in(reg) x86_context.kernel_gs_base);

        // Restore general purpose registers
        core::arch::asm!(
            "mov rax, [{} + 0]",
            "mov rbx, [{} + 8]",
            "mov rcx, [{} + 16]",
            "mov rdx, [{} + 24]",
            "mov rsi, [{} + 32]",
            "mov rdi, [{} + 40]",
            "mov rbp, [{} + 48]",
            "mov r8, [{} + 56]",
            "mov r9, [{} + 64]",
            "mov r10, [{} + 72]",
            "mov r11, [{} + 80]",
            "mov r12, [{} + 88]",
            "mov r13, [{} + 96]",
            "mov r14, [{} + 104]",
            "mov r15, [{} + 112]",
            in(reg) &context.gpr[0],
        );

        // Restore RSP, RFLAGS, and jump to RIP
        core::arch::asm!(
            "mov rsp, {}",
            "push {}",
            "popf",
            "jmp {}",
            in(reg) context.sp,
            in(reg) context.psr,
            in(reg) context.pc,
        );
    }
}

/// Architecture-specific main loop
pub fn run() -> ! {
    crate::info!("x86_64 hypervisor running");

    loop {
        // Main hypervisor loop
        x86_64::instructions::hlt();
    }
}

/// Panic handler for x86_64
pub fn panic(info: &core::panic::PanicInfo) -> ! {
    crate::error!("x86_64 Panic: {}", info);

    // Output panic info via serial port
    if let Some(location) = info.location() {
        crate::error!("  at {}:{}:{}", location.file(), location.line(), location.column());
    }

    if let Some(message) = info.message() {
        crate::error!("  message: {}", message);
    }

    // Halt the system
    loop {
        x86_64::instructions::hlt();
    }
}

/// Exception personality routine
pub extern "C" fn eh_personality() {
    // Rust exception handling - not used in bare metal
}