# SBI 嵌套加速函数接口文档

本文档提供了 RISC-V SBI 嵌套加速扩展（NACL）所有函数的完整 API 参考。

## 扩展标识符

```
SBI_EXT_NACL = 0x0A000000
```

## 函数列表

### 1. sbi_nacl_set_shmem

设置 L0 和 L1 虚拟机监控器之间的共享内存区域。

**函数 ID**: 0x0

**C 原型**:
```c
struct sbiret sbi_nacl_set_shmem(unsigned long shmem_lo,
                                  unsigned long shmem_hi,
                                  unsigned long flags);
```

**参数**:
- `shmem_lo`: 共享内存物理地址低 32 位
- `shmem_hi`: 共享内存物理地址高 32 位（仅 RV64）
- `flags`: 控制标志

**标志位定义**:
```c
#define NACL_SHMEM_FLAG_ENABLE    (1UL << 0)  // 启用共享内存
#define NACL_SHMEM_FLAG_CLEAR     (1UL << 1)  // 清零内存内容
#define NACL_SHMEM_FLAG_RESERVE   (1UL << 2)  // 预留内存区域
#define NACL_SHMEM_FLAG_READONLY  (1UL << 3)  // 只读模式
```

**返回值**:
- 成功: `error = SBI_SUCCESS`, `value = 0`
- 失败:
  - `error = SBI_ERR_INVALID_PARAM`: 参数无效
  - `error = SBI_ERR_INVALID_ADDRESS`: 地址无效或未对齐
  - `error = SBI_ERR_DENIED`: 权限被拒绝
  - `error = SBI_ERR_NO_MEM`: 内存不足

**示例**:
```c
// 设置 4KB 共享内存
struct sbiret ret = sbi_nacl_set_shmem(shmem_phys_addr, 0,
                                       NACL_SHMEM_FLAG_ENABLE |
                                       NACL_SHMEM_FLAG_CLEAR);
if (ret.error != SBI_SUCCESS) {
    printf("Failed to set shared memory: %ld\n", ret.error);
}
```

### 2. sbi_nacl_sync_csr

同步共享内存中的 CSR 值到 L0 虚拟机监控器。

**函数 ID**: 0x1

**C 原型**:
```c
struct sbiret sbi_nacl_sync_csr(unsigned long csr_bitmap_lo,
                                 unsigned long csr_bitmap_hi);
```

**参数**:
- `csr_bitmap_lo`: CSR 位图低 64 位
- `csr_bitmap_hi`: CSR 位图高 64 位（仅 RV64）

**位图说明**:
- 每位对应一个 CSR
- 位为 1 表示该 CSR 需要同步
- 全 0 表示同步所有修改的 CSR

**返回值**:
- 成功: `error = SBI_SUCCESS`, `value = 同步的 CSR 数量`
- 失败:
  - `error = SBI_ERR_INVALID_PARAM`: 无效的 CSR 编号
  - `error = SBI_ERR_DENIED`: 无权限访问该 CSR
  - `error = SBI_ERR_IO_ERROR`: 同步过程中的 I/O 错误

**示例**:
```c
// 同步 vsstatus 和 vstvec CSR
unsigned long bitmap = (1UL << 0) | (1UL << 1);  // 假设 vsstatus=0, vstvec=1
struct sbiret ret = sbi_nacl_sync_csr(bitmap, 0);
if (ret.error == SBI_SUCCESS) {
    printf("Synced %ld CSRs\n", ret.value);
}
```

### 3. sbi_nacl_hfence_vvma

执行 VS 级虚拟内存的 TLB 栅栏操作。

**函数 ID**: 0x2

**C 原型**:
```c
struct sbiret sbi_nacl_hfence_vvma(unsigned long addr,
                                    unsigned long asid,
                                    unsigned long vmid,
                                    unsigned long size);
```

