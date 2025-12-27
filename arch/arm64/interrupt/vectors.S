//! ARM64 Exception Vector Table
//!
//! This file contains the exception vectors for ARM64 EL2 (Hypervisor mode).
//!
//! ## Exception Vector Layout
//!
//! The exception vector table consists of 16 entries (4 exception types × 4 exception sources):
//!
//! | Offset | Exception Source        | Description               |
//! |--------|------------------------|---------------------------|
//! | 0x000  | Synchronous EL1t       | Current EL with SP0       |
//! | 0x080  | IRQ EL1t               | Current EL with SP0       |
//! | 0x100  | FIQ EL1t               | Current EL with SP0       |
//! | 0x180  | SError EL1t            | Current EL with SP0       |
//! | 0x200  | Synchronous EL1h       | Current EL with SPx       |
//! | 0x280  | IRQ EL1h               | Current EL with SPx       |
//! | 0x300  | FIQ EL1h               | Current EL with SPx       |
//! | 0x380  | SError EL1h            | Current EL with SPx       |
//! | 0x400  | Synchronous 64-bit EL0 | Lower EL (AArch64)        |
//! | 0x480  | IRQ 64-bit EL0         | Lower EL (AArch64)        |
//! | 0x500  | FIQ 64-bit EL0         | Lower EL (AArch64)        |
//! | 0x580  | SError 64-bit EL0      | Lower EL (AArch64)        |
//! | 0x600  | Synchronous 32-bit EL0 | Lower EL (AArch32)        |
//! | 0x680  | IRQ 32-bit EL0         | Lower EL (AArch32)        |
//! | 0x700  | FIQ 32-bit EL0         | Lower EL (AArch32)        |
//! | 0x780  | SError 32-bit EL0      | Lower EL (AArch32)        |
//!
//! ## References
//! - [ARM DDI 0487] ARMv8-A Architecture Reference Manual
//! - [Xvisor cpu_entry.S] (/home/zcxggmu/workspace/hello-projs/posp/xvisor/arch/arm/cpu/arm64/cpu_entry.S)

.section .vectors, "ax", %progbits
.align 11

// ============================================================================
// Exception Vector Table
// ============================================================================

/// Exception vectors for ARM64 EL2
///
/// Each vector entry must be 128 bytes (0x80) aligned, and the table must be
/// 2048 bytes (0x800) aligned (11 bits = 2^11 = 2048).
.globl vectors
vectors:
    // -----------------------------------------------------------------------
    // EL1t (Current EL with SP0) - Hypervisor using SP0
    // -----------------------------------------------------------------------
    ventry    el2_sync_sp0       // 0x000: Synchronous exception from EL1t
    ventry    el2_irq_sp0        // 0x080: IRQ from EL1t
    ventry    el2_fiq_sp0        // 0x100: FIQ from EL1t
    ventry    el2_serror_sp0     // 0x180: SError from EL1t

    // -----------------------------------------------------------------------
    // EL1h (Current EL with SPx) - Hypervisor using own SP
    // -----------------------------------------------------------------------
    ventry    el2_sync_spx       // 0x200: Synchronous exception from EL1h
    ventry    el2_irq_spx        // 0x280: IRQ from EL1h
    ventry    el2_fiq_spx        // 0x300: FIQ from EL1h
    ventry    el2_serror_spx     // 0x380: SError from EL1h

    // -----------------------------------------------------------------------
    // EL0 (Lower EL AArch64) - 64-bit Guest
    // -----------------------------------------------------------------------
    ventry    guest_sync_a64     // 0x400: Synchronous from 64-bit EL0
    ventry    guest_irq_a64      // 0x480: IRQ from 64-bit EL0
    ventry    guest_fiq_a64      // 0x500: FIQ from 64-bit EL0
    ventry    guest_serror_a64   // 0x580: SError from 64-bit EL0

    // -----------------------------------------------------------------------
    // EL0 (Lower EL AArch32) - 32-bit Guest
    // -----------------------------------------------------------------------
    ventry    guest_sync_a32     // 0x600: Synchronous from 32-bit EL0
    ventry    guest_irq_a32      // 0x680: IRQ from 32-bit EL0
    ventry    guest_fiq_a32      // 0x700: FIQ from 32-bit EL0
    ventry    guest_serror_a32   // 0x780: SError from 32-bit EL0

