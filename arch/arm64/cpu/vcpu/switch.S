//! VCPU context switch assembly for ARM64
//!
//! Provides low-level VCPU context switching between host and guest.
//! Reference: xvisor/arch/arm/cpu/arm64/cpu_vcpu_switch.S

.global __vcpu_sysregs_save
__vcpu_sysregs_save:
	// x0 = pointer to SysRegs structure
	stp	x1, x2, [sp, #-16]!

	// Save 64-bit EL1/EL0 registers
	mrs	x1, sp_el0
	str	x1, [x0, #0x00]		// sp_el0
	mrs	x1, sp_el1
	str	x1, [x0, #0x08]		// sp_el1
	mrs	x1, elr_el1
	str	x1, [x0, #0x10]		// elr_el1
	mrs	x1, spsr_el1
	str	x1, [x0, #0x18]		// spsr_el1
	mrs	x1, midr_el1
	str	x1, [x0, #0x20]		// midr_el1
	mrs	x1, mpidr_el1
	str	x1, [x0, #0x28]		// mpidr_el1
	mrs	x1, sctlr_el1
	str	x1, [x0, #0x30]		// sctlr_el1
	mrs	x1, actlr_el1
	str	x1, [x0, #0x38]		// actlr_el1
	mrs	x1, cpacr_el1
	str	x1, [x0, #0x40]		// cpacr_el1
	mrs	x1, ttbr0_el1
	str	x1, [x0, #0x48]		// ttbr0_el1
	mrs	x1, ttbr1_el1
	str	x1, [x0, #0x50]		// ttbr1_el1
	mrs	x1, tcr_el1
	str	x1, [x0, #0x58]		// tcr_el1
	mrs	x1, esr_el1
	str	x1, [x0, #0x60]		// esr_el1
	mrs	x1, far_el1
	str	x1, [x0, #0x68]		// far_el1
	mrs	x1, par_el1
	str	x1, [x0, #0x70]		// par_el1
	mrs	x1, mair_el1
	str	x1, [x0, #0x78]		// mair_el1
	mrs	x1, vbar_el1
	str	x1, [x0, #0x80]		// vbar_el1
	mrs	x1, contextidr_el1
	str	x1, [x0, #0x88]		// contextidr_el1
	mrs	x1, tpidr_el0
	str	x1, [x0, #0x90]		// tpidr_el0
	mrs	x1, tpidr_el1
	str	x1, [x0, #0x98]		// tpidr_el1
	mrs	x1, tpidrro_el0
	str	x1, [x0, #0xA0]		// tpidrro_el0

	// Save 32-bit only registers
	mrs	x1, spsr_abt
	str	w1, [x0, #0xA8]		// spsr_abt
	mrs	x1, spsr_und
	str	w1, [x0, #0xAC]		// spsr_und
	mrs	x1, spsr_irq
	str	w1, [x0, #0xB0]		// spsr_irq
	mrs	x1, spsr_fiq
	str	w1, [x0, #0xB4]		// spsr_fiq
	mrs	x1, dacr32_el2
	str	w1, [x0, #0xB8]		// dacr32_el2
	mrs	x1, ifsr32_el2
	str	w1, [x0, #0xBC]		// ifsr32_el2

	// Check for ThumbEE support
	mrs	x1, id_pfr0_el1
	and	x1, x1, #0xF000
	cmp	x1, #0x1000
	b.ne	save_thumbee
	b	save_skip_thumbee
save_thumbee:
	mrs	x1, teecr32_el1
	str	w1, [x0, #0xC0]		// teecr32_el1
	mrs	x1, teehbr32_el1
	str	w1, [x0, #0xC4]		// teehbr32_el1
save_skip_thumbee:

	ldp	x1, x2, [sp], #16
	ret

.global __vcpu_sysregs_restore
__vcpu_sysregs_restore:
	// x0 = pointer to SysRegs structure
	stp	x1, x2, [sp, #-16]!

	// Update VPIDR and VMPIDR
	ldr	x1, [x0, #0x20]		// midr_el1
	msr	vpidr_el2, x1
	ldr	x1, [x0, #0x28]		// mpidr_el1
	msr	vmpidr_el2, x1

	// Restore 64-bit EL1/EL0 registers
	ldr	x1, [x0, #0x00]		// sp_el0
	msr	sp_el0, x1
	ldr	x1, [x0, #0x08]		// sp_el1
	msr	sp_el1, x1
	ldr	x1, [x0, #0x10]		// elr_el1
	msr	elr_el1, x1
	ldr	x1, [x0, #0x18]		// spsr_el1
	msr	spsr_el1, x1
	ldr	x1, [x0, #0x30]		// sctlr_el1
	msr	sctlr_el1, x1
	ldr	x1, [x0, #0x40]		// cpacr_el1
	msr	cpacr_el1, x1
	ldr	x1, [x0, #0x48]		// ttbr0_el1
	msr	ttbr0_el1, x1
	ldr	x1, [x0, #0x50]		// ttbr1_el1
	msr	ttbr1_el1, x1
	ldr	x1, [x0, #0x58]		// tcr_el1
	msr	tcr_el1, x1
	ldr	x1, [x0, #0x60]		// esr_el1
	msr	esr_el1, x1
	ldr	x1, [x0, #0x68]		// far_el1
	msr	far_el1, x1
	ldr	x1, [x0, #0x70]		// par_el1
	msr	par_el1, x1
	ldr	x1, [x0, #0x78]		// mair_el1
	msr	mair_el1, x1
	ldr	x1, [x0, #0x80]		// vbar_el1
	msr	vbar_el1, x1
	ldr	x1, [x0, #0x88]		// contextidr_el1
	msr	contextidr_el1, x1
	ldr	x1, [x0, #0x90]		// tpidr_el0
	msr	tpidr_el0, x1
	ldr	x1, [x0, #0x98]		// tpidr_el1
	msr	tpidr_el1, x1
	ldr	x1, [x0, #0xA0]		// tpidrro_el0
	msr	tpidrro_el0, x1

	// Restore 32-bit only registers
	ldr	w1, [x0, #0xA8]		// spsr_abt
	msr	spsr_abt, x1
	ldr	w1, [x0, #0xAC]		// spsr_und
	msr	spsr_und, x1
	ldr	w1, [x0, #0xB0]		// spsr_irq
	msr	spsr_irq, x1
	ldr	w1, [x0, #0xB4]		// spsr_fiq
	msr	spsr_fiq, x1
	ldr	w1, [x0, #0xB8]		// dacr32_el2
	msr	dacr32_el2, x1
	ldr	w1, [x0, #0xBC]		// ifsr32_el2
	msr	ifsr32_el2, x1

	// Check for ThumbEE support
	mrs	x1, id_pfr0_el1
	and	x1, x1, #0xF000
	cmp	x1, #0x1000
	b.ne	restore_thumbee
	b	restore_skip_thumbee
restore_thumbee:
	ldr	w1, [x0, #0xC0]		// teecr32_el1
	msr	teecr32_el1, x1
	ldr	w1, [x0, #0xC4]		// teehbr32_el1
	msr	teehbr32_el1, x1
restore_skip_thumbee:

	ldp	x1, x2, [sp], #16
	ret

.global __vcpu_vfp_save
__vcpu_vfp_save:
	// x0 = pointer to VfpRegs structure
	stp	x1, x2, [sp, #-16]!

	// Save floating point control registers
	mrs	x1, fpexc32_el2
	str	w1, [x0, #0x00]		// fpexc32_el2
	mrs	x1, fpcr
	str	w1, [x0, #0x04]		// fpcr
	mrs	x1, fpsr
	str	w1, [x0, #0x08]		// fpsr

	// Save floating point registers (q0-q31)
	// q registers are 128-bit (16 bytes each)
	add	x1, x0, #0x10		// Skip to fpregs
	stp	q0, q1, [x1, #0x000]
	stp	q2, q3, [x1, #0x020]
	stp	q4, q5, [x1, #0x040]
	stp	q6, q7, [x1, #0x060]
	stp	q8, q9, [x1, #0x080]
	stp	q10, q11, [x1, #0x0A0]
	stp	q12, q13, [x1, #0x0C0]
	stp	q14, q15, [x1, #0x0E0]
	stp	q16, q17, [x1, #0x100]
	stp	q18, q19, [x1, #0x120]
	stp	q20, q21, [x1, #0x140]
	stp	q22, q23, [x1, #0x160]
	stp	q24, q25, [x1, #0x180]
	stp	q26, q27, [x1, #0x1A0]
	stp	q28, q29, [x1, #0x1C0]
	stp	q30, q31, [x1, #0x1E0]

	ldp	x1, x2, [sp], #16
	ret

.global __vcpu_vfp_restore
__vcpu_vfp_restore:
	// x0 = pointer to VfpRegs structure
	stp	x1, x2, [sp, #-16]!

	// Restore floating point registers (q0-q31)
	add	x1, x0, #0x10		// Skip to fpregs
	ldp	q0, q1, [x1, #0x000]
	ldp	q2, q3, [x1, #0x020]
	ldp	q4, q5, [x1, #0x040]
	ldp	q6, q7, [x1, #0x060]
	ldp	q8, q9, [x1, #0x080]
	ldp	q10, q11, [x1, #0x0A0]
	ldp	q12, q13, [x1, #0x0C0]
	ldp	q14, q15, [x1, #0x0E0]
	ldp	q16, q17, [x1, #0x100]
	ldp	q18, q19, [x1, #0x120]
	ldp	q20, q21, [x1, #0x140]
	ldp	q22, q23, [x1, #0x160]
	ldp	q24, q25, [x1, #0x180]
	ldp	q26, q27, [x1, #0x1A0]
	ldp	q28, q29, [x1, #0x1C0]
	ldp	q30, q31, [x1, #0x1E0]

	// Restore floating point control registers
	ldr	w1, [x0, #0x08]		// fpsr
	msr	fpsr, x1
	ldr	w1, [x0, #0x04]		// fpcr
	msr	fpcr, x1
	ldr	w1, [x0, #0x00]		// fpexc32_el2
	msr	fpexc32_el2, x1

	ldp	x1, x2, [sp], #16
	ret

.global __vcpu_gprs_save
__vcpu_gprs_save:
	// x0 = pointer to SavedGprs structure
	// Save x0-x30 (x0 is saved after loading pointer)
	stp	x1, x2, [sp, #-16]!
	stp	x3, x4, [sp, #-16]!
	stp	x5, x6, [sp, #-16]!
	stp	x7, x8, [sp, #-16]!
	stp	x9, x10, [sp, #-16]!
	stp	x11, x12, [sp, #-16]!
	stp	x13, x14, [sp, #-16]!
	stp	x15, x16, [sp, #-16]!
	stp	x17, x18, [sp, #-16]!
	stp	x19, x20, [sp, #-16]!
	stp	x21, x22, [sp, #-16]!
	stp	x23, x24, [sp, #-16]!
	stp	x25, x26, [sp, #-16]!
	stp	x27, x28, [sp, #-16]!
	stp	x29, x30, [sp, #-16]!

	// Store registers to structure
	add	x1, sp, #16*8		// Point to x1/x2 saved position
	stp	x1, x2, [x0, #0x00]	// x1, x2
	add	x1, sp, #48			// Point to x5/x6
	stp	x3, x4, [x0, #0x10]	// x3, x4
	add	x1, sp, #80			// Point to x9/x10
	stp	x5, x6, [x0, #0x20]	// x5, x6
	add	x1, sp, #112			// Point to x13/x14
	stp	x7, x8, [x0, #0x30]	// x7, x8
	add	x1, sp, #144			// Point to x17/x18
	stp	x9, x10, [x0, #0x40]	// x9, x10
	add	x1, sp, #176			// Point to x21/x22
	stp	x11, x12, [x0, #0x50]	// x11, x12
	add	x1, sp, #208			// Point to x25/x26
	stp	x13, x14, [x0, #0x60]	// x13, x14
	ldp	x29, x30, [sp, #16*8]	// Restore x29/x30
	stp	x15, x16, [x0, #0x70]	// x15, x16
	stp	x17, x18, [x0, #0x80]	// x17, x18
	stp	x19, x20, [x0, #0x90]	// x19, x20
	stp	x21, x22, [x0, #0xA0]	// x21, x22
	stp	x23, x24, [x0, #0xB0]	// x23, x24
	stp	x25, x26, [x0, #0xC0]	// x25, x26
	stp	x27, x28, [x0, #0xD0]	// x27, x28
	stp	x29, x30, [x0, #0xE0]	// x29, x30
	mov	x1, sp
	str	x1, [x0, #0xF0]		// sp (original)

	ldp	x27, x28, [sp], #16
	ldp	x25, x26, [sp], #16
	ldp	x23, x24, [sp], #16
	ldp	x21, x22, [sp], #16
	ldp	x19, x20, [sp], #16
	ldp	x17, x18, [sp], #16
	ldp	x15, x16, [sp], #16
	ldp	x13, x14, [sp], #16
	ldp	x11, x12, [sp], #16
	ldp	x9, x10, [sp], #16
	ldp	x7, x8, [sp], #16
	ldp	x5, x6, [sp], #16
	ldp	x3, x4, [sp], #16
	ldp	x1, x2, [sp], #16
	ret

.global __vcpu_gprs_restore
__vcpu_gprs_restore:
	// x0 = pointer to SavedGprs structure
	stp	x1, x2, [sp, #-16]!

	// Load registers from structure
	ldp	x1, x2, [x0, #0x00]	// x1, x2
	ldp	x3, x4, [x0, #0x10]	// x3, x4
	ldp	x5, x6, [x0, #0x20]	// x5, x6
	ldp	x7, x8, [x0, #0x30]	// x7, x8
	ldp	x9, x10, [x0, #0x40]	// x9, x10
	ldp	x11, x12, [x0, #0x50]	// x11, x12
	ldp	x13, x14, [x0, #0x60]	// x13, x14
	ldp	x15, x16, [x0, #0x70]	// x15, x16
	ldp	x17, x18, [x0, #0x80]	// x17, x18
	ldp	x19, x20, [x0, #0x90]	// x19, x20
	ldp	x21, x22, [x0, #0xA0]	// x21, x22
	ldp	x23, x24, [x0, #0xB0]	// x23, x24
	ldp	x25, x26, [x0, #0xC0]	// x25, x26
	ldp	x27, x28, [x0, #0xD0]	// x27, x28
	ldp	x29, x30, [x0, #0xE0]	// x29, x30

	// Restore SP
	ldr	x1, [x0, #0xF0]		// sp
	mov	sp, x1

	ldp	x1, x2, [sp], #16
	ret

.global __vcpu_switch_to_guest
__vcpu_switch_to_guest:
	// x0 = pointer to VcpuContext
	// Save host GPRs (host x0-x30, host sp)
	stp	x1, x2, [sp, #-16]!
	stp	x3, x4, [sp, #-16]!
	stp	x5, x6, [sp, #-16]!
	stp	x7, x8, [sp, #-16]!
	stp	x9, x10, [sp, #-16]!
	stp	x11, x12, [sp, #-16]!
	stp	x13, x14, [sp, #-16]!
	stp	x15, x16, [sp, #-16]!
	stp	x17, x18, [sp, #-16]!
	stp	x19, x20, [sp, #-16]!
	stp	x21, x22, [sp, #-16]!
	stp	x23, x24, [sp, #-16]!
	stp	x25, x26, [sp, #-16]!
	stp	x27, x28, [sp, #-16]!
	stp	x29, x30, [sp, #-16]!
	mov	x1, sp
	str	x1, [x0, #0x00]		// Save host sp to host_sp_offset

	// Load guest GPRs
	ldp	x1, x2, [x0, #0x08]	// guest gprs x1, x2
	ldp	x3, x4, [x0, #0x18]	// guest gprs x3, x4
	ldp	x5, x6, [x0, #0x28]	// guest gprs x5, x6
	ldp	x7, x8, [x0, #0x38]	// guest gprs x7, x8
	ldp	x9, x10, [x0, #0x48]	// guest gprs x9, x10
	ldp	x11, x12, [x0, #0x58]	// guest gprs x11, x12
	ldp	x13, x14, [x0, #0x68]	// guest gprs x13, x14
	ldp	x15, x16, [x0, #0x78]	// guest gprs x15, x16
	ldp	x17, x18, [x0, #0x88]	// guest gprs x17, x18
	ldp	x19, x20, [x0, #0x98]	// guest gprs x19, x20
	ldp	x21, x22, [x0, #0xA8]	// guest gprs x21, x22
	ldp	x23, x24, [x0, #0xB8]	// guest gprs x23, x24
	ldp	x25, x26, [x0, #0xC8]	// guest gprs x25, x26
	ldp	x27, x28, [x0, #0xD8]	// guest gprs x27, x28
	ldp	x29, x30, [x0, #0xE8]	// guest gprs x29, x30

	// Load guest SP (x0 is guest_sp_offset)
	ldr	x1, [x0, #0xF8]		// guest sp
	mov	sp, x1

	// Restore guest system registers
	add	x1, x0, #0x100		// SysRegs offset
	bl	__vcpu_sysregs_restore

	// Restore guest VFP state
	add	x1, x0, #0x210		// VfpRegs offset
	bl	__vcpu_vfp_restore

	// Load guest PC and PSTATE
	ldr	x1, [x0, #0x200]		// guest elr_el1 (pc)
	msr	elr_el1, x1
	ldr	x2, [x0, #0x208]		// guest spsr_el1 (pstate)
	msr	spsr_el1, x2

	// ERET to guest (EL1)
	eret

	// Should not reach here
	b	.

// ============================================================================
// Pointer Authentication Register Save/Restore (ARMv8.3-PAuth)
// ============================================================================

.global __vcpu_ptrauth_save
__vcpu_ptrauth_save:
	// x0 = pointer to PtrauthRegs structure
	// Save APIA key (instruction authentication)
	stp	x1, x2, [sp, #-16]!
	mrs_s	x1, SYS_APIAKEYLO_EL1
	mrs_s	x2, SYS_APIAKEYHI_EL1
	stp	x1, x2, [x0, #0x00]

	// Save APIB key
	mrs_s	x1, SYS_APIBKEYLO_EL1
	mrs_s	x2, SYS_APIBKEYHI_EL1
	stp	x1, x2, [x0, #0x10]

	// Save APDA key (data authentication)
	mrs_s	x1, SYS_APDAKEYLO_EL1
	mrs_s	x2, SYS_APDAKEYHI_EL1
	stp	x1, x2, [x0, #0x20]

	// Save APDB key
	mrs_s	x1, SYS_APDBKEYLO_EL1
	mrs_s	x2, SYS_APDBKEYHI_EL1
	stp	x1, x2, [x0, #0x30]

	// Save APGA key (generic authentication)
	mrs_s	x1, SYS_APGAKEYLO_EL1
	mrs_s	x2, SYS_APGAKEYHI_EL1
	stp	x1, x2, [x0, #0x40]

	ldp	x1, x2, [sp], #16
	ret

.global __vcpu_ptrauth_restore
__vcpu_ptrauth_restore:
	// x0 = pointer to PtrauthRegs structure
	stp	x1, x2, [sp, #-16]!

	// Restore APIA key
	ldp	x1, x2, [x0, #0x00]
	msr_s	SYS_APIAKEYLO_EL1, x1
	msr_s	SYS_APIAKEYHI_EL1, x2

	// Restore APIB key
	ldp	x1, x2, [x0, #0x10]
	msr_s	SYS_APIBKEYLO_EL1, x1
	msr_s	SYS_APIBKEYHI_EL1, x2

	// Restore APDA key
	ldp	x1, x2, [x0, #0x20]
	msr_s	SYS_APDAKEYLO_EL1, x1
	msr_s	SYS_APDAKEYHI_EL1, x2

	// Restore APDB key
	ldp	x1, x2, [x0, #0x30]
	msr_s	SYS_APDBKEYLO_EL1, x1
	msr_s	SYS_APDBKEYHI_EL1, x2

	// Restore APGA key
	ldp	x1, x2, [x0, #0x40]
	msr_s	SYS_APGAKEYLO_EL1, x1
	msr_s	SYS_APGAKEYHI_EL1, x2

	ldp	x1, x2, [sp], #16
	ret

// ============================================================================
// Generic Timer Register Save/Restore
// ============================================================================

.global __vcpu_timer_save
__vcpu_timer_save:
	// x0 = pointer to TimerRegs structure
	stp	x1, x2, [sp, #-16]!

	// Save timer control and value registers
	mrs	x1, cntp_ctl_el0
	str	w1, [x0, #0x00]		// cntpctl
	mrs	x1, cntv_ctl_el0
	str	w1, [x0, #0x04]		// cntvctl
	mrs	x1, cntkctl_el1
	str	w1, [x0, #0x08]		// cntkctl
	mrs	x1, cntp_cval_el0
	str	x1, [x0, #0x10]		// cntpcval
	mrs	x1, cntv_cval_el0
	str	x1, [x0, #0x18]		// cntvcval
	mrs	x1, cntvoff_el2
	str	x1, [x0, #0x20]		// cntvoff

	// Disable timers
	mov	w1, #0x1
	msr	cnpv_ctl_el0, x1		// disable physical timer
	msr	cnv_ctl_el0, x1		// disable virtual timer

	ldp	x1, x2, [sp], #16
	ret

.global __vcpu_timer_restore
__vcpu_timer_restore:
	// x0 = pointer to TimerRegs structure
	stp	x1, x2, [sp, #-16]!

	// Restore virtual timer offset first
	ldr	x1, [x0, #0x20]
	msr	cntvoff_el2, x1		// cntvoff

	// Restore timer compare values
	ldr	x1, [x0, #0x10]
	msr	cntp_cval_el0, x1		// cntpcval
	ldr	x1, [x0, #0x18]
	msr	cnv_cval_el0, x1		// cntvcval

	// Restore timer control registers
	ldr	w1, [x0, #0x08]
	msr	cntkctl_el1, x1		// cntkctl
	ldr	w1, [x0, #0x00]
	msr	cntp_ctl_el0, x1		// cntpctl
	ldr	w1, [x0, #0x04]
	msr	cnv_ctl_el0, x1		// cntvctl

	ldp	x1, x2, [sp], #16
	ret
