//! ARM64 EL2 Entry Point
//!
//! This file contains the entry point for ARM64 EL2 (Hypervisor mode).
//! It handles:
//! - Primary CPU boot (_start)
//! - Secondary CPU boot (_start_secondary)
//! - Early initialization
//! - Stack setup
//! - MMU enablement
//!
//! ## Boot Flow
//!
//! 1. Bootloader loads Ferrovisor into memory
//! 2. Jumps to _start (primary CPU) or _start_secondary (secondary CPUs)
//! 3. Check we're in EL2
//! 4. Setup early stack
//! 5. Clear BSS
//! 6. Setup VBAR_EL2 with exception vectors
//! 7. Initialize CPU (enable caches, MMU, etc.)
//! 8. Jump to Rust code
//!
//! ## References
//! - [ARM DDI 0487] ARMv8-A Architecture Reference Manual
//! - [Xvisor cpu_entry.S] (/home/zcxggmu/workspace/hello-projs/posp/xvisor/arch/arm/cpu/arm64/cpu_entry.S)

.section .entry, "ax", %progbits
.align 12

/// Primary CPU entry point
///
/// This is the first code executed when Ferrovisor starts.
/// The bootloader (or QEMU) jumps to this address.
.globl _start
.type _start, @function
_start:
    // -----------------------------------------------------------------------
    // Check we're in EL2 (Hypervisor mode)
    // -----------------------------------------------------------------------
    mrs     x0, currentel          // Read CurrentEL register
    and     x0, x0, #0xc           // Extract EL bits [3:2]
    cmp     x0, #0x8               // EL2 = 0b1000 (in bits [3:2])
    b.ne    hang                   // Hang if not in EL2

    // -----------------------------------------------------------------------
    // Setup early stack pointer
    // -----------------------------------------------------------------------
    ldr     x0, =_stack_top        // Load stack top address
    mov     sp, x0                 // Set stack pointer

    // -----------------------------------------------------------------------
    // Clear BSS section
    // -----------------------------------------------------------------------
    ldr     x0, =_bss_start        // BSS start address
    ldr     x1, =_bss_end          // BSS end address
clear_bss:
    cmp     x0, x1                 // Check if done
    b.ge    bss_done               // Exit if x0 >= x1
    str     xzr, [x0], #8          // Store zero and increment
    b       clear_bss              // Loop
bss_done:

    // -----------------------------------------------------------------------
    // Setup exception vectors
    // -----------------------------------------------------------------------
    ldr     x0, =init_vectors      // Load initial vectors address
    msr     vbar_el2, x0           // Set VBAR_EL2

    // -----------------------------------------------------------------------
    // Disable MMU and caches before setup
    // -----------------------------------------------------------------------
    mrs     x0, sctlr_el2           // Read SCTLR_EL2
    bic     x0, x0, #(1 << 0)      // Clear M bit (MMU disable)
    bic     x0, x0, #(1 << 2)      // Clear C bit (Data cache disable)
    bic     x0, x0, #(1 << 12)     // Clear I bit (Instruction cache disable)
    bic     x0, x0, #(1 << 3)      // Clear A bit (Alignment check disable)
    msr     sctlr_el2, x0           // Write SCTLR_EL2
    isb                             // Instruction sync barrier

    // -----------------------------------------------------------------------
    // Setup CPU-specific registers
    // -----------------------------------------------------------------------
    // Setup CPACR_EL1 (from EL2) to enable FP/SIMD at EL1
    mov     x0, #(0x3 << 20)       // Enable FP/SIMD at EL1 (bits 21:20 = 0b11)
    msr     cpacr_el1, x0           // Write CPACR_EL1

    // -----------------------------------------------------------------------
    // Setup HCR_EL2 (Hypervisor Configuration Register)
    // -----------------------------------------------------------------------
    mov     x0, #0                  // Start with 0
    orr     x0, x0, #(1 << 31)     // RW: EL1 is AArch64
    orr     x0, x0, #(1 << 5)      // CPTR_EL2.TFP: Trap FP/SIMD at EL2 (we'll manage it)
    orr     x0, x0, #(1 << 4)      // TGE: Trap General Exceptions (not using yet)
    orr     x0, x0, #(1 << 3)      // TWE: Trap WFE
    orr     x0, x0, #(1 << 2)      // TWI: Trap WFI
    orr     x0, x0, #(1 << 1)      // VM: Enable stage 2 translation
    msr     hcr_el2, x0             // Write HCR_EL2

    // -----------------------------------------------------------------------
    // Setup VTCR_EL2 (Virtualization Translation Control Register)
    // -----------------------------------------------------------------------
    // Default: 48-bit IPA, 4KB granule, shareable inner/outer
    mov     x0, #(0x3 << 16)       // PS: 48-bit PA (0b11)
    orr     x0, x0, #(0x0 << 14)   // TG0: 4KB granule (0b00)
    orr     x0, x0, #(0x3 << 12)   // SH0: Inner shareable (0b11)
    orr     x0, x0, #(0x1 << 10)   // ORGN0: Normal memory, WB-WA (0b1)
    orr     x0, x0, #(0x1 << 8)    // IRGN0: Normal memory, WB-WA (0b1)
    orr     x0, x0, #(0x0 << 6)    // SL0: Start at level 2 (0b00 for 4KB)
    mov     x1, #64                 // T0SZ: 48-bit VA = 64 - 48 = 16
    orr     x0, x0, x1
    orr     x0, x0, #(1 << 31)     // RES1 bit
    orr     x0, x0, #(1 << 23)     // HD: Stage 2 hardware management of dirty state
    msr     vtcr_el2, x0            // Write VTCR_EL2

    // -----------------------------------------------------------------------
    // Setup VTTBR_EL2 (Virtualization Translation Table Base Register)
    // -----------------------------------------------------------------------
    mov     x0, #0                  // No stage-2 page table yet
    msr     vttbr_el2, x0           // Clear VTTBR_EL2

    // -----------------------------------------------------------------------
    // Data and instruction barriers
    // -----------------------------------------------------------------------
    dsb     sy                      // Data sync barrier
    isb                             // Instruction sync barrier

    // -----------------------------------------------------------------------
    // Jump to Rust code (main)
    // -----------------------------------------------------------------------
    ldr     x0, =rust_entry         // Load Rust entry point address
    bl      rust_main               // Branch to Rust main

    // Should never reach here