**参数**:
- `addr`: 客户机虚拟地址（0 表示所有地址）
- `asid`: 地址空间标识符（0 表示所有 ASID）
- `vmid`: 虚拟机标识符（0 表示当前 VMID）
- `size`: 地址范围大小（0 表示单个地址）

**返回值**:
- 成功: `error = SBI_SUCCESS`, `value = 0`
- 失败:
  - `error = SBI_ERR_INVALID_PARAM`: 参数无效
  - `error = SBI_ERR_INVALID_ADDRESS`: 无效地址
  - `error = SBI_ERR_DENIED`: 权限不足

**示例**:
```c
// 失效整个 VS 级 TLB
struct sbiret ret = sbi_nacl_hfence_vvma(0, 0, 0, 0);

// 失效特定地址范围
ret = sbi_nacl_hfence_vvma(0x1000, 0, current_vmid, 0x1000);
```

### 4. sbi_nacl_hfence_gvma

执行 G 级客户机物理内存的 TLB 栅栏操作。

**函数 ID**: 0x3

**C 原型**:
```c
struct sbiret sbi_nacl_hfence_gvma(unsigned long gpa,
                                    unsigned long vmid);
```

**参数**:
- `gpa`: 客户机物理地址，右移 2 位（0 表示所有地址）
- `vmid`: 虚拟机标识符（0 表示所有 VMID）

**返回值**:
- 成功: `error = SBI_SUCCESS`, `value = 0`
- 失败:
  - `error = SBI_ERR_INVALID_PARAM`: 参数无效
  - `error = SBI_ERR_DENIED`: 权限不足

**示例**:
```c
// 失效所有 G 级 TLB 条目
struct sbiret ret = sbi_nacl_hfence_gvma(0, 0);

// 失效特定 GPA 的 TLB 条目
ret = sbi_nacl_hfence_gvma(gpa >> 2, current_vmid);
```

### 5. sbi_nacl_sret

模拟嵌套环境下的 SRET 指令执行。

**函数 ID**: 0x4

**C 原型**:
```c
struct sbiret sbi_nacl_sret(unsigned long flags);
```

**参数**:
- `flags`: 控制标志

**标志位定义**:
```c
#define NACL_SRET_FLAG_AUTO_SWAP    (1UL << 0)  // 自动交换 CSR
#define NACL_SRET_FLAG_SAVE_STATE   (1UL << 1)  // 保存状态
#define NACL_SRET_FLAG_INJECT_IRQ   (1UL << 2)  // 注入中断
#define NACL_SRET_FLAG_DEBUG        (1UL << 3)  // 调试模式
```

**返回值**:
- 成功: `error = SBI_SUCCESS`, `value = 0`
- 失败:
  - `error = SBI_ERR_INVALID_PARAM`: 标志无效
  - `error = SBI_ERR_DENIED`: 权限不足
  - `error = SBI_ERR_INVALID_STATE`: 状态无效

**示例**:
```c
// 执行带自动交换的 SRET
struct sbiret ret = sbi_nacl_sret(NACL_SRET_FLAG_AUTO_SWAP);
if (ret.error != SBI_SUCCESS) {
    printf("SRET failed: %ld\n", ret.error);
}
```

### 6. sbi_nacl_csr_read

读取指定的 CSR 值。

**函数 ID**: 0x5

**C 原型**:
```c
struct sbiret sbi_nacl_csr_read(unsigned long csr_num);
```

**参数**:
- `csr_num`: CSR 编号

**返回值**:
- 成功: `error = SBI_SUCCESS`, `value = CSR 值`
- 失败:
  - `error = SBI_ERR_INVALID_PARAM`: CSR 编号无效
  - `error = SBI_ERR_DENIED`: 无读取权限
  - `error = SBI_ERR_NOT_SUPPORTED`: CSR 不支持

