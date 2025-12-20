# RISC-V SBI 固件特性扩展 (FWFT) 概述

## 简介

RISC-V SBI 固件特性扩展（Firmware Features, FWFT）为监督模式（supervisor-mode）软件提供了一个标准接口，用于管理和控制特定的硬件能力或 SBI 实现特性。该扩展标识符为 0x46574654（"FWFT"），在 SBI 3.0 版本中引入。

## 核心概念

### 扩展目的

FWFT 扩展的主要目的是：
- **特性管理**：允许监督模式软件动态查询和配置硬件特性
- **标准化接口**：提供跨平台的统一特性管理接口
- **灵活控制**：支持根据应用需求启用或禁用特定功能
- **安全增强**：为安全特性提供配置机制

### 关键术语

- **特性（Feature）**：一个可配置的硬件或固件功能
- **本地特性**：每个 hart（硬件线程）独立配置的特性
- **全局特性**：影响整个系统的特性，只需配置一次
- **特性值**：特性的具体配置参数
- **锁定特性**：设置后不能再修改的特性

## 功能特性分类

### 标准特性

FWFT 定义了 6 个标准特性：

1. **MISALIGNED_EXC_DELEG (0x00000000)**
   - 控制非对齐访问异常委托
   - 作用域：本地

2. **LANDING_PAD (0x00000001)**
   - 控制监督模式的着陆垫支持
   - 作用域：本地

3. **SHADOW_STACK (0x00000002)**
   - 控制监督模式的影子栈支持
   - 作用域：本地

4. **DOUBLE_TRAP (0x00000003)**
   - 控制双重陷阱支持
   - 作用域：本地

5. **PTE_AD_HW_UPDATING (0x00000004)**
   - 控制页表项 A/D 位的硬件更新
   - 作用域：本地

6. **POINTER_MASKING_PMLEN (0x00000005)**
   - 控制监督模式的指针掩码长度
   - 作用域：本地

### 平台特定特性

保留 0x40000000 - 0x7FFFFFFF 范围给平台特定的特性定义，允许平台厂商添加自定义功能。

## 接口概述

### 核心函数

FWFT 扩展提供两个核心函数：

1. **sbi_fwft_set()** - 设置特性值
2. **sbi_fwft_get()** - 查询特性值

### 使用模式

```c
// 查询特性是否支持
struct sbiret ret = sbi_fwft_get(SHADOW_STACK);
if (ret.error == SBI_SUCCESS) {
    printf("Shadow stack supported, value: %lu\n", ret.value);
}

// 设置特性
ret = sbi_fwft_set(SHADOW_STACK, 1, 0);
if (ret.error == SBI_SUCCESS) {
    printf("Shadow stack enabled\n");
}
```

## 技术背景

### RISC-V 特性管理

在 RISC-V 系统中，许多硬件特性需要在启动时配置，传统的做法包括：
- 通过设备树（Device Tree）配置
- 通过平台特定的寄存器
- 通过编译时选项

FWFT 提供了一个运行时的标准接口，使配置更加灵活。

### SBI 架构集成

FWFT 扩展完全集成在 SBI 架构中：
- 遵循 SBI 调用约定
- 支持标准的错误处理机制
- 与其他 SBI 扩展协同工作

## 应用场景

### 1. 虚拟化环境

在虚拟化场景中，Hypervisor 可以：
- 为不同的虚拟机配置不同的特性集
- 动态调整虚拟机的硬件能力
- 实现细粒度的权限控制

### 2. 安全增强

安全相关应用可以利用 FWFT：
- 根据安全需求启用影子栈
- 动态调整指针掩码长度
- 实现运行时的安全策略

### 3. 性能优化

性能敏感的应用可以：
- 启用硬件加速特性
- 禁用不必要的功能以减少开销
- 根据工作负载调整系统配置

### 4. 调试和测试

开发和测试场景下：
- 禁用某些特性以简化调试
- 测试软件在不同配置下的行为
- 验证特性间的交互

## 设计原则

### 向后兼容

- 新特性不影响现有功能
- 老版本软件在新硬件上正常运行
- 特性查询机制平滑降级

### 权限控制

- SBI 实现可以拒绝不安全的配置
- 支持虚拟化环境的隔离需求
- 提供特性锁定机制

### 可扩展性

- 为平台特定特性保留空间
- 支持未来特性的扩展
- 版本化机制支持演进

## 实现要求

### 硬件要求

- 需要相应的硬件支持
- 特性能力可能因平台而异
- 需要验证硬件兼容性

### 软件要求

- SBI 实现 3.0 或更高版本
- 支持 FWFT 扩展
- 正确的错误处理

### 依赖关系

- 某些特性可能依赖其他特性
- 需要处理特性间的交互
- 维护系统一致性

## 安全考虑

### 特性隔离

- 本地特性不会影响其他 hart
- 全局特性需要特殊权限
- 虚拟化环境中的隔离保证

### 锁定机制

- 防止意外修改关键特性
- 系统启动后的配置保护
- 安全策略的强制执行

## 性能影响

### 配置开销

- 特性查询通常只需要一次
- 设置操作的开销很小
- 缓存机制减少重复查询

### 运行时影响

- 特性启用可能影响性能
- 硬件加速通常提升性能
- 需要权衡功能和性能

## 相关规范

- [RISC-V SBI 规范](https://github.com/riscv-non-isa/riscv-sbi-doc)
- [RISC-V 特权架构规范](https://github.com/riscv/riscv-isa-manual)
- [Zicfiss 影子栈规范](https://github.com/riscv/riscv-isa-manual)
- [Zicfilp 着陆垫规范](https://github.com/riscv/riscv-isa-manual)

## 参考资料

- [FWFT 扩展规范](https://github.com/riscv-non-isa/riscv-sbi-doc/blob/master/src/ext-firmware-features.adoc)
- [SBI 3.0 规范](https://github.com/riscv-non-isa/riscv-sbi-doc)
- [RISC-V 国际标准](https://riscv.org/technical/specifications/)