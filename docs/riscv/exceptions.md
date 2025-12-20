# RISC-V Hypervisor Traps and Exception Handling

This document describes trap handling, exception delegation, and the enhanced exception mechanisms provided by the RISC-V Hypervisor Extension.

## Trap Cause Codes

The H-extension adds new trap cause codes for virtualization-specific events.

### New Exception Codes

| Code | Name | Description |
|------|------|-------------|
| 20 | Instruction guest-page fault | Guest page fault on instruction fetch |
| 21 | Load guest-page fault | Guest page fault on load |
| 22 | Virtual instruction | Illegal instruction in virtualized mode |
| 23 | Store/AMO guest-page fault | Guest page fault on store/AMO |

### New Interrupt Codes

| Code | Name | Description |
|------|------|-------------|
| 2 | Virtual supervisor software interrupt | VS-level software interrupt |
| 6 | Virtual supervisor timer interrupt | VS-level timer interrupt |
| 10 | Virtual supervisor external interrupt | VS-level external interrupt |
| 12 | Supervisor guest external interrupt | HS-level guest external interrupt |

## Exception Delegation

### Two-Level Delegation

```
M-mode
  ├── medeleg/mideleg
  ├── HS-mode
  │   ├── hedeleg/hideleg
  │   └── VS-mode (guest)
  └── Direct M-mode handling
```

### Delegation Rules

1. **M → HS**: Controlled by `medeleg`/`mideleg`
2. **HS → VS**: Controlled by `hedeleg`/`hideleg`
3. **Required Delegation**: VS-level interrupts always delegated past M-mode

### hedeleg - Hypervisor Exception Delegation

| Bit | Exception | Must be Writable | Read-Only When |
|-----|-----------|------------------|----------------|
| 0 | Instruction address misaligned | Yes (if IALIGN=32) | - |
| 1 | Instruction access fault | Yes | - |
| 2 | Illegal instruction | Yes | - |
| 3 | Breakpoint | Yes | - |
| 4 | Load address misaligned | Yes | - |
| 5 | Load access fault | Yes | - |
| 6 | Store/AMO address misaligned | Yes | - |
| 7 | Store/AMO access fault | Yes | - |
| 8 | Environment call from U/VU | Yes | - |
| 9 | Environment call from HS | Read-only | 0 |
| 10 | Environment call from VS | Yes | - |
| 11 | Environment call from M | Read-only | 0 |
| 12 | Instruction page fault | Read-only | 0 |
| 13 | Load page fault | Read-only | 0 |
| 14 | Reserved | Read-only | 0 |
| 15 | Store/AMO page fault | Read-only | 0 |
| 16 | Software check | Yes | - |
| 17 | Hardware error | Yes | - |
| 18 | Instruction guest-page fault | Yes | - |
| 19 | Load guest-page fault | Yes | - |
| 20 | Virtual instruction | Yes | - |
| 21 | Store/AMO guest-page fault | Yes | - |
| 22-23 | Reserved | Read-only | 0 |

## Virtual-Instruction Exceptions

### Definition

A virtual-instruction exception is raised instead of an illegal-instruction exception when:
1. The instruction would be valid in HS-mode (HS-qualified)
2. Execution is prevented due to virtualization (V=1)
3. The restriction is not a fundamental privilege violation

### Causes of Virtual-Instruction Exceptions

#### Counter Access Violations
- VS/VU-mode access to counters when `hcounteren` bit is 0
- Applies to both low-half and high-half counters (XLEN=32)

#### Hypervisor-Related Violations
- Executing hypervisor instructions (HLV, HLVX, HSV, HFENCE) in VS/VU-mode
- Accessing hypervisor CSRs or VS CSRs directly in VS/VU-mode
- Accessing high-half CSRs when corresponding low-half is restricted

#### Supervisor Instruction Violations
- Executing WFI in VU-mode (when `mstatus.TW=0`)
- Executing SRET in VS-mode (when `hstatus.VTSR=1`)
- Executing SFENCE.VMA/SINVAL.VMA in VS-mode (when `hstatus.VTVM=1`)

#### Supervisor CSR Access Violations
- Accessing supervisor CSRs without proper privileges
- Including both regular and high-half CSRs

### Special Cases

Floating-point and vector instructions always raise illegal-instruction exceptions when FS/VS=0, not virtual-instruction exceptions.

## Guest-Page Faults

### Definition

Guest-page faults occur during two-stage address translation when the G-stage translation fails or is denied by permissions.

### Fault Types

1. **Instruction guest-page fault**: During instruction fetch
2. **Load guest-page fault**: During load access
3. **Store/AMO guest-page fault**: During store/AMO access

### Fault Information

When a guest-page fault occurs:

| Register | Content |
|----------|---------|
| `mtval`/`stval`/`vstval` | Faulting guest virtual address |
| `mtval2`/`htval` | Faulting guest physical address (shifted right by 2) |
| `mtinst`/`htinst` | Transformed instruction or pseudoinstruction |