**支持的 CSR 编号**:
```c
#define NACL_CSR_VSSTATUS     0x200
#define NACL_CSR_VSTVEC       0x205
#define NACL_CSR_VSSCRATCH    0x240
#define NACL_CSR_VSEPC        0x241
#define NACL_CSR_VSCAUSE      0x242
#define NACL_CSR_VSTVAL       0x243
#define NACL_CSR_VSATP        0x280
// ... 更多 CSR 编号
```

**示例**:
```c
// 读取 vsstatus
struct sbiret ret = sbi_nacl_csr_read(NACL_CSR_VSSTATUS);
if (ret.error == SBI_SUCCESS) {
    unsigned long vsstatus = ret.value;
    printf("VSSTATUS = 0x%lx\n", vsstatus);
}
```

### 7. sbi_nacl_csr_write

写入指定的 CSR 值。

**函数 ID**: 0x6

**C 原型**:
```c
struct sbiret sbi_nacl_csr_write(unsigned long csr_num,
                                  unsigned long csr_val);
```

**参数**:
- `csr_num`: CSR 编号
- `csr_val`: 要写入的值

**返回值**:
- 成功: `error = SBI_SUCCESS`, `value = 0`
- 失败:
  - `error = SBI_ERR_INVALID_PARAM`: 参数无效
  - `error = SBI_ERR_DENIED`: 无写入权限
  - `error = SBI_ERR_NOT_SUPPORTED`: CSR 不支持

**示例**:
```c
// 写入 vstvec
struct sbiret ret = sbi_nacl_csr_write(NACL_CSR_VSTVEC, trap_handler);
if (ret.error != SBI_SUCCESS) {
    printf("Failed to write VSTVEC: %ld\n", ret.error);
}
```

### 8. sbi_nacl_csr_exchange

原子性地交换 CSR 值。

**函数 ID**: 0x7

**C 原型**:
```c
struct sbiret sbi_nacl_csr_exchange(unsigned long csr_num,
                                     unsigned long new_val,
                                     unsigned long mask);
```

**参数**:
- `csr_num`: CSR 编号
- `new_val`: 新值
- `mask`: 掩码（仅修改掩码为 1 的位）

**返回值**:
- 成功: `error = SBI_SUCCESS`, `value = 原始值`
- 失败:
  - `error = SBI_ERR_INVALID_PARAM`: 参数无效
  - `error = SBI_ERR_DENIED`: 权限不足

**示例**:
```c
// 原子性设置 vsstatus 的 SIE 位
struct sbiret ret = sbi_nacl_csr_exchange(NACL_CSR_VSSTATUS,
                                           SSTATUS_SIE,
                                           SSTATUS_SIE);
if (ret.error == SBI_SUCCESS) {
    printf("Original VSSTATUS: 0x%lx\n", ret.value);
}
```

### 9. sbi_nacl_inject_irq

向 L1 虚拟机监控器注入虚拟中断。

**函数 ID**: 0x8

**C 原型**:
```c
struct sbiret sbi_nacl_inject_irq(unsigned long irq_type,
                                   unsigned long irq_num,
                                   unsigned long vmid);
```

**参数**:
- `irq_type`: 中断类型
- `irq_num`: 中断编号
- `vmid`: 目标 VMID（0 表示当前 VMID）

**中断类型**:
```c
#define NACL_IRQ_TYPE_SOFT      0  // 软件中断
#define NACL_IRQ_TYPE_TIMER     1  // 定时器中断
#define NACL_IRQ_TYPE_EXTERNAL  2  // 外部中断
#define NACL_IRQ_TYPE_LOCAL     3  // 本地中断
```

**返回值**:
- 成功: `error = SBI_SUCCESS`, `value = 0`
- 失败:
  - `error = SBI_ERR_INVALID_PARAM`: 参数无效
  - `error = SBI_ERR_DENIED`: 权限不足
  - `error = SBI_ERR_NO_MEM`: 中断队列满

**示例**:
```c
// 注入定时器中断
struct sbiret ret = sbi_nacl_inject_irq(NACL_IRQ_TYPE_TIMER, 0, current_vmid);
```

