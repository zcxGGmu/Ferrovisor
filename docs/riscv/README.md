# RISC-V Virtualization (H-Extension) Overview

## Introduction

The RISC-V Hypervisor Extension (H-extension) enables efficient virtualization of the supervisor-level architecture to support hosting guest operating systems on type-1 or type-2 hypervisors. This extension transforms supervisor mode into **hypervisor-extended supervisor mode (HS-mode)** and adds a second stage of address translation for guest physical addresses.

## Key Concepts

### Virtualization Mode (V)
- **V=0**: Non-virtualized mode (M-mode, HS-mode, or U-mode)
- **V=1**: Virtualized mode (VS-mode or VU-mode)

### Privilege Modes with H-Extension

| Virtualization Mode (V) | Nominal Privilege | Abbreviation | Name | Two-Stage Translation |
|------------------------|------------------|--------------|------|----------------------|
| 0 | 0 | 0 | U | U-mode | Off |
| 0 | 1 | 0 | S | HS-mode | Off |
| 0 | 1 | 1 | M | M-mode | Off |
| 1 | 0 | 0 | U | VU-mode | On |
| 1 | 1 | 0 | S | VS-mode | On |

## Core Features

### 1. Two-Stage Address Translation
- **VS-stage**: Translates guest virtual addresses to guest physical addresses
- **G-stage**: Translates guest physical addresses to supervisor physical addresses
- Both stages are active when V=1

### 2. Virtual Supervisor CSRs
- **HS-mode CSRs**: Control virtualization and guest management
- **VS-mode CSRs**: Replicas of supervisor CSRs for guest execution
- Automatic swapping between HS and VS CSRs based on V mode

### 3. Additional Instructions
- **Virtual Memory Access**: HLV, HLVX, HSV (explicit memory accesses as if V=1)
- **Memory Management Fences**: HFENCE.VVMA, HFENCE.GVMA

### 4. Interrupt Management
- **VS-level interrupts**: Virtual software, timer, and external interrupts
- **Guest external interrupts**: Direct device interrupt delivery to guests
- **Interrupt delegation**: Two-level delegation (M→HS→VS)

## Implementation Requirements

### Dependencies
- Base integer ISA RV32I or RV64I (not RV32E/RV64E)
- Standard page-based address translation (Sv32 for RV32, Sv39+ for RV64)
- `mtval` CSR must not be read-only zero

### Extension Activation
- Set bit 7 in `misa` CSR to enable H-extension
- Letter 'H' corresponds to the H-extension

## Performance Benefits

The H-extension reduces the frequency of VM exits compared to classic virtualization techniques, leading to:
- Reduced hypervisor overhead
- Near-native performance for guest OS execution
- Efficient hardware-assisted virtualization support

## Compatibility

- Regular S-mode operating systems can run without modification in HS-mode or as VS-mode guests
- Extension can be efficiently emulated on platforms without hardware support
- Supports nested virtualization

## Next Steps

For detailed technical specifications, see:
- [CSR Definitions](csrs.md)
- [Address Translation](address-translation.md)
- [Instructions](instructions.md)
- [Exception Handling](exceptions.md)