// ============================================================================
// Vector Entry Helper Macro
// ============================================================================

/// Macro to create a vector entry
/// Each entry must be 128 bytes (0x80) aligned
.macro ventry label
    .align 7
    b    \label
.endm

// ============================================================================
// Exception Handler Macros
// ============================================================================

/// Push all general-purpose registers onto stack
///
/// Saved registers layout:
/// - x0-x30: General-purpose registers
/// - SP, PC, PSTATE: Special registers
/// - ELR_EL2, SPSR_EL2: Exception link and processor state
.macro push_regs
    // Allocate space for all registers
    sub     sp, sp, #272

    // Save general-purpose registers x0-x28
    stp     x0, x1, [sp, #0]
    stp     x2, x3, [sp, #16]
    stp     x4, x5, [sp, #32]
    stp     x6, x7, [sp, #48]
    stp     x8, x9, [sp, #64]
    stp     x10, x11, [sp, #80]
    stp     x12, x13, [sp, #96]
    stp     x14, x15, [sp, #112]
    stp     x16, x17, [sp, #128]
    stp     x18, x19, [sp, #144]
    stp     x20, x21, [sp, #160]
    stp     x22, x23, [sp, #176]
    stp     x24, x25, [sp, #192]
    stp     x26, x27, [sp, #208]
    stp     x28, x29, [sp, #224]

    // Save x30 (LR), SP, ELR_EL2, SPSR_EL2
    mrs     x4, sp_el0              // Save SP_EL0
    mrs     x5, elr_el2             // Save Exception Link Register
    mrs     x6, spsr_el2            // Save Saved Processor State Register
    stp     x30, x4, [sp, #240]     // Save x30 (LR) and SP_EL0
    stp     x5, x6, [sp, #256]      // Save ELR_EL2 and SPSR_EL2
.endm

/// Restore all general-purpose registers from stack
.macro pop_regs
    // Restore ELR_EL2, SPSR_EL2, x30 (LR), SP_EL0
    ldp     x5, x6, [sp, #256]      // Restore ELR_EL2 and SPSR_EL2
    ldp     x30, x4, [sp, #240]     // Restore x30 (LR) and SP_EL0
    msr     elr_el2, x5             // Restore Exception Link Register
    msr     spsr_el2, x6            // Restore Saved Processor State Register
    msr     sp_el0, x4              // Restore SP_EL0

    // Restore general-purpose registers x0-x28
    ldp     x0, x1, [sp, #0]
    ldp     x2, x3, [sp, #16]
    ldp     x4, x5, [sp, #32]
    ldp     x6, x7, [sp, #48]
    ldp     x8, x9, [sp, #64]
    ldp     x10, x11, [sp, #80]
    ldp     x12, x13, [sp, #96]
    ldp     x14, x15, [sp, #112]
    ldp     x16, x17, [sp, #128]
    ldp     x18, x19, [sp, #144]
    ldp     x20, x21, [sp, #160]
    ldp     x22, x23, [sp, #176]
    ldp     x24, x25, [sp, #192]
    ldp     x26, x27, [sp, #208]
    ldp     x28, x29, [sp, #224]

    // Restore stack pointer
    add     sp, sp, #272
.endm

/// Call C exception handler
/// \param handler: C function to call
/// \param exc_type: Exception type identifier
.macro call_handler handler, exc_type
    mov     x0, sp                  // First arg: pointer to registers
    mov     x1, #\exc_type          // Second arg: exception type
    bl      \handler
.endm

// ============================================================================
// EL2 Exception Handlers (Hypervisor mode)
// ============================================================================

// Synchronous exception from EL1t (SP0)
.align 7
el2_sync_sp0:
    push_regs
    call_handler rust_el2_sync_sp0, 0
    pop_regs
    eret

// IRQ from EL1t (SP0)
.align 7
el2_irq_sp0:
    push_regs
    call_handler rust_el2_irq_sp0, 1
    pop_regs
    eret

// FIQ from EL1t (SP0)
.align 7
el2_fiq_sp0:
    push_regs
    call_handler rust_el2_fiq_sp0, 2
    pop_regs
    eret

// SError from EL1t (SP0)
.align 7
el2_serror_sp0:
    push_regs
    call_handler rust_el2_serror_sp0, 3
    pop_regs
    eret

// Synchronous exception from EL1h (SPx)
.align 7
el2_sync_spx:
    push_regs
    call_handler rust_el2_sync_spx, 4
    pop_regs
    eret

// IRQ from EL1h (SPx)
.align 7
el2_irq_spx:
    push_regs
    call_handler rust_el2_irq_spx, 5
    pop_regs
    eret

// FIQ from EL1h (SPx)
.align 7
el2_fiq_spx:
    push_regs
    call_handler rust_el2_fiq_spx, 6
    pop_regs
    eret

// SError from EL1h (SPx)
.align 7
el2_serror_spx:
    push_regs
    call_handler rust_el2_serror_spx, 7
    pop_regs
    eret

// ============================================================================
// Guest Exception Handlers (Lower EL - Guest OS)
// ============================================================================

// Synchronous exception from 64-bit EL0 (Guest)
.align 7
guest_sync_a64:
    push_regs
    call_handler rust_guest_sync_a64, 8
    pop_regs
    eret

// IRQ from 64-bit EL0 (Guest)
.align 7
guest_irq_a64:
    push_regs
    call_handler rust_guest_irq_a64, 9
    pop_regs
    eret

// FIQ from 64-bit EL0 (Guest)
.align 7
guest_fiq_a64:
    push_regs
    call_handler rust_guest_fiq_a64, 10
    pop_regs
    eret

// SError from 64-bit EL0 (Guest)
.align 7
guest_serror_a64:
    push_regs
    call_handler rust_guest_serror_a64, 11
    pop_regs
    eret

// Synchronous exception from 32-bit EL0 (Guest)
.align 7
guest_sync_a32:
    push_regs
    call_handler rust_guest_sync_a32, 12
    pop_regs
    eret

// IRQ from 32-bit EL0 (Guest)
.align 7
guest_irq_a32:
    push_regs
    call_handler rust_guest_irq_a32, 13
    pop_regs
    eret

// FIQ from 32-bit EL0 (Guest)
.align 7
guest_fiq_a32:
    push_regs
    call_handler rust_guest_fiq_a32, 14
    pop_regs
    eret

// SError from 32-bit EL0 (Guest)
.align 7
guest_serror_a32:
    push_regs
    call_handler rust_guest_serror_a32, 15
    pop_regs
    eret

// ============================================================================
// Initialization Exception Vectors
// ============================================================================

/// Initial exception vectors used during boot
/// These vectors simply jump to the initialization code
.section .vectors, "ax", %progbits
.align 11
.globl init_vectors
init_vectors:
    ventry    init_entry    // Synchronous EL1t
    ventry    init_entry    // IRQ EL1t
    ventry    init_entry    // FIQ EL1t
    ventry    init_entry    // Error EL1t

    ventry    init_entry    // Synchronous EL1h
    ventry    init_entry    // IRQ EL1h
    ventry    init_entry    // FIQ EL1h
    ventry    init_entry    // Error EL1h

    ventry    init_entry    // Synchronous 64-bit EL0
    ventry    init_entry    // IRQ 64-bit EL0
    ventry    init_entry    // FIQ 64-bit EL0
    ventry    init_entry    // Error 64-bit EL0

    ventry    init_entry    // Synchronous 32-bit EL0
    ventry    init_entry    // IRQ 32-bit EL0
    ventry    init_entry    // FIQ 32-bit EL0
    ventry    init_entry    // Error 32-bit EL0

.align 7
init_entry:
    // Hang here during initialization
    b       .