### 10. sbi_nacl_batch_ops

批量执行多个操作。

**函数 ID**: 0x9

**C 原型**:
```c
struct sbiret sbi_nacl_batch_ops(unsigned long batch_addr,
                                  unsigned long batch_size);
```

**参数**:
- `batch_addr`: 批量操作描述符数组物理地址
- `batch_size`: 批量操作数量

**批量操作描述符**:
```c
struct nacl_batch_op {
    uint32_t op_type;     // 操作类型
    uint32_t flags;       // 操作标志
    uint64_t arg1;        // 参数 1
    uint64_t arg2;        // 参数 2
    uint64_t result;      // 操作结果
    uint64_t reserved;    // 保留
};
```

**操作类型**:
```c
#define NACL_OP_CSR_READ     1
#define NACL_OP_CSR_WRITE    2
#define NACL_OP_HFENCE_VVMA  3
#define NACL_OP_HFENCE_GVMA  4
#define NACL_OP_INJECT_IRQ   5
```

**返回值**:
- 成功: `error = SBI_SUCCESS`, `value = 成功操作数`
- 失败:
  - `error = SBI_ERR_INVALID_PARAM`: 参数无效
  - `error = SBI_ERR_INVALID_ADDRESS`: 地址无效
  - `error = SBI_ERR_DENIED`: 权限不足

### 11. sbi_nacl_get_feature

查询特定功能是否支持。

**函数 ID**: 0xA

**C 原型**:
```c
struct sbiret sbi_nacl_get_feature(unsigned long feature_id);
```

**参数**:
- `feature_id`: 功能 ID

**功能 ID**:
```c
#define NACL_FEATURE_SHARED_MEM     0  // 共享内存
#define NACL_FEATURE_CSR_BATCH      1  // CSR 批量操作
#define NACL_FEATURE_HFENCE_BATCH   2  // HFENCE 批量操作
#define NACL_FEATURE_AUTO_SWAP      3  // 自动交换
#define NACL_FEATURE_NESTED_VIRT    4  // 嵌套虚拟化
#define NACL_FEATURE_IRQ_INJECT     5  // 中断注入
#define NACL_FEATURE_PERF_MON       6  // 性能监控
#define NACL_FEATURE_DEBUG          7  // 调试支持
```

**返回值**:
- 支持: `error = SBI_SUCCESS`, `value = 功能版本`
- 不支持: `error = SBI_ERR_NOT_SUPPORTED`, `value = 0`

**示例**:
```c
// 检查是否支持 CSR 批量操作
struct sbiret ret = sbi_nacl_get_feature(NACL_FEATURE_CSR_BATCH);
if (ret.error == SBI_SUCCESS) {
    printf("CSR batch feature version: %ld\n", ret.value);
}
```

### 12. sbi_nacl_get_version

获取 NACL 扩展版本。

**函数 ID**: 0xB

**C 原型**:
```c
struct sbiret sbi_nacl_get_version(void);
```

**参数**: 无

**返回值**:
- 成功: `error = SBI_SUCCESS`, `value = 版本号`
  - 主版本号: `(value >> 24) & 0xFF`
  - 次版本号: `(value >> 16) & 0xFF`
  - 补丁版本: `(value >> 8) & 0xFF`

**版本格式**:
```c
#define NACL_VERSION(major, minor, patch) \
    (((major) << 24) | ((minor) << 16) | ((patch) << 8))
```

**示例**:
```c
struct sbiret ret = sbi_nacl_get_version();
if (ret.error == SBI_SUCCESS) {
    unsigned long version = ret.value;
    printf("NACL version: %lu.%lu.%lu\n",
           (version >> 24) & 0xFF,
           (version >> 16) & 0xFF,
           (version >> 8) & 0xFF);
}
```

### 13. sbi_nacl_set_mode

设置 NACL 操作模式。

**函数 ID**: 0xC

**C 原型**:
```c
struct sbiret sbi_nacl_set_mode(unsigned long mode);
```

