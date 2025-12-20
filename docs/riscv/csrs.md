# RISC-V Hypervisor CSRs (Control and Status Registers)

This document describes all Control and Status Registers (CSRs) introduced or modified by the RISC-V Hypervisor Extension.

## Hypervisor-Level CSRs (HS-mode)

### Status and Configuration CSRs

#### hstatus - Hypervisor Status Register
Format (HSXLEN-bit read/write):
```
31..... 28 27 26 25 24 23.....20 19 18 17 16 15 14 13 12 11..10 9..8 7 6 5 4 3 2 1 0 (HSXLEN=64)
        |   |  |  |  |      |  |  |  |  |  |  |      |   |  | | | | | | |
        |   |  |  |  |      |  |  |  |  |  |  |      |   |  | | | | | | +- VSXL[1:0] (64-bit only)
        |   |  |  |  |      |  |  |  |  |  |  |      |   |  | | | | | +--- HU (Hypervisor in U-mode)
        |   |  |  |  |      |  |  |  |  |  |  |      |   |  | | | | +----- SPV (Supervisor Previous Virtualization)
        |   |  |  |  |      |  |  |  |  |  |  |      |   |  | | | +------- SPVP (Supervisor Previous Virtual Privilege)
        |   |  |  |  |      |  |  |  |  |  |  |      |   |  | | +--------- GVA (Guest Virtual Address)
        |   |  |  |  |      |  |  |  |  |  |  |      |   |  | +----------- VSBE (Virtual Supervisor Byte Endian)
        |   |  |  |  |      |  |  |  |  |  |  |      |   |  +--------------- VTSR (Virtual SRET)
        |   |  |  |  |      |  |  |  |  |  |  |      |   +------------------ VTW (Virtual WFI)
        |   |  |  |  |      |  |  |  |  |  |  |      +---------------------- VTVM (Virtual TVM)
        |   |  |  |  |      |  |  |  |  |  |  +----------------------------- VGEIN[5:0] (Virtual Guest External Int Num)
```

Key fields:
- **VSXL**: Controls effective XLEN for VS-mode (64-bit only)
- **HU**: Enable hypervisor instructions in U-mode
- **SPV**: Previous virtualization mode on trap entry
- **SPVP**: Previous virtual privilege level on trap entry
- **GVA**: Whether trap wrote guest virtual address
- **VSBE**: Byte endianness for VS-mode memory accesses
- **VTSR/VTW/VTVM**: Virtual versions of TSR/TW/TVM for VS-mode
- **VGEIN**: Selects guest external interrupt source

#### hedeleg - Hypervisor Exception Delegation Register
Controls delegation of synchronous exceptions from HS-mode to VS-mode.

#### hideleg - Hypervisor Interrupt Delegation Register
Controls delegation of interrupts from HS-mode to VS-mode.

### Interrupt CSRs

#### hvip - Hypervisor Virtual Interrupt Pending
Indicates virtual interrupts intended for VS-mode.

#### hip - Hypervisor Interrupt Pending
Supplements HS-level `sip` with VS-level and hypervisor-specific interrupts.

#### hie - Hypervisor Interrupt Enable
Contains enable bits for VS-level and hypervisor-specific interrupts.

### Guest External Interrupt CSRs

#### hgeip - Hypervisor Guest External Interrupt Pending
Read-only register indicating pending guest external interrupts.

#### hgeie - Hypervisor Guest External Interrupt Enable
Contains enable bits for guest external interrupts.

### Memory Management CSRs

#### hgatp - Hypervisor Guest Address Translation and Protection
Controls G-stage address translation (guest physical to supervisor physical).

Format for HSXLEN=64:
```
63.....59 58.....57 56.....44 43.....28 27.....0
|     |     |      |       |        |
|     |     |      |       |        +- PPN[43:0] (Page Table Base)
|     |     |      |       +---------- VMID[13:0] (Virtual Machine ID)
|     |     |      +----------------- MODE[3:0] (Translation Scheme)
|     |     +------------------------ Reserved
|     +------------------------------ Reserved
```

MODE field encoding:
- 0: Bare (no translation)
- 8: Sv39x4 (39-bit with 2-bit extension)
- 9: Sv48x4 (48-bit with 2-bit extension)
- 10: Sv57x4 (57-bit with 2-bit extension)

### Environment Configuration CSRs

#### henvcfg - Hypervisor Environment Configuration
Controls execution environment characteristics when V=1.

Key fields:
- **FIOM**: Fence of I/O implies Memory
- **PBMTE**: Svpbmt extension enable for VS-stage
- **ADUE**: Automatic A/D bit updates for VS-stage
- **LPE**: Zicfilp extension enable for VS-mode
- **SSE**: Zicfiss extension enable for VS-mode

### Counter CSRs

#### hcounteren - Hypervisor Counter Enable
Controls availability of hardware performance counters to guest VMs.

#### htimedelta - Hypervisor Time Delta
64-bit delta between actual `time` CSR and value returned in VS/VU modes.

### Trap-Related CSRs

#### htval - Hypervisor Trap Value
Additional exception information (guest physical address on guest-page fault).

#### htinst - Hypervisor Trap Instruction
Information about the instruction that caused the trap.

## Virtual Supervisor CSRs (VS-mode)

These CSRs are active only when V=1 and substitute for normal supervisor CSRs:

| VS CSR | Normal CSR | Description |
|--------|-------------|-------------|
| vsstatus | sstatus | Virtual supervisor status |
| vsie | sie | Virtual interrupt enable |
| vsip | sip | Virtual interrupt pending |
| vstvec | stvec | Virtual trap vector base |
| vsscratch | sscratch | Virtual scratch register |
| vsepc | sepc | Virtual exception PC |
| vscause | scause | Virtual exception cause |
| vstval | stval | Virtual trap value |
| vsatp | satp | Virtual address translation |

## Machine-Level CSRs Modified by H-Extension

### mstatus/mstatush
Additional fields:
- **MPV**: Machine Previous Virtualization mode
- **GVA**: Guest Virtual Address flag

### mideleg
Bits 10, 6, and 2 (VS-level interrupts) are read-only one.

### mip/mie
Additional active bits for VS-level interrupts.

### mtval2 - Machine Second Trap Value
Additional exception information for M-mode traps.

### mtinst - Machine Trap Instruction
Information about trapping instruction for M-mode.

## CSR Access Rules

### When V=1:
- Supervisor CSRs access VS CSRs
- Direct VS CSR access causes virtual-instruction exception
- HS CSRs retain values but don't affect execution

### When V=0:
- VS CSRs are readable/writable but don't affect execution
- Normal supervisor CSRs function normally

### Privilege Restrictions:
- Some CSRs accessible only from M-mode/HS-mode
- Others accessible from all modes based on delegation