# RISC-V Two-Stage Address Translation

The RISC-V Hypervisor Extension implements a two-stage address translation mechanism when virtualization mode (V) is enabled (V=1). This allows guest operating systems to use virtual memory while the hypervisor maintains isolation between guests.

## Overview

When V=1, memory accesses undergo two-stage translation:

1. **VS-stage (First Stage)**: Guest virtual addresses (GVA) → Guest physical addresses (GPA)
   - Controlled by `vsatp` CSR
   - Uses standard page-based translation (Sv32, Sv39, Sv48, Sv57)

2. **G-stage (Second Stage)**: Guest physical addresses (GPA) → Supervisor physical addresses (SPA)
   - Controlled by `hgatp` CSR
   - Uses extended page formats (Sv32x4, Sv39x4, Sv48x4, Sv57x4)

## VS-Stage Translation (Virtual Supervisor)

Controlled by the `vsatp` register with standard RISC-V virtual memory schemes:

### Supported Schemes
- **Sv32**: 32-bit virtual addressing, 34-bit physical addressing
- **Sv39**: 39-bit virtual addressing, 56-bit physical addressing
- **Sv48**: 48-bit virtual addressing, 56-bit physical addressing
- **Sv57**: 57-bit virtual addressing, 56-bit physical addressing

### VS-Stage Page Table Entries
Standard RISC-V page table entry format:
```
63.....10 9.....8 7.....6 5.....4 3.....2 1.....0
|       |   |   |   |   |   |   |   |   |
|       +---+---+---+---+---+---+---+---+--- PPN
|           |   |   |   |   |   |   +----- X (Execute)
|           |   |   |   |   |   +--------- W (Write)
|           |   |   |   |   +------------- R (Read)
|           |   |   |   +------------------- U (User)
|           |   |   +----------------------- G (Global)
|           |   +--------------------------- A (Accessed)
|           +------------------------------- D (Dirty)
+------------------------------------------- V (Valid)
```

## G-Stage Translation (Guest Physical)

Controlled by the `hgatp` register with extended page formats:

### Supported Schemes
- **Bare**: No translation (GPA = SPA)
- **Sv32x4**: 34-bit guest physical addresses
- **Sv39x4**: 41-bit guest physical addresses
- **Sv48x4**: 50-bit guest physical addresses
- **Sv57x4**: 59-bit guest physical addresses

### Extended Address Formats

#### Sv32x4 (34-bit GPA)
```
33.....22 21.....12 11.....2 1.....0
|         |         |        |
| VPN[1]  | VPN[0]  | offset |
```

#### Sv39x4 (41-bit GPA)
```
40.....30 29.....21 20.....12 11.....2 1.....0
|         |         |         |        |
| VPN[2]  | VPN[1]  | VPN[0]  | offset |
```

#### Sv48x4 (50-bit GPA)
```
49.....39 38.....30 29.....21 20.....12 11.....2 1.....0
|         |         |         |         |        |
| VPN[3]  | VPN[2]  | VPN[1]  | VPN[0]  | offset |
```

#### Sv57x4 (59-bit GPA)
```
58.....48 47.....39 38.....30 29.....21 20.....12 11.....2 1.....0
|         |         |         |         |         |        |
| VPN[4]  | VPN[3]  | VPN[2]  | VPN[1]  | VPN[0]  | offset |
```

### Key Differences from Standard Translation

1. **Root Page Table Size**: 16 KiB instead of 4 KiB (4x larger)
2. **Alignment**: Root page tables must be 16 KiB aligned
3. **Address Extension**: Adds 2 bits to physical address width
4. **VMID**: Virtual Machine Identifier for isolation

### G-Stage PTE Format
Uses the same format as standard RISC-V page table entries with one exception:
- **G-bit**: Reserved for future standard use (should be cleared by software)

## Translation Process

### Step-by-Step Translation

1. **Determine Effective Mode**
   - Check current virtualization mode (V)
   - Check effective privilege level
   - Verify `hgatp` and `vsatp` are active

2. **VS-Stage Translation** (if active)
   - Use `vsatp.MODE` to select translation scheme
   - Walk guest page tables
   - Apply VS-level permissions
   - Handle A/D bit updates (if enabled)
   - Result: Guest Physical Address (GPA)

3. **G-Stage Translation** (if V=1)
   - Use `hgatp.MODE` to select translation scheme
   - Walk hypervisor page tables (16 KiB root)
   - Apply G-stage permissions
   - Check VMID match
   - Result: Supervisor Physical Address (SPA)

4. **Physical Memory Protection**
   - Apply PMP checks to final SPA
   - Enforce memory attributes

### Special Cases

#### Memory Accesses that Bypass Translation
- Physical memory accesses bypass VS-stage
- Still undergo G-stage translation when V=1
- Includes VS-stage page table walks

#### Instruction Fetches
- Can have different permissions than loads/stores
- Execute-only pages handled by MXR bits

## Virtual Machine Identifiers (VMID)

### VMID Field in hgatp
- **VMIDLEN**: Implementation-specific width (0-14 bits)
- **VMIDMAX**: Maximum supported VMID width
  - 7 for Sv32x4
  - 14 for Sv39x4, Sv48x4, Sv57x4

### VMID Usage
- Identifies different virtual machines
- Used for address-translation cache isolation
- Required for HFENCE.GVMA synchronization

## Memory Management Fences

### SFENCE.VMA
- When V=0: Operates on HS-level translations
- When V=1: Operates on VS-level translations within current VMID

### HFENCE.VVMA (Hypervisor)
- Invalidates VS-stage translations
- Optional specification of VA and ASID
- Affects only current VMID

### HFENCE.GVMA (Hypervisor)
- Invalidates G-stage translations
- Optional specification of GPA and VMID
- Critical when changing `hgatp` MODE

## Permission Checking

### Access Permissions
- **Read (R)**: Load permissions
- **Write (W)**: Store permissions
- **Execute (X)**: Instruction fetch permissions
- **User (U)**: User-mode access

### MXR (Make eXecutable Readable)
- **vsstatus.MXR**: Affects VS-stage only
- **sstatus.MXR**: Affects both stages
- Allows reading execute-only pages

### Page Table Update Rules
- A/D bits set only on actual memory accesses
- Not set on speculative execution
- Controlled by Svadu extension enable

## Performance Considerations

### Translation Caches
- TLB entries can cache both stages
- VMID used for cache isolation
- Selective invalidation possible

### Address Calculation Overhead
- Two-stage translation adds latency
- Hardware acceleration critical
- Root page table size impacts cache usage

## Interaction with Other Extensions

### Svpbmt Extension
- Controlled by `henvcfg.PBMTE` for G-stage
- Memory type attributes in PTEs

### Svadu Extension
- Controlled by `henvcfg.ADUE` for VS-stage
- Automatic A/D bit management

### Zicfilp Extension
- Controlled by `henvcfg.LPE` for VS-mode
- Landing pad prediction

### Zicfiss Extension
- Controlled by `henvcfg.SSE` for VS-mode
- Shadow stack support