**参数**:
- `mode`: 操作模式

**模式定义**:
```c
#define NACL_MODE_DISABLED     0  // 禁用 NACL
#define NACL_MODE_PASSTHROUGH  1  // 直通模式
#define NACL_MODE_ACCELERATED  2  // 加速模式
#define NACL_MODE_DEBUG        3  // 调试模式
```

**返回值**:
- 成功: `error = SBI_SUCCESS`, `value = 0`
- 失败:
  - `error = SBI_ERR_INVALID_PARAM`: 模式无效
  - `error = SBI_ERR_DENIED`: 无权限设置模式

### 14. sbi_nacl_get_stats

获取 NACL 操作统计信息。

**函数 ID**: 0xD

**C 原型**:
```c
struct sbiret sbi_nacl_get_stats(unsigned long stat_id,
                                  unsigned long clear);
```

**参数**:
- `stat_id`: 统计 ID
- `clear`: 是否清零统计（非 0 表示清零）

**统计 ID**:
```c
#define NACL_STAT_SBI_CALLS     0  // SBI 调用次数
#define NACL_STAT_CSR_ACCESSES  1  // CSR 访问次数
#define NACL_STAT_HFENCE_CALLS  2  // HFENCE 调用次数
#define NACL_STAT_VM_EXITS      3  // VM 退出次数
#define NACL_STAT_IRQ_INJECTS   4  // 中断注入次数
```

**返回值**:
- 成功: `error = SBI_SUCCESS`, `value = 统计值`
- 失败:
  - `error = SBI_ERR_INVALID_PARAM`: 统计 ID 无效

### 15. sbi_nacl_debug

调试相关操作。

**函数 ID**: 0xE

**C 原型**:
```c
struct sbiret sbi_nacl_debug(unsigned long debug_op,
                              unsigned long arg1,
                              unsigned long arg2);
```

**参数**:
- `debug_op`: 调试操作
- `arg1`: 参数 1
- `arg2`: 参数 2

**调试操作**:
```c
#define NACL_DEBUG_DUMP_STATE    0  // 转储状态
#define NACL_DEBUG_SET_BREAKPT   1  // 设置断点
#define NACL_DEBUG_CLR_BREAKPT   2  // 清除断点
#define NACL_DEBUG_STEP          3  // 单步执行
#define NACL_DEBUG_TRACE         4  // 跟踪执行
```

**返回值**:
- 成功: `error = SBI_SUCCESS`, `value = 操作结果`
- 失败:
  - `error = SBI_ERR_INVALID_PARAM`: 参数无效
  - `error = SBI_ERR_DENIED`: 调试功能未启用

## 错误处理

### 错误码定义

所有 NACL 函数使用标准的 SBI 错误码：

```c
#define SBI_SUCCESS              0   // 成功
#define SBI_ERR_FAILED          -1   // 一般失败
#define SBI_ERR_NOT_SUPPORTED   -2   // 不支持
#define SBI_ERR_INVALID_PARAM   -3   // 参数无效
#define SBI_ERR_DENIED          -4   // 权限被拒绝
#define SBI_ERR_INVALID_ADDRESS -5   // 地址无效
#define SBI_ERR_ALREADY_AVAILABLE -6 // 已存在
#define SBI_ERR_ALREADY_STARTED -7   // 已开始
#define SBI_ERR_ALREADY_STOPPED -8   // 已停止
#define SBI_ERR_NO_SHMEM        -9   // 无共享内存
#define SBI_ERR_INVALID_STATE   -10  // 状态无效
#define SBI_ERR_BAD_RANGE        -11 // 范围错误
#define SBI_ERR_TIMEOUT         -12  // 超时
#define SBI_ERR_NO_MEM          -13  // 内存不足
#define SBI_ERR_IO_ERROR        -14  // I/O 错误
#define SBI_ERR_REMOTE_POWER_ON  -15 // 远程电源开启
#define SBI_ERR_REMOTE_POWER_OFF -16 // 远程电源关闭
#define SBI_ERR_REMOTE_REBOOT   -17  // 远程重启
#define SBI_ERR_REMOTE_SUSPEND  -18  // 远程挂起
#define SBI_ERR_REMOTE_RESUME   -19  // 远程恢复
#define SBI_ERR_SYSTEM_HART_ADDED -20 // 系统线程添加
#define SBI_ERR_SYSTEM_HART_REMOVED -21 // 系统线程移除
#define SBI_ERR_PLATFORM_SPECIFIC -22 // 平台特定
#define SBI_ERR_INVALID_DATA    -23  // 数据无效
#define SBI_ERR_STOPPED         -24  // 已停止
#define SBI_ERR_POLLING         -25  // 轮询中
#define SBI_ERR_PROTOCOL        -26  // 协议错误
```

