# RISC-V Hypervisor Instructions

The RISC-V Hypervisor Extension introduces several new instructions and modifies the behavior of existing ones to support efficient virtualization.

## Virtual Machine Access Instructions

These instructions allow explicit memory accesses as if executing in virtualized mode (V=1). They are valid only in M-mode, HS-mode, or U-mode when `hstatus.HU=1`.

### Load Instructions

| Instruction | Description | Data Width | Access Type |
|-------------|-------------|------------|-------------|
| HLV.B | Hypervisor Load Byte | 8 bits | Signed |
| HLV.BU | Hypervisor Load Byte Unsigned | 8 bits | Unsigned |
| HLV.H | Hypervisor Load Halfword | 16 bits | Signed |
| HLV.HU | Hypervisor Load Halfword Unsigned | 16 bits | Unsigned |
| HLV.W | Hypervisor Load Word | 32 bits | Signed |
| HLV.WU | Hypervisor Load Word Unsigned | 32 bits | Unsigned |
| HLV.D | Hypervisor Load Doubleword | 64 bits | Signed |

### Store Instructions

| Instruction | Description | Data Width |
|-------------|-------------|------------|
| HSV.B | Hypervisor Store Byte | 8 bits |
| HSV.H | Hypervisor Store Halfword | 16 bits |
| HSV.W | Hypervisor Store Word | 32 bits |
| HSV.D | Hypervisor Store Doubleword | 64 bits |

### Execute-Only Access Instructions

These instructions require execute permission but not read permission:

| Instruction | Description | Note |
|-------------|-------------|------|
| HLVX.HU | Hypervisor Load Halfword Unsigned Execute-Only | Cannot override PMP execute-only |
| HLVX.WU | Hypervisor Load Word Unsigned Execute-Only | Valid even for RV32 |

### Instruction Behavior

1. **Address Translation**: Always uses two-stage translation (V=1)
2. **Privilege Level**: Controlled by `hstatus.SPVP`
   - SPVP=0: Access as VU-mode
   - SPVP=1: Access as VS-mode
3. **Memory Attributes**: Uses `hstatus.VSBE` for endianness
4. **MXR Handling**: HS-level MXR affects both stages, VS-level MXR only VS-stage

## Memory Management Fence Instructions

These privileged fence instructions manage translation cache invalidation.

### HFENCE.VVMA - Hypervisor Fence for VS Virtual Memory

```assembly
HFENCE.VVMA rs1, rs2
```

**Purpose**: Order stores to VS-level page tables before subsequent VS-stage translations.

**Operands**:
- `rs1`: Guest virtual address register (x0 = all addresses)
- `rs2`: Guest ASID register (x0 = all ASIDs)

**Valid in**: M-mode or HS-mode only

**Effects**:
- Invalidates VS-stage TLB entries matching:
  - Specified guest virtual address (if rs1 ≠ x0)
  - Specified ASID (if rs2 ≠ x0)
  - Current VMID from `hgatp.VMID`

### HFENCE.GVMA - Hypervisor Fence for Guest Physical Memory

```assembly
HFENCE.GVMA rs1, rs2
```

**Purpose**: Order stores to G-stage page tables before subsequent G-stage translations.

**Operands**:
- `rs1`: Guest physical address register, shifted right by 2 bits (x0 = all addresses)
- `rs2`: VMID register (x0 = all VMIDs)

**Valid in**: HS-mode (when `mstatus.TVM=0`) or M-mode

**Effects**:
- Invalidates G-stage TLB entries matching:
  - Specified guest physical address (if rs1 ≠ x0)
  - Specified VMID (if rs2 ≠ x0)

### Important Notes

1. **No TVM/VTVM restrictions**: These fences don't trap when `mstatus.TVM=1` or `hstatus.VTVM=1`
2. **VMID Handling**: Changes to `hgatp` may require HFENCE.GVMA
3. **Address Encoding**: Guest physical addresses in HFENCE.GVMA are right-shifted by 2 bits

## Modified Existing Instructions

### SRET - Supervisor Return

Modified behavior based on current virtualization mode:

#### When V=0 (HS-mode or M-mode)
- Uses `hstatus.SPV` and `sstatus.SPP`
- Sets new privilege based on SPP
- Sets V based on SPV

#### When V=1 (VS-mode)
- Uses `vsstatus.SPP` only
- Sets new privilege based on SPP
- V remains 1

### SFENCE.VMA - Supervisor Fence

Modified behavior based on virtualization mode:

#### When V=0
- Operates on HS-level address translations
- Uses HS-level virtual addresses and ASIDs

#### When V=1
- Operates on VS-level address translations
- Uses guest virtual addresses and ASIDs
- Affected by current VMID

### WFI - Wait For Interrupt

Modified trapping conditions:
- In VS-mode with `hstatus.VTW=1`: Traps as virtual-instruction
- In VU-mode with `mstatus.TW=0`: Traps as virtual-instruction
- Normal WFI behavior otherwise

### CSR Access Instructions

#### Access Rules When V=1
- Supervisor CSRs access corresponding VS CSRs
- Direct VS CSR access causes virtual-instruction exception
- Some supervisor CSRs have no VS counterpart (remain normal)

#### High-Half CSR Handling (XLEN=32)
- Invalid high-half access can cause virtual-instruction exception
- Determined by corresponding low-half CSR accessibility

## Exception Conditions

### Virtual-Instruction Exception

Raised instead of illegal-instruction for:

1. **HS-qualified instructions** prevented when V=1:
   - Counter accesses when `hcounteren` bits clear
   - Hypervisor CSRs access
   - VS CSRs direct access
   - WFI in VS-mode with VTW=1
   - SRET in VS-mode with VTSR=1
   - SFENCE.VMA/SINVAL.VMA in VS-mode with VTVM=1

2. **Execute in VS/VU-mode when V=1**:
   - Hypervisor instructions (HLV, HLVX, HSV, HFENCE)
   - Supervisor instructions (SRET, SFENCE)
   - Certain supervisor CSR accesses

### Illegal-Instruction Exception

Raised for:

1. Instructions not HS-qualified
2. Floating-point/vector instructions when FS/VS = 0
3. Accesses to non-implemented CSRs
4. Privilege violations not covered by virtual-instruction rules

## Instruction Encoding Details

### Virtual Memory Access Instructions

All HLV/HLVX/HSV instructions follow this encoding pattern:

```
31.....26 25.....20 19.....15 14.....12 11.....7 6.....0
| opcode | funct3 | rs1 | funct2 | rs1' | rd | opcode
```

### Memory Fence Instructions

HFENCE instructions use the system instruction opcode space with custom funct3/funct7 values.

## Performance Considerations

### Execution Modes
- **Native**: No V=1, standard supervisor execution
- **Emulated**: V=1, guest execution with two-stage translation
- **Hypervisor Access**: HLV/HSV instructions for VM state

### Cache Coherency
- HFENCE instructions critical for TLB coherence
- VMID changes require G-stage fence
- Address space changes require VS-stage fence

### Best Practices
1. Use HFENCE.VVMA after VS page table updates
2. Use HFENCE.GVMA after G-stage page table updates
3. Consider using broad fences (rs1=x0, rs2=x0) for simplicity
4. Account for VMID in multi-VM scenarios