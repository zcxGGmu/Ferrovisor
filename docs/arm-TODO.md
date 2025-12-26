# Ferrovisor ARM 架构支持计划

## 文档信息

| 项目 | 说明 |
|------|------|
| **创建日期** | 2025-12-27 |
| **更新日期** | 2025-12-27 |
| **版本** | v2.2 (MMU Stage-2 已完成) |
| **状态** | 实施阶段 2 |
| **参考项目** | Xvisor (/home/zcxggmu/workspace/hello-projs/posp/xvisor) |

## 进度追踪

### 已完成 ✅

#### 阶段 2.1: MMU Stage-2 页表管理 (2025-12-27)
- [x] `arch/arm64/mmu/stage2.rs` - Stage-2 页表结构 (430 行)
  - PTE bit 定义 (VALID, TABLE, AF, SH, HAP, MEMATTR, XN)
  - Block/Page descriptor 创建
  - PageTable 结构 (512 entries, 4KB aligned)
  - PageTableLevel 枚举 (L0-L3) 和辅助方法
  - level_index() 函数
- [x] `arch/arm64/mmu/operations.rs` - 页表操作 (455 行)
  - MapFlags 结构体 (cacheable, bufferable, writable, executable, device)
  - map_range() - IPA -> PA 映射
  - unmap_range() - 取消映射
  - walk_page_table() - 页表遍历
  - TLB 操作: tlb_flush_ipa(), tlb_flush_all()
  - pte_sync() - 页表项同步
- [x] `arch/arm64/mmu/vttbr.rs` - VTTBR_EL2 管理 (214 行)
  - VmidAllocator (AtomicU16 + AtomicU64 bitmap)
  - allocate_vmid() - VMID 分配 (fast/slow path)
  - free_vmid() - VMID 释放
  - is_vmid_allocated() - 检查 VMID 状态
  - make_vttbr() - 创建 VTTBR_EL2 值
  - read_vttbr_el2()/write_vttbr_el2() - 寄存器访问

**代码统计:**
- 新增/修改文件: 3 个
- 总代码量: ~1,100 行

**Commit:** (待提交)

---

#### 阶段 0.1: ARM64 CPU 抽象层接口和目录结构 (2025-12-27)
- [x] 创建 `arch/arm64/` 目录结构
- [x] `arch/arm64/mod.rs` - ARM64 架构主模块
  - ExceptionLevel 枚举 (EL0-EL3)
  - PStateFlags bitflags
  - SystemRegEncoding 结构
  - EL2 系统寄存器编码 (HCR_EL2, VTTBR_EL2, VTCR_EL2 等)
- [x] `arch/arm64/cpu/` - CPU 管理模块
  - `cpu/mod.rs` - CPU 管理函数
  - `cpu/regs.rs` - 系统寄存器访问 (EL2, EL1, info)
  - `cpu/features.rs` - CPU 特性检测 (CpuInfo, CpuFeatures)
  - `cpu/state.rs` - VCPU 上下文结构
  - `cpu/init.rs` - EL2 模式初始化
- [x] `arch/arm64/mmu/` - Stage-2 MMU 模块
  - `mmu/mod.rs`
  - `mmu/stage2.rs` - 页表结构
  - `mmu/vttbr.rs` - VMID 分配
  - `mmu/vtcr.rs` - VTCR 配置
  - `mmu/attrs.rs` - 内存属性
- [x] `arch/arm64/interrupt/` - 中断处理模块
  - `interrupt/mod.rs`
  - `interrupt/gic.rs` - GIC 框架
  - `interrupt/vgic.rs` - VGIC 状态
  - `interrupt/virq.rs` - 虚拟中断
- [x] `arch/arm64/smp/` - 多处理器支持
  - `smp/mod.rs`
  - `smp/psci.rs` - PSCI 接口
  - `smp/spin_table.rs` - Spin Table 方法
- [x] `arch/arm64/platform/` - 平台支持
  - `platform/mod.rs`
  - `platform/qemu_virt.rs` - QEMU virt 平台
  - `platform/foundation_v8.rs` - ARM Foundation v8 模型

**代码统计:**
- 新增文件: 21 个
- 总代码量: ~3,000 行

**Commit:** c8ecd3a

---

## 目录