### Guest Physical Address in htval/mtval2

- **Non-zero**: Right-shifted by 2 bits (accommodates addresses > XLEN)
- **Zero**: Fault occurred in first-stage (VS) translation
- **Implicit accesses**: Address of VS page table entry that faulted

### Pseudoinstruction Values

For implicit VS-stage translation failures:

| Value | Meaning |
|-------|---------|
| 0x00002000 | 32-bit read for VS-stage translation (RV32) |
| 0x00002020 | 32-bit write for VS-stage translation (RV32) |
| 0x00003000 | 64-bit read for VS-stage translation (RV64) |
| 0x00003020 | 64-bit write for VS-stage translation (RV64) |

## Trap Entry

### Trap Flow

1. **Occurrence**: Exception or interrupt in some privilege mode
2. **Delegation Check**: Check delegation registers
3. **Mode Transition**: Transition to appropriate handler mode
4. **Context Save**: Save relevant state
5. **Handler Execution**: Execute trap handler code

### Trap Entry by Mode

#### Traps in U-mode/HS-mode (V=0)
- Go to M-mode unless delegated by `medeleg`/`mideleg`
- If delegated, go to HS-mode

#### Traps in VU-mode/VS-mode (V=1)
- Go to M-mode unless delegated by `medeleg`/`mideleg`
- If delegated to HS-mode, check `hedeleg`/`hideleg`
- If further delegated, go to VS-mode

### Saved Context

#### M-mode Trap Entry
- V set to 0
- `mstatus`/`mstatush`: MPV, MPP, MPIE, MIE updated
- `mepc`, `mcause`, `mtval`, `mtval2`, `mtinst` written

#### HS-mode Trap Entry
- V set to 0
- `hstatus`: SPV, SPVP, GVA updated
- `sstatus`: SPP, SPIE, SIE updated
- `sepc`, `scause`, `stval`, `htval`, `htinst` written

#### VS-mode Trap Entry
- V remains 1
- `vsstatus`: SPP updated
- `vsepc`, `vscause`, `vstval` written
- HS-level CSRs unchanged

### Context Save Values

#### hstatus Fields on HS-mode Trap

| Previous Mode | SPV | SPP |
|---------------|-----|-----|
| U-mode | 0 | 0 |
| HS-mode | 0 | 1 |
| VU-mode | 1 | 0 |
| VS-mode | 1 | 1 |

#### vsstatus SPP on VS-mode Trap

| Previous Mode | SPP |
|---------------|-----|
| VU-mode | 0 |
| VS-mode | 1 |

## Trap Return

### MRET

1. **Determine New Mode**: Based on MPP and MPV
2. **Update mstatus**: MPV←0, MPP←0, MIE←MPIE, MPIE←1
3. **Set Privilege**: According to previous mode
4. **Set PC**: mepc

### SRET

#### When V=0
- **Determine New Mode**: Based on SPV and SPP
- **Update hstatus**: SPV←0
- **Update sstatus**: SPP←0, SIE←SPIE, SPIE←1
- **Set Privilege and PC**: According to mode

#### When V=1
- **Update vsstatus**: SPP←0, SIE←SPIE, SPIE←1
- **Set Privilege**: According to SPP
- **Set PC**: vsepc

## Exception Priority

### Synchronous Exception Priority (Highest to Lowest)

1. **Instruction address breakpoint**
2. **Instruction address translation faults** (page fault, guest-page fault, access fault)
3. **Instruction access fault**
4. **Illegal instruction / Virtual instruction**
5. **Environment call / Breakpoint**
6. **Load/store/AMO address breakpoint**
7. **Address misalignment** (optional)
8. **Address translation faults** (page fault, guest-page fault, access fault)
9. **Load/store/AMO access fault**
10. **Address misalignment** (if not higher priority)

## Transformed Instructions

### Purpose

The trap instruction registers (`mtinst`/`htinst`) can contain transformed versions of trapping instructions to aid in emulation.

### Transformation Rules

#### Load/Store Instructions
- Original instruction with rs1 replaced by address offset
- Immediate fields set to zero
- Preserves funct3, rd/rs2, and opcode

#### Atomic Instructions
- Original instruction with bits 19:15 containing address offset
- All other fields preserved

#### Compressed Instructions
1. Expand to 32-bit equivalent
2. Transform 32-bit instruction
3. Set bit 1 to 0

### Value Encoding

- **Bit 0 = 1**: Standard instruction encoding
- **Bit 0 = 0, Bit 1 = 0**: Pseudoinstruction
- **Bit 0 = 1, Bit 1 = 1**: Compressed instruction

## Interaction with Other Extensions

### Ssdbltrp (Double Trap)
- Enables double trap detection
- Uses `vsstatus.SDT` bit
- Affects trap priority

### Zicfilp (Landing Pad)
- Modifies trap handling for forward control flow
- Uses `vsstatus.SPELP`
- Landing pad prediction

### Zicfiss (Shadow Stack)
- Additional exception types
- Modified CSR behavior
- Shadow stack violations