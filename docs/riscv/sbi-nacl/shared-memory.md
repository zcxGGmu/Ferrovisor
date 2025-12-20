# SBI 嵌套加速共享内存接口

## 概述

SBI 嵌套加速扩展（NACL）通过共享内存机制在 L0 虚拟机监控器和 L1 虚拟机监控器之间提供高效的通信接口。这个共享内存区域用于传递嵌套虚拟化的相关数据，减少了传统的 SBI 调用开销。

## 内存布局

### 整体结构

嵌套加速共享内存的大小和布局是固定的：

```
总大小 = 4096 + 1024 * (XLEN / 8) 字节
┌─────────────────────────────────────────────────────────┐
│                  总体结构                              │
├─────────────────────────────────────────────────────────┤
│  Scratch 空间 (4096 字节)                              │
├─────────────┬───────────────────────────────────────────┤
│   CSR 空间  │           扩展空间                     │
└─────────────┴───────────────────────────────────────────┘
  1024*XLEN    1024*XLEN - 4096 字节
  位           字节
```

### Scratch 空间 (0x00000000 - 0x00000FFF)

这个区域用于存储嵌套加速功能特定的数据结构：

```
偏移     | 大小    | 用途
---------|--------|--------------------
0x0000   | 512    | 嵌套 SRET 上下文 (Nested SRET Context)
0x0200   | 128    | 嵌套自动交换上下文 (Nested Autoswap CSR Context)
0x0280   | 128    | 保留 (Reserved)
0x0300   | 1792   | 嵌套 HFENCE 条目 (Nested HFENCE Entries)
0x0F80   | 128    | 嵌套 CSR 脏位图 (Nested CSR Dirty Bitmap)
0x1000   | 3072   | 保留 (Reserved)
```

### CSR 空间 (0x00001000 - 0x00001FFF)

这个区域是一个数组，用于存储 RISC-V H 扩展的 CSR 值：

```c
#define NACL_CSR_OFFSET  0x00001000
#define NACL_CSR_COUNT  1024  // 1024 个 XLEN 位字

struct nacl_csr_array {
    unsigned long csrs[NACL_CSR_COUNT];  // 1024 个 CSR 值
};
```

## CSR 映射

### H 扩展 CSR 列表

| 索引 | CSR 名称 | 地址偏移 | 描述 |
|------|----------|----------|------|
| 0x600 | `ustatus` | 0x00001800 | 虚拟用户状态 |
| 0x602 | `vsstatus` | 0x00002000 | 虚拟监管器状态 |
| 0x603 | `uie` | 0x00002400 | 用户中断使能 |
| 0x604 | `ueip` | 0x00002404 | 用户中断挂起 |
| 0x605 | `sie` | 00002804 | 监管器中断使能 |
| 0x606 | `sip` | 0x00002844 | 监管器中断挂起 |
| 0x607 | `vstvec` | 0x0000205 | 虚拟陷阱向量基址 |
| 0x608 | `vsscratch` | 0x0000400 | 虚拟暂存寄存器 |
| 0x609 | `vsepc` | 0x0002414 | 虚拟异常程序计数器 |
| 0x60A | `vscause` | 0x2424 | 虚拟异常原因 |
| 0x60B | `vtval` | 00002434 | 虚拟陷阱值 |
| 0x60C | `vsatp` | 0x0280 | 虚拟地址转换和保护 |
| 0x60D | `hstatus` | 0x0600 | 虚拟机监控器状态 |
| 0x60E | `hideleg` | 0x603 | 异常委托 |
| 0x60F | `hie` | 0x604 | 中断使能 |
| 0x610 | `hvip` | 0x614 | 虚拟中断挂起 |
| 0x611 | `hip` | 0x644 | 中断挂起 |
| 0x612 | `hgeie` | 0x612 | 客户机外部中断使能 |
| 0x613 | `hgeip` | 0x614 | 客户机外部中断挂起 |
| 0x614 | `henvcfg` | 0x60A | 环境配置 |
| 0x615 | `hcounteren` | 0x606 | 计数器使能 |
| 0x618 | `htval` | 0x643 | 陷阱值 |
| 0x619 | `htinst` | 0x645 | 陷阱指令 |
| 0x61A | `hgatp` | 0x680 | 客户机地址转换和保护 |