- [一、项目背景](#一项目背景)
- [二、Xvisor ARM 架构深度分析](#二xvisor-arm-架构深度分析)
- [三、ARM 架构支持差距分析](#三arm-架构支持差距分析)
- [四、ARM 支持实施计划](#四arm-支持实施计划)
- [五、ARM 目录结构设计](#五arm-目录结构设计)
- [六、Xvisor 关键文件详细映射](#六xvisor-关键文件详细映射)
- [七、风险评估](#七风险评估)
- [八、参考资料](#八参考资料)
- [九、里程碑](#九里程碑)

---

## 一、项目背景

### 1.1 当前状态

Ferrovisor 是一个基于 Rust 实现的 Type-1 裸机虚拟机监视器，目前在 **RISC-V 64-bit** 架构上实现了完整的虚拟化功能。对于 ARM 架构的支持，当前仅有框架代码，无具体实现。

**当前架构支持状态：**

| 架构 | 状态 | 代码量 | 完成度 |
|------|------|--------|--------|
| RISC-V 64-bit | ✅ 完整实现 | ~12,746 行 | 100% |
| ARM64 | 🚧 框架代码 | ~100 行 | < 5% |
| ARMv7 (32-bit) | ❌ 未实现 | 0 行 | 0% |
| x86_64 | 🚧 框架代码 | ~100 行 | < 5% |

### 1.2 Xvisor 项目 ARM 支持总览

Xvisor 是一个成熟的 ARM 虚拟化项目，其 ARM 支持代码规模如下：

| 组件 | 文件数 | 代码量 (行) | 状态 |
|------|--------|-------------|------|
| ARM64 CPU | 35 | ~4,422 | ✅ |
| ARMv7 CPU (arm32ve) | 35 | ~4,780 | ✅ |
| ARM Common | 17 | ~15,000 | ✅ |
| 板级支持 | 4+ | ~2,000 | ✅ |
| 设备树 | 50+ | ~5,000 | ✅ |
| **总计** | **140+** | **~31,000** | ✅ |

---

## 二、Xvisor ARM 架构深度分析

### 2.1 Xvisor ARM 目录结构

```
xvisor/arch/arm/
├── configs/                    # ARM 配置文件
├── dts/                        # 设备树源文件
│   ├── arm/                    # ARM 通用设备树
│   ├── broadcom/               # Broadcom (Raspberry Pi)
│   ├── rockchip/               # Rockchip (RK3399)
│   ├── allwinner/              # Allwinner
│   ├── marvell/                # Marvell
│   ├── renesas/                # Renesas
│   └── xilinx/                 # Xilinx
├── board/                      # 板级支持
│   ├── common/                 # 通用板级代码
│   │   ├── smp_ops.c           # SMP 操作框架
│   │   ├── versatile/          # Versatile 平台支持
│   │   └── include/
│   └── generic/                # 通用开发板
│       ├── bcm2836.c           # Raspberry Pi 2
│       ├── bcm2837.c           # Raspberry Pi 3
│       ├── foundation-v8.c     # ARMv8 基金会模型
│       ├── vexpress.c          # VExpress
│       └── rk3399.c            # Rockchip RK3399
├── cpu/                        # CPU 实现
│   ├── arm64/                  # ARMv8-A 64位
│   │   ├── cpu_init.c          # CPU 初始化 (112 行)
│   │   ├── cpu_entry.S         # 入口和异常向量
│   │   ├── cpu_vcpu_helper.c   # VCPU 辅助函数 (899 行)
│   │   ├── cpu_vcpu_switch.S   # VCPU 上下文切换
│   │   ├── cpu_vcpu_excep.c    # 异常处理 (187 行)
│   │   ├── cpu_vcpu_emulate.c  # 指令仿真 (613 行)
│   │   ├── cpu_vcpu_inject.c   # 中断注入 (291 行)
│   │   ├── cpu_vcpu_irq.c      # IRQ 处理 (217 行)
│   │   ├── cpu_vcpu_sysregs.c  # 系统寄存器 (464 行)
│   │   ├── cpu_vcpu_vfp.c      # VFP/NEON (156 行)
│   │   ├── cpu_vcpu_coproc.c   # 协处理器 (288 行)
│   │   ├── cpu_vcpu_mem.c      # 内存访问 (173 行)
│   │   ├── cpu_vcpu_ptrauth.c  # 指针认证 (110 行)
│   │   ├── cpu_interrupts.c    # 中断控制 (246 行)
│   │   ├── cpu_cache.S         # 缓存操作
│   │   ├── cpu_atomic.c        # 原子操作 (140 行)
│   │   ├── cpu_atomic64.c      # 64位原子操作 (141 行)
│   │   ├── cpu_locks.c         # 锁实现 (194 行)
│   │   ├── cpu_memcpy.S        # 内存复制
│   │   ├── cpu_memset.S        # 内存设置
│   │   ├── cpu_delay.S         # 延迟函数
│   │   ├── cpu_proc.S          # 处理器特定函数
│   │   ├── cpu_stacktrace.c    # 堆栈跟踪 (125 行)
│   │   └── cpu_elf.c           # ELF 处理 (66 行)
│   ├── arm32ve/                # ARMv7 VE (Virtualization Extensions)
│   │   ├── cpu_init.c          # CPU 初始化 (113 行)
│   │   ├── cpu_vcpu_helper.c   # VCPU 辅助函数 (1094 行)
│   │   ├── cpu_vcpu_switch.S   # VCPU 上下文切换
│   │   ├── cpu_vcpu_excep.c    # 异常处理 (184 行)
│   │   ├── cpu_vcpu_emulate.c  # 指令仿真 (564 行)
│   │   ├── cpu_vcpu_cp15.c     # CP15 协处理器 (653 行)
│   │   ├── cpu_vcpu_cp14.c     # CP14 调试协处理器 (218 行)
│   │   ├── cpu_vcpu_vfp.c      # VFP 仿真 (193 行)
│   │   ├── cpu_vcpu_coproc.c   # 协处理器框架 (320 行)
│   │   ├── cpu_interrupts.c    # 中断控制 (268 行)
│   │   └── ... (其余与 arm64 类似)
│   └── common/                 # ARMv7/ARMv8 通用代码
│       ├── mmu_lpae.c          # LPAE MMU (397 行)
│       ├── mmu_lpae_entry_ttbl.c
│       ├── vgic.c              # VGIC 通用实现 (~40KB, 1000+ 行)
│       ├── vgic_v2.c           # GICv2 特定代码 (7.7KB)
│       ├── vgic_v3.c           # GICv3 特定代码 (11.7KB)
│       ├── emulate_arm.c       # ARM 指令仿真 (~105KB, 2700+ 行)
│       ├── emulate_thumb.c     # Thumb 指令仿真
│       ├── emulate_psci.c      # PSCI 仿真 (8.7KB)
│       ├── arm_psci.c          # PSCI 框架 (7.4KB)
│       ├── generic_timer.c     # 通用定时器 (16.7KB)
│       ├── smp_ops.c           # SMP 框架 (9.7KB)
│       ├── smp_psci.c          # PSCI SMP 启动
│       ├── smp_spin_table.c    # Spin Table SMP 启动
│       ├── smp_scu.c           # SCU SMP 启动 (5.2KB)
│       ├── smp_imx.c           # i.MX SMP 启动 (5.6KB)
│       ├── arm_locks.c         # ARM 锁实现 (4.6KB)
│       └── include/
│           ├── arm_features.h  # ARM 特性定义
│           ├── psci.h          # PSCI 接口定义
│           └── mmu_lpae.h      # LPAE MMU 定义
└── include/                    # ARM 头文件
    └── arm_features.h          # CPU 特性枚举
```

### 2.2 ARMv7 (ARM32ve) 特有功能

**关键数据结构 - arch_regs (arm32ve):**

```c
struct arch_regs {
    u32 gpr[CPU_GPR_COUNT];    // R0-R12, R14 (LR)
    u32 r13_usr;               // User SP
    u32 r13_irq;               // IRQ SP
    u32 r13_svc;               // Supervisor SP
    u32 r13_abt;               // Abort SP
    u32 r13_und;               // Undefined SP
    u32 r13_hyp;               // Hypervisor SP
    u32 spsr;                  // Saved PSR
    u32 pc;                    // Program Counter
};
```

**ARM 特性标志位 (arm_features.h):**

```c
enum arm_features {
    ARM_FEATURE_VFP,           // VFP 支持
    ARM_FEATURE_VFP3,          // VFPv3
    ARM_FEATURE_VFP4,          // VFPv4
    ARM_FEATURE_VFP_FP16,      // FP16 支持
    ARM_FEATURE_NEON,          // NEON SIMD
    ARM_FEATURE_THUMB2,        // Thumb-2 指令集
    ARM_FEATURE_THUMB_DIV,     // Thumb 除法指令
    ARM_FEATURE_ARM_DIV,       // ARM 除法指令
    ARM_FEATURE_MPU,           // MPU (非 MMU)
    ARM_FEATURE_V6,            // ARMv6
    ARM_FEATURE_V6K,           // ARMv6K
    ARM_FEATURE_V7,            // ARMv7
    ARM_FEATURE_V7MP,          // v7 多处理扩展
    ARM_FEATURE_V8,            // ARMv8
    ARM_FEATURE_LPAE,          // 大物理地址扩展
    ARM_FEATURE_TRUSTZONE,     // TrustZone
    ARM_FEATURE_GENERIC_TIMER, // 通用定时器
    ARM_FEATURE_MVFR,          // Media/VFP 特性寄存器
    ARM_FEATURE_AUXCR,         // 辅助控制寄存器
    ARM_FEATURE_XSCALE,        // Intel XScale
    ARM_FEATURE_IWMMXT,        // Intel 无线 MMX
    ARM_FEATURE_OMAPCP,        // OMAP CP15 特殊处理
    ARM_FEATURE_THUMB2EE,      // Thumb-2 执行环境
    ARM_FEATURE_PTRAUTH,       // 指针认证
};
```

**支持 CPU ID (arm_features.h):**

```c
#define ARM_CPUID_ARM1026      0x4106a262
#define ARM_CPUID_ARM926       0x41069265
#define ARM_CPUID_ARM1136      0x4117b363
#define ARM_CPUID_ARM11MPCORE  0x410fb022
#define ARM_CPUID_CORTEXA7     0x410fc070
#define ARM_CPUID_CORTEXA8     0x410fc080
#define ARM_CPUID_CORTEXA9     0x410fc090
#define ARM_CPUID_CORTEXA15    0x412fc0f1
#define ARM_CPUID_ARMV7        0x000f0000
#define ARM_CPUID_ARMV8        0x000f0001
```

### 2.3 ARMv8 (ARM64) 特有功能

**关键数据结构 - arch_regs (arm64):**

```c
struct arch_regs {
    u64 gpr[CPU_GPR_COUNT];    // X0-X29
    u64 lr;                    // X30 (Link Register)
    u64 sp;                    // Stack Pointer
    u64 pc;                    // Program Counter
    u64 pstate;                // PState
};

struct arm_priv_sysregs {
    // EL1/EL0 系统寄存器
    u64 sp_el0;                // 0x00
    u64 sp_el1;                // 0x08
    u64 elr_el1;               // 0x10 - Exception Link Register
    u64 spsr_el1;              // 0x18 - Saved PSR
    u64 midr_el1;              // 0x20 - Processor ID
    u64 mpidr_el1;             // 0x28 - Multiprocessor ID
    u64 sctlr_el1;             // 0x30 - System Control
    u64 actlr_el1;             // 0x38 - Auxiliary Control
    u64 cpacr_el1;             // 0x40 - Coprocessor Access
    u64 ttbr0_el1;             // 0x48 - Translation Table Base 0
    u64 ttbr1_el1;             // 0x50 - Translation Table Base 1
    u64 tcr_el1;               // 0x58 - Translation Control
    u64 esr_el1;               // 0x60 - Exception Syndrome
    u64 far_el1;               // 0x68 - Fault Address
    u64 par_el1;               // 0x70 - Physical Address
    u64 mair_el1;              // 0x78 - Memory Attributes
    u64 vbar_el1;              // 0x80 - Vector Base Address
    u64 contextidr_el1;        // 0x88 - Context ID
    u64 tpidr_el0;             // 0x90 - Thread ID (User)
    u64 tpidr_el1;             // 0x98 - Thread ID (Priv)
    u64 tpidrro_el0;           // 0xA0 - Thread ID RO
    // ARMv7 32位模式寄存器
    u32 spsr_abt;              // 0xA8
    u32 spsr_und;              // 0xAC
    u32 spsr_irq;              // 0xB0
    u32 spsr_fiq;              // 0xB4
    u32 dacr32_el2;            // 0xB8 - Domain Access
    u32 ifsr32_el2;            // 0xBC - Instruction Fault
    u32 teecr32_el1;           // 0xC0 - ThumbEE Control
    u32 teehbr32_el1;          // 0xC4 - ThumbEE Handler
};

struct arm_priv_vfp {
    u32 mvfr0;                 // Media and VFP Feature Register 0
    u32 mvfr1;                 // Media and VFP Feature Register 1
    u32 mvfr2;                 // Media and VFP Feature Register 2
    u32 fpcr;                  // Floating-point Control
    u32 fpsr;                  // Floating-point Status
    u32 fpexc32;               // FP Exception (ARMv7)
    u64 fpregs[64];            // 32 x 128-bit FP registers
};

struct arm_priv_ptrauth {
    u64 apiakeylo_el1;         // 0x00 - IA key A low
    u64 apiakeyhi_el1;         // 0x08 - IA key A high
    u64 apibkeylo_el1;         // 0x10 - IB key A low
    u64 apibkeyhi_el1;         // 0x18 - IB key A high
    u64 apdakeylo_el1;         // 0x20 - DA key A low
    u64 apdakeyhi_el1;         // 0x28 - DA key A high
    u64 apdbkeylo_el1;         // 0x30 - DB key A low
    u64 apdbkeyhi_el1;         // 0x38 - DB key A high
    u64 apgakeylo_el1;         // 0x40 - GA key A low
    u64 apgakeyhi_el1;         // 0x48 - GA key A high
};
```

### 2.4 CP15 协处理器 (ARMv7)

**CP15 寄存器分类 (cpu_vcpu_cp15.c):**

| CRn | 寄存器 | 功能 | 代码行数 |
|-----|--------|------|----------|
| 0 | MIDR, CCSIDR, CLIDR | CPU ID, 缓存 ID | ~50 |
| 1 | SCTLR, ACTLR, CPACR | 系统控制, 辅助控制 | ~80 |
| 2 | TTBR0, TTBR1, TTBCR | 页表基址, 控制 | ~100 |
| 3 | DACR | 域访问控制 | ~30 |
| 5 | DFSR, IFSR | 故障状态 | ~50 |
| 6 | DFAR, IFAR | 故障地址 | ~30 |
| 7 | cache ops | 缓存操作 | ~120 |
| 8 | TLB ops | TLB 操作 | ~80 |
| 9 | PMU | 性能监控 | ~40 |
| 10 | PRRR, NMRR | 内存区域 | ~30 |
| 12 | VBAR, MVBAR | 向量基址 | ~40 |
| 13 | FCSE, CONTEXT | 进程 ID | ~20 |
| 15 | implementation | 实现特定 | ~30 |

**总计**: 653 行 CP15 仿真代码

### 2.5 VGIC (虚拟 GIC) 架构

**VGIC 数据结构:**

```c
struct vgic_guest_state {
    struct vmm_guest *guest;
    u8 id[8];                  // VGIC 类型 ID
    u32 num_cpu;               // CPU 数量
    u32 num_irq;               // IRQ 数量
    struct vgic_vcpu_state vstate[VGIC_MAX_NCPU];
    vmm_spinlock_t dist_lock;
    u32 enabled;
    struct vgic_irq_state irq_state[VGIC_MAX_NIRQ];
    u32 sgi_source[VGIC_MAX_NCPU][16];  // SGI 源
    u32 irq_target[VGIC_MAX_NIRQ];
    u32 priority1[32][VGIC_MAX_NCPU];
    u32 priority2[VGIC_MAX_NIRQ - 32];
    u32 irq_enabled[VGIC_MAX_NCPU][VGIC_MAX_NIRQ / 32];
    u32 irq_pending[VGIC_MAX_NCPU][VGIC_MAX_NIRQ / 32];
};

struct vgic_vcpu_state {
    struct vmm_vcpu *vcpu;
    u32 parent_irq;
    struct vgic_hw_state hw;   // 硬件状态
    u32 lr_used_count;
    u32 lr_used[VGIC_MAX_LRS / 32];
    u8 irq_lr[VGIC_MAX_NIRQ];  // IRQ -> LR 映射
};
```

**VGIC 文件大小对比:**

| 文件 | 大小 | 功能 |
|------|------|------|
| vgic.c | ~40KB | 通用 VGIC 实现 |
| vgic_v2.c | ~7.7KB | GICv2 特定实现 |
| vgic_v3.c | ~11.7KB | GICv3 特定实现 |

### 2.6 LPAE MMU (Stage-2) 架构

**页表级别常量 (mmu_lpae.c):**

```c
#define TTBL_L0_BLOCK_SIZE     (512ULL * 1024 * 1024 * 1024)  // 512GB
#define TTBL_L1_BLOCK_SIZE     (1ULL * 1024 * 1024 * 1024)    // 1GB
#define TTBL_L2_BLOCK_SIZE     (2ULL * 1024 * 1024)           // 2MB
#define TTBL_L3_BLOCK_SIZE     (4ULL * 1024)                  // 4KB

#define TTBL_L0_BLOCK_SHIFT    39
#define TTBL_L1_BLOCK_SHIFT    30
#define TTBL_L2_BLOCK_SHIFT    21
#define TTBL_L3_BLOCK_SHIFT    12

#define TTBL_L0_INDEX_SHIFT    39
#define TTBL_L1_INDEX_SHIFT    30
#define TTBL_L2_INDEX_SHIFT    21
#define TTBL_L3_INDEX_SHIFT    12

#define TTBL_L0_MAP_MASK       0x7FFFFFFFFF000
#define TTBL_L1_MAP_MASK       0x3FFFFF000
#define TTBL_L2_MAP_MASK       0x1FFFE000
#define TTBL_L3_MAP_MASK       0xFFFFF000
```

**MMU 操作函数:**

| 函数 | 功能 |
|------|------|
| arch_mmu_pgtbl_min_align_order() | 页表对齐 |
| arch_mmu_pgtbl_size_order() | 页表大小 |
| arch_mmu_stage2_tlbflush() | Stage-2 TLB 刷新 |
| arch_mmu_stage1_tlbflush() | Stage-1 TLB 刷新 |
| arch_mmu_valid_block_size() | 验证块大小 |
| arch_mmu_start_level() | 起始级别 |
| arch_mmu_level_block_size() | 级别块大小 |
| arch_mmu_level_block_shift() | 级别位移 |
| arch_mmu_level_map_mask() | 级别映射掩码 |
| arch_mmu_level_index() | 级别索引 |

### 2.7 指令仿真 (emulate_arm.c)

**emulate_arm.c 是最大的单个源文件** (~105KB, ~2700 行)

**支持的指令类别:**
- 数据处理指令
- 加载/存储指令
- 分支指令
- 协处理器指令
- SIMD 指令

---

## 三、ARM 架构支持差距分析

### 3.1 对比表：Ferrovisor vs Xvisor

| 功能模块 | Xvisor | Ferrovisor | 差距 |
|----------|--------|------------|------|
| **CPU 核心** | | | |
| ARMv7 HYP 模式 | ✅ 4780 行 | ❌ | 完全缺失 |
| ARMv8 EL2 模式 | ✅ 4422 行 | ❌ | 完全缺失 |
| VCPU 上下文切换 | ✅ | ❌ | 完全缺失 |
| 特权级管理 | ✅ | ❌ | 完全缺失 |
| **内存管理** | | | |
| Stage-2 页表 | ✅ 397 行 | ❌ | 完全缺失 |
| LPAE 支持 | ✅ | ❌ | 完全缺失 |
| VTTBR/VTCR 管理 | ✅ | ❌ | 完全缺失 |
| VMID 分配 | ✅ | ❌ | 完全缺失 |
| **中断虚拟化** | | | |
| VGIC v2 | ✅ ~48KB | ❌ | 完全缺失 |
| VGIC v3 | ✅ ~52KB | ❌ | 完全缺失 |
| 虚拟中断注入 | ✅ | ❌ | 完全缺失 |
| **系统寄存器仿真** | | | |
| CP15 (ARMv7) | ✅ 653 行 | ❌ | 完全缺失 |
| 系统寄存器 (ARMv8) | ✅ 464 行 | ❌ | 完全缺失 |
| ID 寄存器仿真 | ✅ | ❌ | 完全缺失 |
| **FPU 虚拟化** | | | |
| VFP 仿真 | ✅ | ❌ | 完全缺失 |
| NEON/ASIMD | ✅ | ❌ | 完全缺失 |
| Lazy FPU 切换 | ✅ | ❌ | 完全缺失 |
| **电源管理** | | | |
| PSCI v0.2 | ✅ 8.7KB | ❌ | 完全缺失 |
| CPU Hotplug | ✅ | ❌ | 完全缺失 |
| WFI 处理 | ✅ | ❌ | 完全缺失 |
| **SMP 支持** | | | |
| PSCI 启动 | ✅ | ❌ | 完全缺失 |
| Spin Table 启动 | ✅ | ❌ | 完全缺失 |
| SCU 启动 | ✅ 5.2KB | ❌ | 完全缺失 |
| **Timer 虚拟化** | | | |
| Generic Timer | ✅ 16.7KB | ❌ | 完全缺失 |
| 虚拟 Timer | ✅ | ❌ | 完全缺失 |
| **设备树** | | | |
| ARM 设备树解析 | ✅ | 部分 | 需 ARM 特定适配 |
| 虚拟设备树生成 | ✅ | 部分 | 需 ARM 特定适配 |
| **板级支持** | | | |
| QEMU virt | ✅ | ❌ | 完全缺失 |
| Raspberry Pi | ✅ | ❌ | 完全缺失 |
| Rockchip | ✅ | ❌ | 完全缺失 |

### 3.2 需要移植的关键文件数量

| 类别 | 文件数 | 总代码量 (行) |
|------|--------|---------------|
| ARM64 CPU 文件 | 35 | ~4,422 |
| ARMv7 CPU 文件 | 35 | ~4,780 |
| ARM Common 文件 | 17 | ~15,000 |
| 板级支持 | 4+ | ~2,000 |
| **总计** | **90+** | **~26,000** |

---

## 四、ARM 支持实施计划

### 阶段 0：准备阶段 (Week 1-2)

#### 3.0.1 架构设计

**任务：**
- [ ] 设计 ARM64/ARMv7 CPU 抽象层接口
- [ ] 定义与 RISC-V 共享的虚拟化抽象接口
- [ ] 制定 ARM 模块目录结构（参考 Xvisor）
- [ ] 确定 ARMv8 EL2 和 ARMv7 HYP 模式支持策略

**参考文件：**
- `xvisor/arch/arm/cpu/arm64/include/arch_regs.h`
- `xvisor/arch/arm/cpu/arm32ve/include/arch_regs.h`
- `xvisor/arch/arm/include/arm_features.h`

**交付物：**
- `arch/arm64/` 和 `arch/arm32/` 目录结构设计文档
- ARM 虚拟化抽象接口定义 (`arch/arm/cpu/interface.md`)
- 与 RISC-V 共享的 trait 定义

#### 3.0.2 开发环境搭建

**任务：**
- [ ] 配置 ARM 交叉编译工具链 (aarch64-none-elf, arm-none-eabi)
- [ ] 设置 QEMU ARM virt 平台测试环境
  - QEMU ARM virt: `qemu-system-aarch64 -M virt`
  - QEMU ARM vexpress: `qemu-system-arm -M vexpress-a15`
- [ ] 准备 ARM 开发板测试环境 (可选：Raspberry Pi 4, Rockchip)
- [ ] 创建 ARM 构建配置 (`.cargo/config.toml`)

**参考文件：**
- `xvisor/build/arm64/` - ARM64 构建配置
- `xvisor/build/arm32ve/` - ARMv7 构建配置

**交付物：**
- ARM 交叉编译脚本 (`scripts/build-arm.sh`)
- QEMU ARM 启动脚本 (`scripts/run-qemu-arm.sh`)
- CI/CD ARM 构建配置

---

### 阶段 1：CPU 基础支持 (Week 3-6)

> **状态更新 (2025-12-27):** 部分任务已在阶段 0.1 中完成基础框架

#### 3.1.1 ARMv8 EL2 模式初始化

**任务：**
- [ ] 实现 EL2 入口代码 (`arch/arm64/cpu/entry.S`)
- [x] 实现 CPU 初始化框架 (`arch/arm64/cpu/init.rs`)
  - [x] EL2 进入和配置框架
  - [x] HCR_EL2 寄存器位定义
  - [x] SCTLR_EL2 位定义
  - [x] VTCR_EL2 位定义
  - [ ] 完整初始化流程 (TODO)
- [ ] 实现异常向量表 (`arch/arm64/interrupt/vectors.S`)
  - 同步异常
  - IRQ 异常
  - FIQ 异常
  - SError 异常
- [ ] 实现 EL2 到 EL1 降级 (可选 VHE)

**参考文件：**
- `xvisor/arch/arm/cpu/arm64/cpu_init.c` (112 行)
- `xvisor/arch/arm/cpu/arm64/cpu_entry.S`
- `xvisor/arch/arm/cpu/arm64/include/cpu_defines.h`

**关键初始化步骤 (参考 cpu_init.c):**
1. 检测 CPU ID 和特性
2. 配置 EL2 系统寄存器
3. 设置异常向量表
4. 配置 VFP/NEON
5. 使能缓存
6. 配置 MMU

**交付物：**
- [x] `arch/arm64/cpu/init.rs` (部分完成)
- [ ] `arch/arm64/cpu/entry.S`
- [ ] `arch/arm64/interrupt/vectors.S`

#### 3.1.2 ARMv7 HYP 模式初始化

**任务：**
- [ ] 实现 HYP 入口代码 (`arch/arm32/cpu/entry.S`)
- [ ] 实现 CPU 初始化 (`arch/arm32/cpu/init.rs`)
  - HYP 模式进入
  - HCR, HCPTR, HSTR 寄存器初始化
  - HSCTLR 配置
- [ ] 实现异常向量表 (`arch/arm32/interrupt/vectors.S`)

**参考文件：**
- `xvisor/arch/arm/cpu/arm32ve/cpu_init.c` (113 行)
- `xvisor/arch/arm/cpu/arm32ve/cpu_entry.S`

**交付物：**
- `arch/arm32/cpu/entry.S`
- `arch/arm32/cpu/init.rs`
- `arch/arm32/interrupt/vectors.S`

#### 3.1.3 CPU 寄存器管理

> **状态更新 (2025-12-27):** 部分完成

**任务：**
- [x] 实现系统寄存器访问接口 (`arch/arm64/cpu/regs.rs`)
  - [x] MSR/MRRS 指令封装 (inline asm)
  - [ ] 通用寄存器 (x0-x30) 框架
  - [ ] 特殊寄存器 (SP, PC, PSTATE)
- [x] 实现 EL2 系统寄存器定义 (`arch/arm64/mod.rs` el2_regs 模块)
  - [x] HCR_EL2, VTTBR_EL2, VTCR_EL2 编码
  - [ ] CPTR_EL2, HSTR_EL2 (TODO)
  - [ ] SCR_EL3 (如果支持)
- [x] 实现 ID 寄存器解析 (`arch/arm64/cpu/regs.rs` info 模块)
  - [x] MIDR_EL1, MPIDR_EL1
  - [ ] ID_AA64PFR0_EL1 ~ ID_AA64MMFR2_EL1 (TODO)
- [x] 实现 CPU 特性检测 (`arch/arm64/cpu/features.rs`)
  - [x] CpuInfo 结构
  - [x] CpuFeatures bitflags
  - [ ] ARMv8.0/8.1/8.2/8.3/8.4/8.5/8.6/9.0 版本检测 (TODO)
  - [ ] 虚拟化扩展检测 (TODO)
  - [ ] PAN/UAO 支持 (TODO)
  - [ ] SVE 检测 (TODO)
  - [ ] Pointer Authentication 检测 (TODO)

**参考文件：**
- `xvisor/arch/arm/cpu/arm64/include/arch_regs.h`
- `xvisor/arch/arm/include/arm_features.h`
- `xvisor/arch/arm/cpu/arm64/cpu_inline_asm.h`

**交付物：**
- [x] `arch/arm64/cpu/regs.rs` (部分完成)
- [ ] `arch/arm64/cpu/el2_regs.rs` (整合到 mod.rs)
- [ ] `arch/arm64/cpu/id.rs` (整合到 regs.rs)
- [x] `arch/arm64/cpu/features.rs` (部分完成)

#### 3.1.4 VCPU 上下文切换

> **状态更新 (2025-12-27):** 结构体定义完成，汇编实现待完成

**任务：**
- [x] 实现 VCPU 上下文结构 (`arch/arm64/cpu/state.rs`)
  - [x] SavedGprs (x0-x30)
  - [x] SavedSpecialRegs (SP, PC, PSTATE)
  - [x] SavedEl1SysRegs
  - [x] SavedVfpRegs
  - [x] ArmPrivContext
  - [x] VcpuContext
- [ ] 实现上下文切换汇编 (`arch/arm64/cpu/vcpu/switch.S`)
  - Host -> Guest 切换 (ERET 到 EL1)
  - Guest -> Host 切换 (异常到 EL2)
  - VCPU 状态保存/恢复
- [ ] 实现 Traps 处理 (`arch/arm64/cpu/vcpu/trap.rs`)
  - 异常级别转换处理
  - 异步异常处理
  - Fault 处理

**参考文件：**
- `xvisor/arch/arm/cpu/arm64/cpu_vcpu_helper.c` (899 行)
- `xvisor/arch/arm/cpu/arm64/cpu_vcpu_switch.S`
- `xvisor/arch/arm/cpu/arm64/include/cpu_vcpu_helper.h`
- `xvisor/arch/arm/cpu/arm64/include/cpu_vcpu_switch.h`

**arm_priv_sysregs 字段映射:**
```
sp_el0      0x00
sp_el1      0x08
elr_el1     0x10
spsr_el1    0x18
midr_el1    0x20
mpidr_el1   0x28
sctlr_el1   0x30
actlr_el1   0x38
cpacr_el1   0x40
ttbr0_el1   0x48
ttbr1_el1   0x50
tcr_el1     0x58
esr_el1     0x60
far_el1     0x68
par_el1     0x70
mair_el1    0x78
vbar_el1    0x80
contextidr_el1 0x88
tpidr_el0   0x90
tpidr_el1   0x98
tpidrro_el0 0xA0
```

**交付物：**
- `arch/arm64/cpu/vcpu/context.rs`
- `arch/arm64/cpu/vcpu/switch.S`
- `arch/arm64/cpu/vcpu/trap.rs`
- `arch/arm64/cpu/vcpu/mod.rs`

---

### 阶段 2：内存管理单元 (MMU) (Week 7-10)

#### 3.2.1 Stage-2 页表管理

> **状态更新 (2025-12-27):** ✅ 已完成 Stage-2 页表结构、VTTBR_EL2 管理和页表操作

**任务：**
- [x] 实现 Stage-2 页表结构 (`arch/arm64/mmu/stage2.rs`)
  - [x] 4级页表 (48-bit IPA)
  - [x] 3级页表 (可选 40-bit IPA)
  - [x] 页表项格式定义 (PTE bits, masks, attributes)
- [x] 实现 VTTBR_EL2 管理 (`arch/arm64/mmu/vttbr.rs`)
  - [x] VMID 分配器 (AtomicU64 bitmap, 线程安全)
  - [x] 页表基址管理
  - [x] VMID 8-bit 分配 (256 VMs)
- [ ] 实现 VTCR_EL2 配置 (`arch/arm64/mmu/vtcr.rs`)
  - [ ] T0SZ, SL0, IRGN0, ORGN0, SH0, TG0 配置
  - [ ] VTCR_EL2 值计算
- [x] 实现 Stage-2 页表操作 (`arch/arm64/mmu/operations.rs`)
  - [x] 页表映射/取消映射 (map_range, unmap_range)
  - [x] TLB 无效化 (TLBI IPAS2E1IS, TLBI VMALLS12E1IS)
  - [x] 缓存维护 (pte_sync, DMB/DSB)

**参考文件：**
- `xvisor/arch/arm/cpu/common/mmu_lpae.c` (397 行)
- `xvisor/arch/arm/cpu/common/include/mmu_lpae.h`
- `xvisor/arch/arm/cpu/arm64/include/arch_mmu.h`

**LPAE 页表常量:**
```rust
pub const TTBL_L0_BLOCK_SIZE: u64 = 512 * 1024 * 1024 * 1024;  // 512GB
pub const TTBL_L1_BLOCK_SIZE: u64 = 1 * 1024 * 1024 * 1024;     // 1GB
pub const TTBL_L2_BLOCK_SIZE: u64 = 2 * 1024 * 1024;            // 2MB
pub const TTBL_L3_BLOCK_SIZE: u64 = 4 * 1024;                   // 4KB

pub const TTBL_L0_BLOCK_SHIFT: u32 = 39;
pub const TTBL_L1_BLOCK_SHIFT: u32 = 30;
pub const TTBL_L2_BLOCK_SHIFT: u32 = 21;
pub const TTBL_L3_BLOCK_SHIFT: u32 = 12;
```

**交付物：**
- `arch/arm64/mmu/stage2.rs`
- `arch/arm64/mmu/vttbr.rs`
- `arch/arm64/mmu/vtcr.rs`
- `arch/arm64/mmu/operations.rs`

#### 3.2.2 地址转换

**任务：**
- [ ] 实现 IPA -> PA 转换 (`arch/arm64/mmu/translate.rs`)
  - Walk Stage-2 页表
  - 处理页错误
  - Fault 解码
- [ ] 实现内存属性管理 (`arch/arm64/mmu/attrs.rs`)
  - MAIR_EL2 配置
  - Device/Greedy/Normal 内存类型
  - Shareability 属性
- [ ] 实现 VMID 管理 (`arch/arm64/mmu/vmid.rs`)
  - VMID 分配/回收
  - VMID 刷新 (VMALL)

**参考文件：**
- `xvisor/arch/arm/cpu/common/mmu_lpae.c` - arch_mmu_level_index()

**交付物：**
- `arch/arm64/mmu/translate.rs`
- `arch/arm64/mmu/attrs.rs`
- `arch/arm64/mmu/vmid.rs`

#### 3.2.3 与共享 MMU 框架集成

**任务：**
- [ ] 实现 GStage trait for ARM (`arch/arm64/mmu/gstage.rs`)
  - 类似 RISC-V 的 GStageManager
  - 与 `core/mm/gstage.rs` 集成
- [ ] 实现 Stage-2 缺页处理 (`arch/arm64/mmu/fault.rs`)
  - IPA fault 处理
  - Permission fault 处理

**交付物：**
- `arch/arm64/mmu/gstage.rs`
- `arch/arm64/mmu/fault.rs`
- `arch/arm64/mmu/mod.rs`

---

### 阶段 3：中断虚拟化 (Week 11-14)

#### 3.3.1 GIC 基础支持

**任务：**
- [ ] 实现 GICv2 驱动 (`drivers/irqchip/arm_gicv2.rs`)
  - GICD (Distributor) 管理
    - GICD_CTLR, GICD_TYPER, GICD_ISENABLER, GICD_ICENABLER
    - GICD_ISPENDR, GICD_ICPENDR
    - GICD_IPRIORITYR, GICD_ITARGETSR
  - GICC (CPU Interface) 管理
    - GICC_CTLR, GICC_PMR, GICC_BPR, GICC_IAR, GICC_EOIR
  - 中断使能/禁用
  - 中断优先级配置
- [ ] 实现 GICv3 驱动 (`drivers/irqchip/arm_gicv3.rs`)
  - GICD (Distributor) 管理
  - GICR (Redistributor) 管理
    - GICR_WAKER, GICR_PROPBASER, GICR_PENDBASER
  - ARE (Affinity Routing Enable) 支持
  - ICC 系统寄存器接口
- [ ] 实现 GIC 发现 (`arch/arm64/interrupt/gic_discovery.rs`)
  - 设备树解析
  - 版本检测
  - IRQ 数量检测

**参考文件：**
- `xvisor/drivers/irqchip/irq-gic.c`
- `xvisor/drivers/irqchip/irq-gic-v3.c`
- `xvisor/arch/arm/cpu/common/vgic.c`
- `xvisor/arch/arm/cpu/common/vgic_v3.c`
- `xvisor/arch/arm/cpu/arm64/include/arch_gicv3.h`

**交付物：**
- `drivers/irqchip/arm_gicv2.rs`
- `drivers/irqchip/arm_gicv3.rs`
- `arch/arm64/interrupt/gic_discovery.rs`

#### 3.3.2 VGIC (虚拟 GIC) 实现

**任务：**
- [ ] 实现 VGIC 框架 (`arch/arm64/interrupt/vgic/mod.rs`)
  - VGIC 状态管理
  - List Register (LR) 管理
- [ ] 实现 VGIC v2 (`arch/arm64/interrupt/vgic/vgicv2.rs`)
  - 虚拟 CPU 接口仿真
  - 中断注入到 Guest
  - LR 寄存器管理
- [ ] 实现 VGIC v3 (`arch/arm64/interrupt/vgic/vgicv3.rs`)
  - 虚拟 Redistributor
  - ICC 系统寄存器仿真
    - ICC_IAR1_EL1, ICC_EOIR1_EL1
    - ICC_IGRPEN0_EL1, ICC_IGRPEN1_EL1
  - INTID 范围扩展支持
- [ ] 实现 VGIC 中断路由 (`arch/arm64/interrupt/vgic/routing.rs`)
  - SGI (0-15) 路由
  - PPI (16-31) 路由
  - SPI (32-1019) 路由
  - LPI (1024+) 路由 (可选)

**参考文件：**
- `xvisor/arch/arm/cpu/common/vgic.c` (~40KB)
- `xvisor/arch/arm/cpu/common/vgic_v2.c` (~7.7KB)
- `xvisor/arch/arm/cpu/common/vgic_v3.c` (~11.7KB)

**VGIC 数据结构映射:**
```rust
pub struct VgicGuestState {
    pub guest: GuestRef,
    pub id: [u8; 8],
    pub num_cpu: u32,
    pub num_irq: u32,
    pub vcpu_states: [VgicVcpuState; VGIC_MAX_NCPU],
    pub dist_lock: SpinLock,
    pub enabled: u32,
    pub irq_states: [VgicIrqState; VGIC_MAX_NIRQ],
    pub sgi_sources: [[u32; 16]; VGIC_MAX_NCPU],
    pub irq_targets: [u32; VGIC_MAX_NIRQ],
    pub priorities: [[u32; 32]; VGIC_MAX_NCPU],
    pub priority2: [u32; VGIC_MAX_NIRQ - 32],
    pub irq_enabled: [[u32; VGIC_MAX_NIRQ / 32]; VGIC_MAX_NCPU],
    pub irq_pending: [[u32; VGIC_MAX_NIRQ / 32]; VGIC_MAX_NCPU],
}

pub const VGIC_MAX_NCPU: usize = 8;
pub const VGIC_MAX_NIRQ: usize = 256;
pub const VGIC_MAX_LRS: usize = 4;  // GICv2
```

**交付物：**
- `arch/arm64/interrupt/vgic/mod.rs`
- `arch/arm64/interrupt/vgic/vgicv2.rs`
- `arch/arm64/interrupt/vgic/vgicv3.rs`
- `arch/arm64/interrupt/vgic/routing.rs`

#### 3.3.3 虚拟中断处理

**任务：**
- [ ] 实现虚拟中断注入 (`arch/arm64/interrupt/virq.rs`)
  - 设置 VGIC LR
  - HCR_EL2.VI/VF 位管理
  - 中断优先级处理
- [ ] 实现虚拟中断 EOI 处理
- [ ] 实现中断委托 (HIDELEG)

**交付物：**
- `arch/arm64/interrupt/virq.rs`
- `arch/arm64/interrupt/mod.rs`

---

### 阶段 4：系统寄存器虚拟化 (Week 15-18)

#### 3.4.1 系统寄存器仿真框架

**任务：**
- [ ] 实现系统寄存器 trap 处理 (`arch/arm64/cpu/sysreg/trap.rs`)
  - HSTR_EL2 trap 处理
  - CPTR_EL2 trap 处理 (TCPAC, TFP, TTA)
  - 系统寄存器访问解码
- [ ] 实现系统寄存器读写分发器 (`arch/arm64/cpu/sysreg/dispatch.rs`)
  - Op0, Op1, CRn, CRm, Op2 解码
  - 寄存器访问路由
- [ ] 实现保存的寄存器状态 (`arch/arm64/cpu/sysreg/state.rs`)
  - 每个 VCPU 的系统寄存器状态

**参考文件：**
- `xvisor/arch/arm/cpu/arm64/cpu_vcpu_sysregs.c` (464 行)
- `xvisor/arch/arm/cpu/arm64/include/cpu_vcpu_sysregs.h`

**交付物：**
- `arch/arm64/cpu/sysreg/mod.rs`
- `arch/arm64/cpu/sysreg/trap.rs`
- `arch/arm64/cpu/sysreg/dispatch.rs`
- `arch/arm64/cpu/sysreg/state.rs`

#### 3.4.2 关键系统寄存器实现

**任务：**
- [ ] 实现 ID 寄存器仿真 (`arch/arm64/cpu/sysreg/id_regs.rs`)
  - ID_AA64PFR0_EL1 ~ ID_AA64PFR1_EL1
  - ID_AA64DFR0_EL1 ~ ID_AA64DFR1_EL1
  - ID_AA64ISAR0_EL1 ~ ID_AA64ISAR2_EL1
  - ID_AA64MMFR0_EL1 ~ ID_AA64MMFR2_EL1
  - MIDR_EL1, MPIDR_EL1, REVIDR_EL1
- [ ] 实现系统控制寄存器 (`arch/arm64/cpu/sysreg/sctlr.rs`)
  - SCTLR_EL1 仿真
  - ACTLR_EL1 仿真
- [ ] 实现页表寄存器 (`arch/arm64/cpu/sysreg/mm.rs`)
  - TTBR0_EL1, TTBR1_EL1
  - TCR_EL1
  - MAIR_EL1
  - AMAIR_EL1
- [ ] 实现调试寄存器 (`arch/arm64/cpu/sysreg/debug.rs`)
  - MDSCR_EL1
  - DBG registers

**交付物：**
- `arch/arm64/cpu/sysreg/id_regs.rs`
- `arch/arm64/cpu/sysreg/sctlr.rs`
- `arch/arm64/cpu/sysreg/mm.rs`
- `arch/arm64/cpu/sysreg/debug.rs`

#### 3.4.3 CP15 协处理器仿真 (ARMv7)

**任务：**
- [ ] 实现 CP15 协处理器框架 (`arch/arm32/cpu/coproc/cp15.rs`)
- [ ] 实现 CP15 寄存器读写
  - CRn=0: MIDR, CCSIDR, CLIDR, CCSIDR2
  - CRn=1: SCTLR, ACTLR, CPACR
  - CRn=2: TTBR0, TTBR1, TTBCR
  - CRn=3: DACR
  - CRn=5: DFSR, IFSR
  - CRn=6: DFAR, IFAR
  - CRn=7: 缓存操作
  - CRn=8: TLB 操作
  - CRn=9: 性能监控
  - CRn=10: PRRR, NMRR
  - CRn=12: VBAR, MVBAR, ISR
  - CRn=13: FCSE, CONTEXT
  - CRn=15: 实现特定

**参考文件：**
- `xvisor/arch/arm/cpu/arm32ve/cpu_vcpu_cp15.c` (653 行)
- `xvisor/arch/arm/cpu/arm32ve/include/cpu_vcpu_cp15.h`

**CP15 寄存器类别 (653 行代码):**
```rust
pub enum Cp15Register {
    // CRn=0
    Midr,        // Main ID Register
    Ctr,         // Cache Type Register
    TcmType,     // TCM Type Register
    Ccsidr,      // Cache Size ID Register
    Clidr,       // Cache Level ID Register
    // CRn=1
    Sctlr,       // System Control Register
    Actlr,       // Auxiliary Control Register
    Cpacr,       // Coprocessor Access Control Register
    // CRn=2
    Ttbr0,       // Translation Table Base Register 0
    Ttbr1,       // Translation Table Base Register 1
    Ttbcr,       // Translation Table Base Control Register
    // ... 等等
}
```

**交付物：**
- `arch/arm32/cpu/coproc/cp15.rs`
- `arch/arm32/cpu/coproc/mod.rs`

#### 3.4.4 CP14 协处理器仿真 (ARMv7)

**任务：**
- [ ] 实现 CP14 调试协处理器 (`arch/arm32/cpu/coproc/cp14.rs`)
  - ThumbEE 寄存器
  - 调试寄存器 (部分)

**参考文件：**
- `xvisor/arch/arm/cpu/arm32ve/cpu_vcpu_cp14.c` (218 行)

**交付物：**
- `arch/arm32/cpu/coproc/cp14.rs`

---

### 阶段 5：FPU/SIMD 虚拟化 (Week 19-20)

#### 3.5.1 VFP/NEON 保存和恢复

**任务：**
- [ ] 实现 VFP 寄存器保存 (`arch/arm64/cpu/fpu/vfp.rs`)
  - V registers (V0-V31, 128-bit)
  - FPSCR, FPCR, FPSR
  - MVFR0, MVFR1, MVFR2
- [ ] 实现 NEON/ASIMD 支持 (`arch/arm64/cpu/fpu/neon.rs`)
  - SIMD 寄存器管理
  - SVE 寄存器管理 (可选)
- [ ] 实现 Lazy FPU 切换 (`arch/arm64/cpu/fpu/lazy.rs`)
  - CPTR_EL2.TFP 设置
  - FPU trap 处理
  - 延迟保存/恢复

**参考文件：**
- `xvisor/arch/arm/cpu/arm64/cpu_vcpu_vfp.c` (156 行)
- `xvisor/arch/arm/cpu/arm32ve/cpu_vcpu_vfp.c` (193 行)
- `xvisor/arch/arm/cpu/arm64/include/cpu_vcpu_vfp.h`
- `xvisor/arch/arm/cpu/arm64/include/arm_priv_vfp.h`

**arm_priv_vfp 结构:**
```rust
pub struct ArmPrivVfp {
    pub mvfr0: u32,      // Media and VFP Feature Register 0
    pub mvfr1: u32,      // Media and VFP Feature Register 1
    pub mvfr2: u32,      // Media and VFP Feature Register 2
    pub fpcr: u32,       // Floating-point Control Register
    pub fpsr: u32,       // Floating-point Status Register
    pub fpexc32: u32,    // FP Exception Register (ARMv7)
    pub fpregs: [u64; 64], // 32 x 128-bit FP registers
}
```

**交付物：**
- `arch/arm64/cpu/fpu/mod.rs`
- `arch/arm64/cpu/fpu/vfp.rs`
- `arch/arm64/cpu/fpu/neon.rs`
- `arch/arm64/cpu/fpu/lazy.rs`

---

### 阶段 6：电源管理 (Week 21-22)

#### 3.6.1 PSCI 实现

**任务：**
- [ ] 实现 PSCI v0.2/v1.0 接口 (`arch/arm64/psci/mod.rs`)
  - PSCI_VERSION
  - CPU_ON (启动 CPU)
  - CPU_OFF (关闭 CPU)
  - CPU_SUSPEND (CPU 挂起)
  - AFFINITY_INFO (查询 CPU 状态)
  - MIGRATE (迁移)
  - SYSTEM_OFF
  - SYSTEM_RESET
- [ ] 实现 PSCI SMC 调用处理 (`arch/arm64/psci/smccc.rs`)
  - SMC 调用约定
  - SMC64/SMC32 支持
  - 标准服务调用 (PSCI)
- [ ] 实现 CPU 状态管理 (`arch/arm64/psci/cpu_state.rs`)
  - CPU 在线/离线状态
  - CPU 挂起状态
  - 亲和级别状态

**参考文件：**
- `xvisor/arch/arm/cpu/common/emulate_psci.c` (8.7KB)
- `xvisor/arch/arm/cpu/common/arm_psci.c` (7.4KB)
- `xvisor/arch/arm/cpu/common/include/psci.h`

**PSCI 函数定义:**
```rust
pub const PSCI_0_2_FN_BASE: u32 = 0x84000000;
pub const PSCI_0_2_FN_PSCI_VERSION: u32 = 0;
pub const PSCI_0_2_FN_CPU_SUSPEND: u32 = 1;
pub const PSCI_0_2_FN_CPU_OFF: u32 = 2;
pub const PSCI_0_2_FN_CPU_ON: u32 = 3;
pub const PSCI_0_2_FN_AFFINITY_INFO: u32 = 4;
pub const PSCI_0_2_FN_MIGRATE: u32 = 5;
pub const PSCI_0_2_FN_SYSTEM_OFF: u32 = 8;
pub const PSCI_0_2_FN_SYSTEM_RESET: u32 = 9;

pub const PSCI_RET_SUCCESS: u32 = 0;
pub const PSCI_RET_NOT_SUPPORTED: u32 = -1;
pub const PSCI_RET_INVALID_PARAMS: u32 = -2;
pub const PSCI_RET_DENIED: u32 = -3;
pub const PSCI_RET_ALREADY_ON: u32 = -4;
```

**交付物：**
- `arch/arm64/psci/mod.rs`
- `arch/arm64/psci/smccc.rs`
- `arch/arm64/psci/cpu_state.rs`

#### 3.6.2 WFI/WFE 处理

**任务：**
- [ ] 实现 WFI 陷阱处理 (`arch/arm64/cpu/wfi.rs`)
  - WFI 指令 trap
  - 低功耗状态处理
- [ ] 实现 WFE 处理 (`arch/arm64/cpu/wfe.rs`)
  - SEV 指令处理
  - 事件队列管理

**交付物：**
- `arch/arm64/cpu/wfi.rs`
- `arch/arm64/cpu/wfe.rs`

---

### 阶段 7：SMP 支持 (Week 23-24)

#### 3.7.1 SMP 启动

**任务：**
- [ ] 实现 SMP 框架 (`arch/arm64/smp/mod.rs`)
- [ ] 实现 Spin Table 启动 (`arch/arm64/smp/spin_table.rs`)
  - 从设备树读取 spin table 地址
  - 写入启动入口点和 CPU ID
- [ ] 实现 PSCI 启动 (`arch/arm64/smp/psci_boot.rs`)
  - 使用 PSCI CPU_ON 启动从 CPU
- [ ] 实现 SCU 启动 (`arch/arm64/smp/scu.rs`) (ARMv7)
  - Snoop Control Unit 初始化
- [ ] 实现 SMP 初始化 (`arch/arm64/smp/init.rs`)
  - 从 CPU 启动流程
  - CPU 同步机制

**参考文件：**
- `xvisor/arch/arm/cpu/common/smp_ops.c` (9.7KB)
- `xvisor/arch/arm/cpu/common/smp_spin_table.c`
- `xvisor/arch/arm/cpu/common/smp_psci.c`
- `xvisor/arch/arm/cpu/common/smp_scu.c` (5.2KB)
- `xvisor/arch/arm/cpu/common/smp_imx.c` (5.6KB)
- `xvisor/arch/arm/board/common/include/smp_ops.h`

**SMP 操作函数:**
```rust
pub trait SmpOps {
    fn cpu_start(cpu_id: u32, entry_addr: usize, arg: usize) -> Result;
    fn cpu_stop(cpu_id: u32) -> Result;
    fn cpu_on(cpu_id: u32) -> bool;
    fn cpu_offline(cpu_id: u32) -> bool;
}
```

**交付物：**
- `arch/arm64/smp/mod.rs`
- `arch/arm64/smp/spin_table.rs`
- `arch/arm64/smp/psci_boot.rs`
- `arch/arm64/smp/scu.rs`
- `arch/arm64/smp/init.rs`

#### 3.7.2 CPU Hotplug

**任务：**
- [ ] 实现 CPU 热插拔 (`arch/arm64/smp/hotplug.rs`)
  - CPU 在线/离线操作
  - CPU 通知机制

**交付物：**
- `arch/arm64/smp/hotplug.rs`

---

### 阶段 8：Timer 虚拟化 (Week 25-26)

#### 3.8.1 Generic Timer 支持

**任务：**
- [ ] 实现 Generic Timer 驱动 (`drivers/timer/arm_generic_timer.rs`)
  - CNTP (Physical Timer) 访问
  - CNTV (Virtual Timer) 访问
  - CNTHP (Hyp Physical Timer) 访问
  - Counter 频率配置
  - Timer 中断处理
- [ ] 实现虚拟 Timer (`arch/arm64/timer/vtimer.rs`)
  - CNTV_CVAL_EL0, CNTV_CTL_EL0
  - CNTVCT_EL0 (Counter)
  - Timer 中断注入
- [ ] 实现 EL2 Timer (`arch/arm64/timer/htimer.rs`)
  - CNTHP_CVAL_EL2
  - CNTHP_CTL_EL2
  - Hypervisor 调度使用

**参考文件：**
- `xvisor/arch/arm/cpu/common/generic_timer.c` (16.7KB)
- `xvisor/arch/arm/cpu/arm64/include/cpu_generic_timer.h`

**Generic Timer 寄存器:**
```rust
// Physical Timer
pub const CNTPCT_EL0: u32;    // Physical Counter
pub const CNTP_CVAL_EL0: u32; // Physical Compare Value
pub const CNTP_TVAL_EL0: u32; // Physical Timer Value
pub const CNTP_CTL_EL0: u32;  // Physical Timer Control

// Virtual Timer
pub const CNTVCT_EL0: u32;    // Virtual Counter
pub const CNTV_CVAL_EL0: u32; // Virtual Compare Value
pub const CNTV_TVAL_EL0: u32; // Virtual Timer Value
pub const CNTV_CTL_EL0: u32;  // Virtual Timer Control

// Hyp Physical Timer
pub const CNTHP_CVAL_EL2: u32; // Hyp Physical Compare Value
pub const CNTHP_TVAL_EL2: u32; // Hyp Physical Timer Value
pub const CNTHP_CTL_EL2: u32;  // Hyp Physical Timer Control

// Counter Frequency
pub const CNTFRQ_EL0: u32;     // Counter Frequency Register
```

**交付物：**
- `drivers/timer/arm_generic_timer.rs`
- `arch/arm64/timer/mod.rs`
- `arch/arm64/timer/vtimer.rs`
- `arch/arm64/timer/htimer.rs`

---

### 阶段 9：设备树和平台支持 (Week 27-28)

#### 3.9.1 ARM 设备树适配

**任务：**
- [ ] 实现 ARM 设备树解析 (`arch/arm64/devtree/parse.rs`)
  - CPU 节点解析 (enable-method, cpu-release-addr)
  - GIC 节点解析 (interrupt-controller)
  - Timer 节点解析 (arm,armv8-timer)
  - CPUS 节点解析
- [ ] 实现虚拟设备树生成 (`arch/arm64/devtree/vm_fdt.rs`)
  - 为 VM 生成 ARM 设备树
  - GIC virt 设备节点
  - Generic Timer 节点
  - CPU 拓扑

**参考文件：**
- `xvisor/arch/arm/dts/arm/` - ARM 设备树源文件
- `xvisor/build/arm64/*.dts` - 预编译设备树

**设备树节点示例:**
```dts
gic: interrupt-controller@... {
    compatible = "arm,gic-v3";
    reg = <...>;
    interrupts = <...>;
    interrupt-controller;
    #interrupt-cells = <3>;
};

timer {
    compatible = "arm,armv8-timer";
    interrupts = <...>;
    clock-frequency = <...>;
};
```

**交付物：**
- `arch/arm64/devtree/mod.rs`
- `arch/arm64/devtree/parse.rs`
- `arch/arm64/devtree/vm_fdt.rs`

#### 3.9.2 平台支持

**任务：**
- [ ] QEMU virt 平台 (`arch/arm64/platform/qemu_virt.rs`)
  - 内存布局
  - 中断映射
  - UART 配置
- [ ] Foundation v8 模型 (`arch/arm64/platform/foundation_v8.rs`)
- [ ] Raspberry Pi 4 (`arch/arm64/platform/rpi4.rs`) (可选)
- [ ] Rockchip RK3399 (`arch/arm64/platform/rk3399.rs`) (可选)

**参考文件：**
- `xvisor/arch/arm/board/generic/foundation-v8.c`
- `xvisor/build/arm64/raspi4.dts`
- `xvisor/build/arm64/rk3399.dts`

**交付物：**
- `arch/arm64/platform/mod.rs`
- `arch/arm64/platform/qemu_virt.rs`
- `arch/arm64/platform/foundation_v8.rs`

---

### 阶段 10：测试和优化 (Week 29-32)

#### 3.10.1 单元测试

**任务：**
- [ ] CPU 单元测试
  - 寄存器读写测试
  - 特性检测测试
- [ ] MMU 单元测试
  - 页表操作测试
  - 地址转换测试
- [ ] VGIC 单元测试
  - 中断注入测试
  - 路由测试
- [ ] 系统寄存器仿真测试

#### 3.10.2 集成测试

**任务：**
- [ ] 启动 Linux Guest (ARMv8)
  - Device Tree boot
  - ACPI boot (可选)
- [ ] 多核测试
  - SMP 启动测试
  - CPU Hotplug 测试
- [ ] 设备测试 (VirtIO)
- [ ] 性能测试

#### 3.10.3 文档完善

**任务：**
- [ ] ARM 架构文档
- [ ] API 文档 (rustdoc)
- [ ] 用户指南
- [ ] 移植指南
- [ ] 调试指南

**交付物：**
- `docs/arm64-architecture.md`
- `docs/arm64-porting-guide.md`
- `docs/arm64-debugging.md`

---

## 五、ARM 目录结构设计

基于 Xvisor 的完整目录结构设计：

```
arch/arm64/                           # ARMv8-A 64位支持
├── cpu/
│   ├── entry.S                      # EL2 入口和异常向量
│   ├── init.rs                      # CPU 初始化
│   ├── regs.rs                      # 通用寄存器访问
│   ├── el2_regs.rs                  # EL2 系统寄存器定义
│   ├── id.rs                        # CPU ID 寄存器解析
│   ├── features.rs                  # CPU 特性检测
│   ├── cache.rs                     # 缓存操作
│   ├── barrier.rs                   # 内存屏障
│   ├── atomic.rs                    # 原子操作
│   ├── memcpy.rs                    # 内存复制 (asm)
│   ├── memset.rs                    # 内存设置 (asm)
│   ├── delay.rs                     # 延迟函数
│   ├── stacktrace.rs                # 堆栈跟踪
│   ├── elf.rs                       # ELF 处理
│   ├── vcpu/
│   │   ├── mod.rs                   # VCPU 模块
│   │   ├── context.rs               # VCPU 上下文结构
│   │   ├── switch.S                 # 上下文切换汇编
│   │   ├── trap.rs                  # Trap 处理
│   │   ├── exception.rs             # 异常处理
│   │   ├── emulate.rs               # 指令仿真
│   │   ├── inject.rs                # 中断注入
│   │   ├── irq.rs                   # IRQ 处理
│   │   ├── mem.rs                   # 内存访问
│   │   ├── helper.rs                # 辅助函数
│   │   ├── coproc.rs                # 协处理器框架
│   │   └── ptrauth.rs               # 指针认证 (可选)
│   ├── sysreg/
│   │   ├── mod.rs                   # 系统寄存器模块
│   │   ├── trap.rs                  # 系统寄存器 trap
│   │   ├── dispatch.rs              # 寄存器访问分发
│   │   ├── state.rs                 # 保存的寄存器状态
│   │   ├── id_regs.rs               # ID 寄存器实现
│   │   ├── sctlr.rs                 # 系统控制寄存器
│   │   ├── mm.rs                    # MMU 相关寄存器
│   │   └── debug.rs                 # 调试寄存器
│   ├── fpu/
│   │   ├── mod.rs                   # FPU 模块
│   │   ├── vfp.rs                   # VFP 寄存器
│   │   ├── neon.rs                  # NEON/ASIMD
│   │   └── lazy.rs                  # Lazy FPU 切换
│   ├── wfi.rs                       # WFI 处理
│   └── wfe.rs                       # WFE 处理
├── mmu/
│   ├── mod.rs                       # MMU 模块
│   ├── stage2.rs                    # Stage-2 页表结构
│   ├── vttbr.rs                     # VTTBR_EL2 管理
│   ├── vtcr.rs                      # VTCR_EL2 配置
│   ├── operations.rs                # 页表操作
│   ├── translate.rs                 # 地址转换
│   ├── attrs.rs                     # 内存属性
│   ├── vmid.rs                      # VMID 管理
│   ├── gstage.rs                    # GStage trait 实现
│   └── fault.rs                     # Stage-2 缺页处理
├── interrupt/
│   ├── mod.rs                       # 中断模块
│   ├── vectors.S                    # 异常向量表
│   ├── handler.rs                   # 异常处理程序
│   ├── gic_discovery.rs             # GIC 发现
│   ├── virq.rs                      # 虚拟中断处理
│   └── vgic/
│       ├── mod.rs                   # VGIC 模块
│       ├── vgicv2.rs                # GICv2 虚拟化
│       ├── vgicv3.rs                # GICv3 虚拟化
│       └── routing.rs               # 中断路由
├── timer/
│   ├── mod.rs                       # Timer 模块
│   ├── vtimer.rs                    # 虚拟 Timer
│   └── htimer.rs                    # Hypervisor Timer
├── smp/
│   ├── mod.rs                       # SMP 模块
│   ├── init.rs                      # SMP 初始化
│   ├── spin_table.rs                # Spin Table 启动
│   ├── psci_boot.rs                 # PSCI 启动
│   ├── scu.rs                       # SCU 支持
│   └── hotplug.rs                   # CPU 热插拔
├── psci/
│   ├── mod.rs                       # PSCI 模块
│   ├── smccc.rs                     # SMC 调用约定
│   └── cpu_state.rs                 # CPU 状态管理
├── devtree/
│   ├── mod.rs                       # 设备树模块
│   ├── parse.rs                     # 设备树解析
│   └── vm_fdt.rs                    # 虚拟设备树生成
├── platform/
│   ├── mod.rs                       # 平台模块
│   ├── qemu_virt.rs                 # QEMU virt 平台
│   ├── foundation_v8.rs             # Foundation v8 模型
│   ├── rpi4.rs                      # Raspberry Pi 4
│   └── rk3399.rs                    # Rockchip RK3399
├── locks/
│   └── mod.rs                       # ARM 锁实现
└── mod.rs                           # ARM64 架构模块

arch/arm32/                           # ARMv7-A 32位支持 (可选)
├── cpu/
│   ├── entry.S                      # HYP 入口
│   ├── init.rs                      # CPU 初始化
│   ├── ... (结构与 arm64 类似)
│   ├── coproc/
│   │   ├── mod.rs                   # 协处理器模块
│   │   ├── cp15.rs                  # CP15 协处理器 (653行)
│   │   └── cp14.rs                  # CP14 调试协处理器 (218行)
│   └── ...
└── ... (其余结构与 arm64 类似)

drivers/irqchip/
├── mod.rs
├── arm_gicv2.rs                     # GICv2 驱动
└── arm_gicv3.rs                     # GICv3 驱动

drivers/timer/
├── mod.rs
└── arm_generic_timer.rs             # ARM Generic Timer 驱动

include/arm64/                       # ARM64 公共头文件
├── arch_regs.h                      # 寄存器结构定义
├── cpu_defines.h                    # CPU 常量
├── arch_barrier.h                   # 内存屏障
├── arch_cache.h                     # 缓存操作
├── arch_mmu.h                       # MMU 定义
├── arch_gicv3.h                     # GICv3 定义
├── arm_features.h                   # ARM 特性枚举
└── psci.h                           # PSCI 定义

include/arm32/                       # ARM32 公共头文件
└── ... (类似 arm64)

scripts/
├── build-arm.sh                     # ARM 交叉编译脚本
└── run-qemu-arm.sh                  # QEMU ARM 启动脚本
```

---

## 六、Xvisor 关键文件详细映射

### 6.1 ARM64 CPU 文件映射

| Xvisor 文件 | 行数 | Ferrovisor 对应文件 | 优先级 |
|-------------|------|-------------------|--------|
| cpu_init.c | 112 | cpu/init.rs | P0 |
| cpu_entry.S | ~100 | cpu/entry.S | P0 |
| cpu_vcpu_helper.c | 899 | cpu/vcpu/helper.rs | P0 |
| cpu_vcpu_switch.S | ~200 | cpu/vcpu/switch.S | P0 |
| cpu_vcpu_excep.c | 187 | cpu/vcpu/exception.rs | P0 |
| cpu_vcpu_emulate.c | 613 | cpu/vcpu/emulate.rs | P1 |
| cpu_vcpu_inject.c | 291 | cpu/vcpu/inject.rs | P0 |
| cpu_vcpu_irq.c | 217 | cpu/vcpu/irq.rs | P0 |
| cpu_vcpu_sysregs.c | 464 | cpu/sysreg/*.rs | P0 |
| cpu_vcpu_vfp.c | 156 | cpu/fpu/*.rs | P1 |
| cpu_vcpu_coproc.c | 288 | cpu/vcpu/coproc.rs | P1 |
| cpu_vcpu_mem.c | 173 | cpu/vcpu/mem.rs | P1 |
| cpu_vcpu_ptrauth.c | 110 | cpu/vcpu/ptrauth.rs | P2 |
| cpu_interrupts.c | 246 | interrupt/handler.rs | P0 |
| cpu_cache.S | ~150 | cpu/cache.rs | P1 |
| cpu_atomic.c | 140 | cpu/atomic.rs | P1 |
| cpu_atomic64.c | 141 | cpu/atomic64.rs | P1 |
| cpu_locks.c | 194 | cpu/locks.rs | P1 |
| cpu_stacktrace.c | 125 | cpu/stacktrace.rs | P2 |
| cpu_elf.c | 66 | cpu/elf.rs | P2 |

**优先级说明:**
- P0: 核心功能，必须实现
- P1: 重要功能，应该实现
- P2: 可选功能

### 6.2 ARM32 CPU 文件映射

| Xvisor 文件 | 行数 | Ferrovisor 对应文件 | 优先级 |
|-------------|------|-------------------|--------|
| cpu_init.c | 113 | cpu/init.rs | P0 |
| cpu_vcpu_helper.c | 1094 | cpu/vcpu/helper.rs | P0 |
| cpu_vcpu_cp15.c | 653 | cpu/coproc/cp15.rs | P0 |
| cpu_vcpu_emulate.c | 564 | cpu/vcpu/emulate.rs | P1 |
| cpu_vcpu_excep.c | 184 | cpu/vcpu/exception.rs | P0 |
| cpu_vcpu_vfp.c | 193 | cpu/fpu/*.rs | P1 |
| cpu_vcpu_coproc.c | 320 | cpu/coproc/mod.rs | P0 |
| cpu_vcpu_cp14.c | 218 | cpu/coproc/cp14.rs | P1 |
| cpu_interrupts.c | 268 | interrupt/handler.rs | P0 |

### 6.3 ARM Common 文件映射

| Xvisor 文件 | 大小 | Ferrovisor 对应文件 | 优先级 |
|-------------|------|-------------------|--------|
| mmu_lpae.c | 397行 | mmu/stage2.rs | P0 |
| vgic.c | ~40KB | interrupt/vgic/*.rs | P0 |
| vgic_v2.c | ~7.7KB | interrupt/vgic/vgicv2.rs | P0 |
| vgic_v3.c | ~11.7KB | interrupt/vgic/vgicv3.rs | P0 |
| emulate_arm.c | ~105KB | cpu/vcpu/emulate.rs | P1 |
| emulate_psci.c | 8.7KB | psci/*.rs | P0 |
| arm_psci.c | 7.4KB | psci/*.rs | P0 |
| generic_timer.c | 16.7KB | timer/*.rs | P0 |
| smp_ops.c | 9.7KB | smp/*.rs | P0 |
| smp_psci.c | ~2KB | smp/psci_boot.rs | P0 |
| smp_spin_table.c | ~3.9KB | smp/spin_table.rs | P0 |
| smp_scu.c | 5.2KB | smp/scu.rs | P1 |
| smp_imx.c | 5.6KB | smp/imx.rs | P2 |

### 6.4 板级支持文件映射

| Xvisor 文件 | Ferrovisor 对应文件 | 优先级 |
|-------------|-------------------|--------|
| board/generic/foundation-v8.c | platform/foundation_v8.rs | P0 |
| board/generic/vexpress.c | platform/vexpress.rs | P1 |
| board/generic/bcm2836.c | platform/rpi2.rs | P1 |
| board/generic/rk3399.c | platform/rk3399.rs | P1 |

---

## 七、风险评估

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| **硬件可用性** | ARM 开发板获取成本高 | 中 | 优先使用 QEMU virt 平台 |
| **调试复杂度** | 汇编代码和异常处理难调试 | 高 | 充分的单元测试，使用 JTAG |
| **时间估计** | 实际时间可能超出计划 | 中 | 迭代开发，优先实现核心功能 |
| **代码复用** | Rust ARM 生态不如 C | 中 | 参考现有 Rust OS 项目 (Theseus, Rust-OSdev) |
| **文档缺失** | ARM 架构文档分散 | 低 | 参考 ARM ARM 和特权架构手册 |
| **兼容性** | 不同 ARM 芯片差异大 | 中 | 优先支持主流平台 (QEMU, 树莓派) |
| **CP15 复杂度** | 协处理器仿真复杂 | 中 | 逐步实现，优先支持常用寄存器 |
| **VGIC 复杂度** | GICv2/v3 差异大 | 中 | 分别实现，先完成 GICv2 |

---

## 八、参考资料

### 8.1 ARM 官方文档

| 文档 | 说明 | 来源 |
|------|------|------|
| ARM DDI 0487 | ARMv8-A Architecture Reference Manual | ARM 官方 |
| ARM DDI 0406 | ARMv7-A Architecture Reference Manual | ARM 官方 |
| ARM IHI 0069 | Generic Interrupt Controller spec | ARM 官方 |
| ARM DEN 0028 | SMC Calling Convention | ARM 官方 |
| ARM DEN 0022 | Power State Coordination Interface | ARM 官方 |

### 8.2 开源项目参考

| 项目 | URL | 说明 |
|------|-----|------|
| Xvisor | /home/zcxggmu/workspace/hello-projs/posp/xvisor | 主要参考项目 |
| KVM ARM | Linux kernel virt/kvm/arm | Linux KVM ARM 实现 |
| Xen ARM | https://xenproject.org | Xen ARM 支持 |
| Theseus OS | https://github.com/theseus-os/Theseus | Rust OS |
| Oreboot | https://github.com/oreboot/oreboot | Rust 固件 |

### 8.3 Rust ARM 资源

| 资源 | 说明 |
|------|------|
| Rust-OSdev Wiki | Rust OS 开发指南 |
| cortex-a | ARM Cortex-A 寄存器访问 crate |
| aarch64-paging | ARMv8 页表管理 crate |
| armv8-a | ARMv8-A 定义和类型 |

---

## 九、里程碑

| 里程碑 | 目标 | 时间 | 验收标准 |
|--------|------|------|----------|
| M0 | ARM 环境搭建完成 | Week 2 | 可交叉编译，QEMU 启动 |
| M1 | 基本 CPU 初始化和 EL2/HYP 进入 | Week 6 | 进入 EL2/HYP 模式 |
| M2 | VCPU 上下文切换 | Week 8 | 可切换 VCPU |
| M3 | Stage-2 页表和地址转换 | Week 10 | Guest 可访问内存 |
| M4 | VGIC 中断虚拟化 | Week 14 | Guest 可接收中断 |
| M5 | 系统寄存器仿真 | Week 18 | Guest 可读写系统寄存器 |
| M6 | FPU/SIMD 支持 | Week 20 | Guest 可使用 FPU |
| M7 | PSCI 和 SMP | Week 24 | 多核运行 |
| M8 | Timer 虚拟化 | Week 26 | Guest Timer 工作 |
| M9 | 平台支持和设备树 | Week 28 | 可在 QEMU 启动 |
| M10 | 完整测试和文档 | Week 32 | 可运行 Linux Guest |

---

## 十、总结

### 10.1 关键数据

**Xvisor ARM 支持规模：**
- 总文件数: 140+
- 总代码量: ~31,000 行
- ARM64 CPU: ~4,422 行
- ARMv7 CPU: ~4,780 行
- ARM Common: ~15,000 行

**Ferrovisor 需要实现：**
- 核心文件: 90+ 个
- 估计代码量: ~26,000 行
- 预计时间: 32 周

### 10.2 关键原则

1. **参考 Xvisor**: 充分利用 Xvisor 的成熟实现
2. **迭代开发**: 优先实现核心功能 (P0)，逐步完善
3. **模块化设计**: 与 RISC-V 代码共享抽象接口
4. **测试驱动**: 每个阶段都有明确的测试目标
5. **文档先行**: 充分利用 ARM 官方文档和 Xvisor 源码

### 10.3 预期成果

- 32 周后 Ferrovisor 将具备与 Xvisor 相当的 ARM64 虚拟化能力
- 支持 QEMU virt 平台和部分 ARM 开发板
- 能够运行 Linux Guest 操作系统
- 支持 SMP 多核
- 完整的设备树和平台支持

---

*本计划将根据实际开发进度和需求变化进行动态调整。*

*版本历史:*
- *v1.0 (2025-12-27): 初始版本*
- *v2.0 (2025-12-27): 深度优化版，添加完整的 Xvisor 目录结构和文件映射*