hang:
    b       hang                     // Infinite loop
.size _start, . - _start

// ============================================================================
// Secondary CPU Entry Point
// ============================================================================

.globl _start_secondary
.type _start_secondary, @function
_start_secondary:
    // -----------------------------------------------------------------------
    // Check we're in EL2
    // -----------------------------------------------------------------------
    mrs     x0, currentel
    and     x0, x0, #0xc
    cmp     x0, #0x8
    b.ne    hang

    // -----------------------------------------------------------------------
    // Setup stack for secondary CPU
    // -----------------------------------------------------------------------
    // Get CPU ID from MPIDR_EL1
    mrs     x0, mpidr_el1
    and     x0, x0, #0xff           // Extract CPU ID from affinity 0

    // Calculate stack base (stack_size * cpu_id + stack_top)
    ldr     x1, =_stack_top
    mov     x2, #0x4000             // 16KB stack per CPU
    mul     x0, x0, x2
    sub     sp, x1, x0

    // -----------------------------------------------------------------------
    // Setup exception vectors
    // -----------------------------------------------------------------------
    ldr     x0, =vectors            // Use normal vectors (not init_vectors)
    msr     vbar_el2, x0

    // -----------------------------------------------------------------------
    // Jump to Rust secondary CPU entry
    // -----------------------------------------------------------------------
    ldr     x0, =rust_entry
    bl      rust_secondary_main     // Branch to Rust secondary main

    // Should never return
    b       hang
.size _start_secondary, . - _start_secondary

// ============================================================================
// Helper Functions
// ============================================================================

/// Get current CPU ID
/// Returns: x0 = CPU ID (0-7)
.globl get_cpu_id
.type get_cpu_id, @function
get_cpu_id:
    mrs     x0, mpidr_el1
    and     x0, x0, #0xff           // Extract affinity 0
    ret
.size get_cpu_id, . - get_cpu_id

/// Get current exception level
/// Returns: x0 = Current EL (2 = EL2, 1 = EL1, 0 = EL0)
.globl get_current_el
.type get_current_el, @function
get_current_el:
    mrs     x0, currentel
    and     x0, x0, #0xc
    lsr     x0, x0, #2
    ret
.size get_current_el, . - get_current_el

/// Read system register
/// Parameters: x0 = register encoding (Op0, Op1, CRn, CRm, Op2 packed)
/// Returns: x0 = register value
.globl read_sysreg
.type read_sysreg, @function
read_sysreg:
    mrs     x0, s3_0_c0_c0_0        // Placeholder (will use inline asm in Rust)
    ret
.size read_sysreg, . - read_sysreg

/// Write system register
/// Parameters: x0 = register encoding, x1 = value
.globl write_sysreg
.type write_sysreg, @function
write_sysreg:
    msr     s3_0_c0_c0_0, x1        // Placeholder (will use inline asm in Rust)
    ret
.size write_sysreg, . - write_sysreg

// ============================================================================
// Data Definitions
// ============================================================================

/// Stack definitions
.globl _stack_top
_stack_top:
    .quad   0                       // Will be set by linker script

/// BSS section boundaries
.globl _bss_start
.globl _bss_end
_bss_start:
    .quad   0                       // Will be set by linker script
_bss_end:
    .quad   0                       // Will be set by linker script

/// Rust entry point
.extern rust_main
.extern rust_secondary_main
.extern rust_entry