## 访问权限

### L1 虚拟机监控器权限

- **读取**：可以读取所有 CSR 值
- **写入**：可以写入所有 CSR 值，但需要通过脏位图通知 L0
- **缓存一致性**：写入后需要调用同步函数确保 L0 知晓更改

### L0 虚拟机监控器权限

- **读取**：L0 可以随时读取共享内存
- **验证**：需要验证 L1 写入的合法性
- **执行**：负责实际执行需要模拟的操作

## 内存一致性

### 同步机制

为确保内存一致性，NACL 提供了专门的同步函数：

1. `sbi_nacl_sync_csr()`：同步 CSR 值
2. `sbi_nacl_sync_hfence()`：同步 HFENCE 操作
3. `sbi_nacl_sync_sret()`：同步所有待处理操作并模拟 SRET

### 脏位图

128 字节的脏位图用于跟踪哪些 CSR 被修改：

```c
#define NACL_CSR_BITMAP_SIZE 128  // 128 字节 = 1024 位

struct nacl_csr_bitmap {
    uint8_t bitmap[NACL_CSR_BITMAP_SIZE];
};
```

- 每一位对应一个 CSR
- 位为 1 表示该 CSR 已被修改
- 位为 0 表示 CSR 未被修改

### 初始化和清理

```c
// 设置共享内存
struct sbiret ret = sbi_nacl_set_shmem(shmem_lo, shmem_hi, flags);
if (ret.error != SBI_SUCCESS) {
    // 处理错误
}

// 清零脏位图
memset(&nacl->csr_bitmap, 0, sizeof(nacl->csr_bitmap));
```

## 地址转换

### 物理地址映射

```
物理地址 = 共享内存物理基址 + 偏移
虚拟地址 = 0x00000000 + 偏移
```

### 访问宏

```c
// CSR 访问宏
#define NACL_CSR(csr_index) \
    (*((volatile unsigned long *)(nacl_base + NACL_CSR_OFFSET + \
    (csr_index) * sizeof(unsigned long)))

// Scratch 空间访问宏
#define NACL_SCRATCH(offset) \
    (*((volatile unsigned long *)(nacl_base + (offset)))

// 脏位图访问宏
#define NACL_BITMAP() \
    ((volatile uint8_t *)(nacl_base + 0x0F80))
```

## 错误处理

### 常见错误

1. **地址未对齐**：物理地址未按页对齐
2. **大小不匹配**：共享内存大小不符合要求
3. **权限不足**：L1 尝试访问未分配的内存区域
4. **同步失败**：同步操作因各种原因失败

### 错误代码

所有 NACL 函数都返回 `sbiret` 结构，包含：
- `error`：错误码
- `value`：返回值（如果有）

### 错误恢复

```c
if (ret.error != SBI_SUCCESS) {
    switch (ret.error) {
        case SBI_ERR_INVALID_PARAM:
            // 处理无效参数错误
            break;
        case SBI_ERR_DENIED:
            // 处理权限被拒绝错误
            break;
        case SBI_ERR_INVALID_ADDRESS:
            // 处理无效地址错误
            break;
        default:
            // 处理其他错误
            break;
    }
}
```

## 性能优化

### 批量操作

- **CSR 批量写入**：一次写入多个 CSR，设置多个脏位
- **HFENCE 批量处理**：一次处理多个 HFENCE 操作
- **延迟同步**：延迟非关键的同步操作

### 缓存优化

- **局部缓存**：在本地缓存常用的 CSR 值
- **批量读取**：一次性读取多个 CSR 值
- **预测性加载**：预加载可能需要访问的 CSR

### 内存布局优化

- **对齐访问**：确保所有访问都是对齐的
- **局部性原理**：将相关的数据结构放在一起
- **缓存友好**：考虑 CPU 缓存行大小