### 错误处理建议

1. **始终检查返回值**：每次调用后都应检查 `error` 字段
2. **记录错误**：记录错误以便调试和分析
3. **优雅降级**：在不支持某些功能时提供替代方案
4. **重试机制**：对临时错误实现重试机制

```c
// 错误处理示例
struct sbiret ret = sbi_nacl_csr_read(NACL_CSR_VSSTATUS);
switch (ret.error) {
    case SBI_SUCCESS:
        // 成功处理
        handle_vsstatus(ret.value);
        break;
    case SBI_ERR_NOT_SUPPORTED:
        // 功能不支持，使用替代方案
        fallback_vsstatus_read();
        break;
    case SBI_ERR_DENIED:
        // 权限不足
        report_permission_error();
        break;
    default:
        // 其他错误
        report_error("CSR read failed", ret.error);
        break;
}
```

## 性能优化建议

### 批量操作

- 尽可能使用批量操作函数
- 将多个 CSR 操作合并为一次调用
- 使用 `sbi_nacl_batch_ops` 进行复杂操作序列

### 同步优化

- 仅在必要时调用同步函数
- 使用脏位图只同步修改的 CSR
- 延迟非关键同步操作

### 内存访问

- 使用对齐的内存访问
- 利用缓存局部性
- 避免频繁的小内存操作

## 使用示例

### 基础初始化序列

```c
int nacl_init(void) {
    // 1. 检查 NACL 支持
    struct sbiret ret = sbi_nacl_get_version();
    if (ret.error != SBI_SUCCESS) {
        return -ENOTSUP;
    }

    // 2. 设置共享内存
    void *shmem = alloc_page_aligned(4 * 1024);
    ret = sbi_nacl_set_shmem((unsigned long)shmem, 0,
                              NACL_SHMEM_FLAG_ENABLE |
                              NACL_SHMEM_FLAG_CLEAR);
    if (ret.error != SBI_SUCCESS) {
        return -EIO;
    }

    // 3. 设置加速模式
    ret = sbi_nacl_set_mode(NACL_MODE_ACCELERATED);
    if (ret.error != SBI_SUCCESS) {
        return -EIO;
    }

    return 0;
}
```

### CSR 批量操作

```c
void batch_csr_operations(void) {
    // 准备批量操作描述符
    struct nacl_batch_op ops[4] = {
        {NACL_OP_CSR_READ, 0, NACL_CSR_VSSTATUS, 0, 0, 0},
        {NACL_OP_CSR_WRITE, 0, NACL_CSR_VSTVEC, 0, trap_handler, 0},
        {NACL_OP_CSR_READ, 0, NACL_CSR_VSIE, 0, 0, 0},
        {NACL_OP_CSR_WRITE, 0, NACL_CSR_VSIE, 0, new_vsie, 0}
    };

    // 执行批量操作
    struct sbiret ret = sbi_nacl_batch_ops((unsigned long)ops, 4);
    if (ret.error == SBI_SUCCESS) {
        printf("Successfully executed %ld operations\n", ret.value);
    }
}
```