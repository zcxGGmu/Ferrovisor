# RISC-V 虚拟机监控器扩展文档总结

本文档提供了 RISC-V 虚拟机监控器扩展（H扩展）文档的全面概述。

## 文档结构

```
docs/riscv/
├── README.md           # RISC-V 虚拟化概述
├── README-zh.md        # RISC-V 虚拟化概述（中文）
├── csrs.md             # 控制和状态寄存器
├── csrs-zh.md          # 控制和状态寄存器（中文）
├── address-translation.md # 两级地址转换
├── address-translation-zh.md # 两级地址转换（中文）
├── instructions.md     # 虚拟化指令
├── instructions-zh.md  # 虚拟化指令（中文）
├── exceptions.md       # 陷阱和异常处理
├── exceptions-zh.md    # 陷阱和异常处理（中文）
├── SUMMARY.md          # 英文总结
└── SUMMARY-zh.md       # 本总结
```

## 快速参考

### 核心概念

| 概念 | 描述 | 关键寄存器/指令 |
|------|------|----------------|
| 虚拟化模式 | V=0（非虚拟化）或 V=1（虚拟化） | `hstatus`、`mstatus` |
| 特权模式 | M/HS/U（V=0），VS/VU（V=1） | CSR 访问规则 |
| 两级地址转换 | GVA→GPA→SPA | `vsatp`、`hgatp` |
| VMID | 虚拟机标识符 | `hgatp.VMID` |

### 必需的 CSR

| CSR | 模式 | 目的 |
|-----|------|---------|
| `hstatus` | HS | 虚拟机监控器状态和控制 |
| `hedeleg`/`hideleg` | HS | 异常/中断委托 |
| `hgatp` | HS | G 级地址转换 |
| `vsstatus` | VS | 虚拟监管器状态 |
| `hvip`/`hip`/`hie` | HS | 虚拟中断管理 |

### 关键指令

| 指令 | 目的 | 有效模式 |
|-------------|---------|-------------|
| `HLV.*`/`HSV.*` | 虚拟内存访问 | M/HS/U（如果 HU=1） |
| `HFENCE.VVMA` | VS 级 TLB 栅栏 | M/HS |
| `HFENCE.GVMA` | G 级 TLB 栅栏 | M/HS |
| `SRET` | 从监管器陷阱返回 | 所有（行为变化） |

## 实现检查清单

### 最低要求
- [ ] 基础 ISA RV32I 或 RV64I
- [ ] 基于页面的虚拟内存（RV32 为 Sv32，RV64 为 Sv39+）
- [ ] 非只读 `mtval` CSR
- [ ] 在 `misa` 中设置第 7 位以启用 H 扩展

### HS 模式支持
- [ ] 实现所有 HS 模式 CSR
- [ ] 支持两级地址转换
- [ ] 处理 VS 级中断
- [ ] 实现虚拟内存访问指令

### VS 模式支持
- [ ] 实现 VS CSR 副本
- [ ] 支持客户机物理地址转换
- [ ] 处理虚拟指令异常
- [ ] 如需要，支持嵌套虚拟化

## 性能考虑

### TLB 管理
- 实现 VMID 标记以实现隔离
- 支持选择性失效
- 为多 VM 工作负载考虑 TLB 大小

### 页表结构
- G 级使用 16 KiB 根页表
- 客户机物理地址扩展 2 位
- 高效的页表遍历硬件

### 中断处理
- 通过 `hvip` 快速中断注入
- 客户机外部中断路由
- 最小化中断延迟

## 安全特性

### 隔离
- 硬件强制内存隔离
- 基于 VMID 的地址空间隔离
- 特权级别分离

### 保护
- 两级权限检查
- 与物理内存保护集成
- 与安全启动支持兼容

## 兼容性

### 软件兼容性
- 标准监管模式操作系统无修改运行
- 具有 H 扩展特性的 HS 模式操作系统
- 应用程序透明地运行在 VS 模式客户机中

### 硬件兼容性
- 可以在没有 H 扩展的硬件上模拟
- 没有 H 扩展时优雅降级
- 支持嵌套虚拟化

## 推荐的扩展

### 推荐的
- **Svadu**：自动 A/D 位管理
- **Svpbmt**：内存类型属性
- **Zicfilp**：着陆点预测
- **Ssdbltrp**：双重陷阱检测

### 可选的
- **Zicfiss**：影子栈支持
- **Sstc**：定时器计数器委托
- **Sscofpmf**：性能监控

## 调试支持

### 调试 CSR
- 转换的指令编码
- 客户机物理地址报告
- 虚拟化模式跟踪

### 异常信息
- `htval` 中的详细故障信息
- `htinst` 中的指令转换
- 状态位中的虚拟化模式

## 从经典虚拟化迁移

### 优势
- 减少 VM 退出
- 硬件辅助内存管理
- 标准化的虚拟化接口
- 更好的性能隔离

### 需要的更改
- 更新虚拟机监控器以使用 H 扩展 CSR
- 修改客户机退出处理
- 实现新的陷阱委托方案
- 为两级地址转换更新内存管理

## 参考实现

### 开源项目
- [QEMU](https://www.qemu.org/) - 模拟支持
- [KVM](https://www.kernel.org/doc/html/latest/virt/kvm/) - 基于内核的 VM
- [FireMarshal](https://github.com/firemarshal/firemarshal) - RISC-V 虚拟机监控器框架

### 文档链接
- [RISC-V ISA 手册](https://github.com/riscv/riscv-isa-manual)
- [RISC-V 特权规范](https://github.com/riscv/riscv-isa-manual/blob/main/src/privileged.adoc)
- [RISC-V 虚拟化规范](https://github.com/riscv/riscv-isa-manual/blob/main/src/hypervisor.adoc)

## 中英文文档对应关系

| 英文文档 | 中文文档 | 说明 |
|-----------|-----------|------|
| README.md | README-zh.md | 虚拟化概述 |
| csrs.md | csrs-zh.md | 控制状态寄存器 |
| address-translation.md | address-translation-zh.md | 两级地址转换 |
| instructions.md | instructions-zh.md | 虚拟化指令 |
| exceptions.md | exceptions-zh.md | 陷阱和异常处理 |
| SUMMARY.md | SUMMARY-zh.md | 总结和快速参考 |