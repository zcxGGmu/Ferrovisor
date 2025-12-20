# SBI 嵌套加速使用示例和最佳实践

本文档提供了 RISC-V SBI 嵌套加速扩展（NACL）的实际使用示例、最佳实践和性能优化建议。

## 目录

1. [快速开始示例](#快速开始示例)
2. [典型使用场景](#典型使用场景)
3. [高级示例](#高级示例)
4. [性能优化最佳实践](#性能优化最佳实践)
5. [调试和故障排除](#调试和故障排除)
6. [常见问题](#常见问题)

## 快速开始示例

### 基础初始化

```c
#include <sbi/sbi.h>
#include <sbi/riscv_asm.h>

// NACL 初始化函数
int nacl_init_example(void) {
    struct sbiret ret;

    // 1. 检查 NACL 扩展是否可用
    ret = sbi_probe_extension(SBI_EXT_NACL);
    if (ret.error != SBI_SUCCESS || ret.value == 0) {
        printf("NACL extension not available\n");
        return -ENOTSUP;
    }

    // 2. 获取 NACL 版本信息
    ret = sbi_nacl_get_version();
    if (ret.error == SBI_SUCCESS) {
        unsigned long version = ret.value;
        printf("NACL version: %lu.%lu.%lu\n",
               (version >> 24) & 0xFF,
               (version >> 16) & 0xFF,
               (version >> 8) & 0xFF);
    }

    // 3. 分配并设置共享内存
    void *shmem = alloc_contiguous(4096);  // 4KB 对齐
    if (!shmem) {
        printf("Failed to allocate shared memory\n");
        return -ENOMEM;
    }

    // 清零共享内存
    memset(shmem, 0, 4096);

    // 设置共享内存
    ret = sbi_nacl_set_shmem((unsigned long)shmem, 0,
                              NACL_SHMEM_FLAG_ENABLE);
    if (ret.error != SBI_SUCCESS) {
        printf("Failed to set shared memory: %ld\n", ret.error);
        free_contiguous(shmem);
        return -EIO;
    }

    // 4. 设置加速模式
    ret = sbi_nacl_set_mode(NACL_MODE_ACCELERATED);
    if (ret.error != SBI_SUCCESS) {
        printf("Failed to set accelerated mode: %ld\n", ret.error);
    }

    printf("NACL initialized successfully\n");
    return 0;
}
```

### 基本 CSR 操作

```c
// CSR 读取示例
void example_csr_read(void) {
    struct sbiret ret;

    // 读取 vsstatus
    ret = sbi_nacl_csr_read(NACL_CSR_VSSTATUS);
    if (ret.error == SBI_SUCCESS) {
        unsigned long vsstatus = ret.value;
        printf("VSSTATUS = 0x%lx\n", vsstatus);

        // 检查特定位
        if (vsstatus & SSTATUS_SIE) {
            printf("Supervisor interrupts enabled\n");
        }
    } else {
        printf("Failed to read VSSTATUS: %ld\n", ret.error);
    }
}

// CSR 写入示例
void example_csr_write(void) {
    struct sbiret ret;

    // 设置 vstvec
    unsigned long trap_handler = 0x80000000;
    ret = sbi_nacl_csr_write(NACL_CSR_VSTVEC, trap_handler);
    if (ret.error != SBI_SUCCESS) {
        printf("Failed to write VSTVEC: %ld\n", ret.error);
        return;
    }

    // 验证写入
    ret = sbi_nacl_csr_read(NACL_CSR_VSTVEC);
    if (ret.error == SBI_SUCCESS && ret.value == trap_handler) {
        printf("VSTVEC set successfully to 0x%lx\n", trap_handler);
    }
}

// 原子性 CSR 修改示例
void example_csr_exchange(void) {
    struct sbiret ret;

    // 原子性设置 vsstatus 的 SIE 位
    ret = sbi_nacl_csr_exchange(NACL_CSR_VSSTATUS,
                                SSTATUS_SIE,  // 新值
                                SSTATUS_SIE); // 掩码
    if (ret.error == SBI_SUCCESS) {
        unsigned long old_vsstatus = ret.value;
        printf("Old VSSTATUS: 0x%lx\n", old_vsstatus);
        printf("Successfully set SIE bit\n");
    }
}
```

## 典型使用场景

### 1. 虚拟机切换优化

```c
// 虚拟机切换时的状态保存和恢复
struct vm_context {
    unsigned long vsstatus;
    unsigned long vstvec;
    unsigned long vsepc;
    unsigned long vscause;
    unsigned long vstval;
    unsigned long vsatp;
    unsigned long hgeie;
    unsigned long hgeip;
};

// 保存虚拟机状态
int save_vm_state(struct vm_context *ctx) {
    struct sbiret ret;

    // 批量读取关键 CSR
    struct nacl_batch_op ops[] = {
        {NACL_OP_CSR_READ, 0, NACL_CSR_VSSTATUS, 0, 0, 0},
        {NACL_OP_CSR_READ, 0, NACL_CSR_VSTVEC, 0, 0, 0},
        {NACL_OP_CSR_READ, 0, NACL_CSR_VSEPC, 0, 0, 0},
        {NACL_OP_CSR_READ, 0, NACL_CSR_VSCAUSE, 0, 0, 0},
        {NACL_OP_CSR_READ, 0, NACL_CSR_VSTVAL, 0, 0, 0},
        {NACL_OP_CSR_READ, 0, NACL_CSR_VSATP, 0, 0, 0},
    };

    ret = sbi_nacl_batch_ops((unsigned long)ops, 6);
    if (ret.error != SBI_SUCCESS) {
        return -EIO;
    }

    // 保存结果到上下文
    ctx->vsstatus = ops[0].result;
    ctx->vstvec = ops[1].result;
    ctx->vsepc = ops[2].result;
    ctx->vscause = ops[3].result;
    ctx->vstval = ops[4].result;
    ctx->vsatp = ops[5].result;

    return 0;
}

// 恢复虚拟机状态
int restore_vm_state(struct vm_context *ctx) {
    struct sbiret ret;

    // 批量写入关键 CSR
    struct nacl_batch_op ops[] = {
        {NACL_OP_CSR_WRITE, 0, NACL_CSR_VSATP, 0, ctx->vsatp, 0},
        {NACL_OP_CSR_WRITE, 0, NACL_CSR_VSTVEC, 0, ctx->vstvec, 0},
        {NACL_OP_CSR_WRITE, 0, NACL_CSR_VSEPC, 0, ctx->vsepc, 0},
        {NACL_OP_CSR_WRITE, 0, NACL_CSR_VSCAUSE, 0, ctx->vscause, 0},
        {NACL_OP_CSR_WRITE, 0, NACL_CSR_VSTVAL, 0, ctx->vstval, 0},
        {NACL_OP_CSR_WRITE, 0, NACL_CSR_VSSTATUS, 0, ctx->vsstatus, 0},
    };

    ret = sbi_nacl_batch_ops((unsigned long)ops, 6);
    if (ret.error != SBI_SUCCESS) {
        return -EIO;
    }

    // 同步所有更改
    ret = sbi_nacl_sync_csr((1UL << 6) - 1, 0);  // 同步前 6 个 CSR
    if (ret.error != SBI_SUCCESS) {
        return -EIO;
    }

    return 0;
}
```

### 2. 内存管理优化

```c
// 高效的页表更新
struct pt_update_batch {
    unsigned long gpa;
    unsigned long pte;
    unsigned long asid;
    unsigned long vmid;
    int count;
};

// 批量更新页表并执行 HFENCE
int batch_pt_update(struct pt_update_batch *batch) {
    struct sbiret ret;

    // 1. 批量写入 PTE
    for (int i = 0; i < batch->count; i++) {
        // 使用共享内存直接写入 PTE
        // 这里假设共享内存已映射到可写地址
        volatile unsigned long *pte_addr =
            (volatile unsigned long *)(gpa_to_hva(batch[i].gpa));
        *pte_addr = batch[i].pte;
    }

    // 2. 执行 HFENCE.VVMA 失效相关 TLB
    if (batch->count > 0) {
        // 计算地址范围
        unsigned long min_gpa = batch[0].gpa;
        unsigned long max_gpa = batch[0].gpa;

        for (int i = 1; i < batch->count; i++) {
            if (batch[i].gpa < min_gpa) min_gpa = batch[i].gpa;
            if (batch[i].gpa > max_gpa) max_gpa = batch[i].gpa;
        }

        // 执行批量 HFENCE
        ret = sbi_nacl_hfence_vvma(min_gpa,
                                    batch[0].asid,
                                    batch[0].vmid,
                                    max_gpa - min_gpa + PAGE_SIZE);
        if (ret.error != SBI_SUCCESS) {
            return -EIO;
        }
    }

    return 0;
}
```

### 3. 中断处理优化

```c
// 高效的虚拟中断注入
struct virq_queue {
    unsigned long irq_type;
    unsigned long irq_num;
    unsigned long vmid;
    unsigned long pending;
};

// 批量注入中断
int batch_inject_irqs(struct virq_queue *queue, int count) {
    struct sbiret ret;
    int injected = 0;

    // 批量注入待处理的中断
    for (int i = 0; i < count; i++) {
        if (queue[i].pending) {
            ret = sbi_nacl_inject_irq(queue[i].irq_type,
                                       queue[i].irq_num,
                                       queue[i].vmid);
            if (ret.error == SBI_SUCCESS) {
                queue[i].pending = 0;
                injected++;
            } else {
                printf("Failed to inject IRQ %ld: %ld\n",
                       queue[i].irq_num, ret.error);
            }
        }
    }

    return injected;
}

// 中断处理示例
void handle_virtual_interrupts(void) {
    static struct virq_queue irq_queue[32];
    static int queue_count = 0;

    // 模拟中断到达
    if (queue_count < 32) {
        irq_queue[queue_count].irq_type = NACL_IRQ_TYPE_TIMER;
        irq_queue[queue_count].irq_num = 5;
        irq_queue[queue_count].vmid = current_vmid;
        irq_queue[queue_count].pending = 1;
        queue_count++;
    }

    // 批量处理中断
    if (queue_count > 0) {
        int injected = batch_inject_irqs(irq_queue, queue_count);
        if (injected > 0) {
            // 压缩队列
            int remaining = 0;
            for (int i = 0; i < queue_count; i++) {
                if (irq_queue[i].pending) {
                    irq_queue[remaining++] = irq_queue[i];
                }
            }
            queue_count = remaining;
        }
    }
}
```

## 高级示例

### 1. 嵌套虚拟化管理

```c
// 嵌套虚拟机管理器
struct nested_vm {
    unsigned long vmid;
    unsigned long parent_vmid;
    struct vm_context ctx;
    struct nested_vm *child;
    struct nested_vm *next;
};

static struct nested_vm *vm_list = NULL;

// 创建嵌套虚拟机
int create_nested_vm(unsigned long vmid, unsigned long parent_vmid) {
    struct nested_vm *nvm = malloc(sizeof(struct nested_vm));
    if (!nvm) {
        return -ENOMEM;
    }

    // 初始化虚拟机结构
    nvm->vmid = vmid;
    nvm->parent_vmid = parent_vmid;
    nvm->child = NULL;
    nvm->next = vm_list;
    vm_list = nvm;

    // 设置初始虚拟机状态
    init_vm_context(&nvm->ctx);

    printf("Created nested VM %ld under parent %ld\n", vmid, parent_vmid);
    return 0;
}

// 进入嵌套虚拟机
int enter_nested_vm(unsigned long vmid) {
    struct nested_vm *nvm = find_vm(vmid);
    if (!nvm) {
        return -ENOENT;
    }

    // 保存当前状态
    save_vm_state(&current_vm_ctx);

    // 恢复目标虚拟机状态
    restore_vm_state(&nvm->ctx);

    // 切换 VMID
    switch_vmid(vmid);

    // 模拟 SRET 进入虚拟机
    struct sbiret ret = sbi_nacl_sret(NACL_SRET_FLAG_AUTO_SWAP);
    if (ret.error != SBI_SUCCESS) {
        printf("Failed to execute SRET: %ld\n", ret.error);
        return -EIO;
    }

    current_vmid = vmid;
    return 0;
}

// 退出嵌套虚拟机
int exit_nested_vm(void) {
    // 保存当前虚拟机状态
    struct nested_vm *nvm = find_vm(current_vmid);
    if (nvm) {
        save_vm_state(&nvm->ctx);
    }

    // 恢复父虚拟机状态
    if (nvm && nvm->parent_vmid) {
        restore_vm_state(&find_vm(nvm->parent_vmid)->ctx);
        switch_vmid(nvm->parent_vmid);
        current_vmid = nvm->parent_vmid;
    }

    return 0;
}
```

### 2. 性能监控和分析

```c
// 性能统计结构
struct nacl_perf_stats {
    unsigned long sbi_calls;
    unsigned long csr_reads;
    unsigned long csr_writes;
    unsigned long hfence_calls;
    unsigned long sret_calls;
    unsigned long vm_exits;
    unsigned long irq_injects;
    unsigned long batch_ops;
};

// 获取性能统计
void get_perf_stats(struct nacl_perf_stats *stats) {
    struct sbiret ret;

    // 获取各项统计
    ret = sbi_nacl_get_stats(NACL_STAT_SBI_CALLS, 0);
    stats->sbi_calls = (ret.error == SBI_SUCCESS) ? ret.value : 0;

    ret = sbi_nacl_get_stats(NACL_STAT_CSR_ACCESSES, 0);
    stats->csr_reads = (ret.error == SBI_SUCCESS) ? ret.value : 0;

    ret = sbi_nacl_get_stats(NACL_STAT_HFENCE_CALLS, 0);
    stats->hfence_calls = (ret.error == SBI_SUCCESS) ? ret.value : 0;

    ret = sbi_nacl_get_stats(NACL_STAT_VM_EXITS, 0);
    stats->vm_exits = (ret.error == SBI_SUCCESS) ? ret.value : 0;

    ret = sbi_nacl_get_stats(NACL_STAT_IRQ_INJECTS, 0);
    stats->irq_injects = (ret.error == SBI_SUCCESS) ? ret.value : 0;
}

// 打印性能报告
void print_perf_report(void) {
    struct nacl_perf_stats stats;
    get_perf_stats(&stats);

    printf("\n=== NACL Performance Report ===\n");
    printf("SBI Calls:      %lu\n", stats.sbi_calls);
    printf("CSR Accesses:   %lu\n", stats.csr_reads);
    printf("HFENCE Calls:   %lu\n", stats.hfence_calls);
    printf("SRET Calls:     %lu\n", stats.sret_calls);
    printf("VM Exits:       %lu\n", stats.vm_exits);
    printf("IRQ Injections: %lu\n", stats.irq_injects);

    // 计算效率指标
    if (stats.sbi_calls > 0) {
        printf("Average operations per call: %.2f\n",
               (float)(stats.csr_reads + stats.hfence_calls) / stats.sbi_calls);
    }

    if (stats.vm_exits > 0) {
        printf("HFENCE efficiency: %.2f%%\n",
               100.0 * (1.0 - (float)stats.hfence_calls / stats.vm_exits));
    }
}
```

### 3. 调试支持

```c
// 调试控制结构
struct nacl_debug_ctx {
    int enabled;
    int trace_enabled;
    unsigned long breakpt_addr;
    unsigned long step_count;
};

// 设置断点
int set_breakpoint(unsigned long addr) {
    struct sbiret ret;

    ret = sbi_nacl_debug(NACL_DEBUG_SET_BREAKPT, addr, 0);
    if (ret.error == SBI_SUCCESS) {
        printf("Breakpoint set at 0x%lx\n", addr);
        return 0;
    } else {
        printf("Failed to set breakpoint: %ld\n", ret.error);
        return -EIO;
    }
}

// 启用跟踪
int enable_tracing(void) {
    struct sbiret ret;

    ret = sbi_nacl_debug(NACL_DEBUG_TRACE, 1, 0);
    if (ret.error == SBI_SUCCESS) {
        printf("Tracing enabled\n");
        return 0;
    } else {
        printf("Failed to enable tracing: %ld\n", ret.error);
        return -EIO;
    }
}

// 单步执行
int single_step(unsigned long steps) {
    struct sbiret ret;

    for (unsigned long i = 0; i < steps; i++) {
        ret = sbi_nacl_debug(NACL_DEBUG_STEP, 0, 0);
        if (ret.error != SBI_SUCCESS) {
            printf("Step %lu failed: %ld\n", i, ret.error);
            return -EIO;
        }

        // 打印当前状态
        printf("Step %lu: PC = 0x%lx\n", i + 1, ret.value);
    }

    return 0;
}
```

## 性能优化最佳实践

### 1. 批量操作优化

```c
// 好的做法：使用批量操作
void optimized_csr_operations(void) {
    // 准备批量操作
    struct nacl_batch_op ops[8];
    int op_count = 0;

    // 收集所有待执行的 CSR 操作
    ops[op_count++] = (struct nacl_batch_op){
        NACL_OP_CSR_READ, 0, NACL_CSR_VSSTATUS, 0, 0, 0
    };
    ops[op_count++] = (struct nacl_batch_op){
        NACL_OP_CSR_WRITE, 0, NACL_CSR_VSTVEC, 0, new_trap_handler, 0
    };
    ops[op_count++] = (struct nacl_batch_op){
        NACL_OP_CSR_EXCHANGE, 0, NACL_CSR_VSIE, 0, new_vsie, VSIE_MASK
    };

    // 一次性执行所有操作
    struct sbiret ret = sbi_nacl_batch_ops((unsigned long)ops, op_count);
    if (ret.error == SBI_SUCCESS) {
        printf("Executed %ld operations in batch\n", ret.value);
    }
}

// 不好的做法：单独调用每个操作
void unoptimized_csr_operations(void) {
    // 每个操作单独调用，效率低下
    sbi_nacl_csr_read(NACL_CSR_VSSTATUS);
    sbi_nacl_csr_write(NACL_CSR_VSTVEC, new_trap_handler);
    sbi_nacl_csr_exchange(NACL_CSR_VSIE, new_vsie, VSIE_MASK);
}
```

### 2. 内存访问优化

```c
// 优化内存访问模式
void optimized_memory_access(void) {
    // 1. 对齐访问
    struct aligned_data {
        unsigned long csr_values[16] __attribute__((aligned(64)));
    };

    // 2. 缓存友好的数据布局
    struct cache_friendly_layout {
        // 热点数据放在前面
        unsigned long frequently_used[4];
        // 冷数据放在后面
        unsigned long rarely_used[12];
    };

    // 3. 预取数据
    __builtin_prefetch(&shared_mem->csr_array, 0, 3);

    // 4. 使用 SIMD 指令（如果支持）
    #ifdef __riscv_vector
    vuint64m1_t vec = vle64_v_u64m1(data, vl);
    #endif
}
```

### 3. 同步优化

```c
// 延迟同步策略
struct delayed_sync_ctx {
    unsigned long pending_csr_mask;
    unsigned long sync_threshold;
    unsigned long pending_count;
};

void delayed_sync_init(struct delayed_sync_ctx *ctx) {
    ctx->pending_csr_mask = 0;
    ctx->sync_threshold = 16;  // 16 个修改后同步
    ctx->pending_count = 0;
}

void mark_csr_dirty(struct delayed_sync_ctx *ctx, unsigned long csr_num) {
    ctx->pending_csr_mask |= (1UL << csr_num);
    ctx->pending_count++;

    // 达到阈值时执行同步
    if (ctx->pending_count >= ctx->sync_threshold) {
        struct sbiret ret = sbi_nacl_sync_csr(ctx->pending_csr_mask, 0);
        if (ret.error == SBI_SUCCESS) {
            ctx->pending_csr_mask = 0;
            ctx->pending_count = 0;
        }
    }
}

// 强制同步
void force_sync(struct delayed_sync_ctx *ctx) {
    if (ctx->pending_count > 0) {
        sbi_nacl_sync_csr(ctx->pending_csr_mask, 0);
        ctx->pending_csr_mask = 0;
        ctx->pending_count = 0;
    }
}
```

## 调试和故障排除

### 1. 常见错误模式

```c
// 错误检测和恢复
int detect_and_recover_from_error(void) {
    static unsigned long consecutive_errors = 0;
    static unsigned long last_error_time = 0;
    unsigned long current_time = get_timestamp();

    // 检查错误频率
    if (current_time - last_error_time < ERROR_WINDOW_MS) {
        consecutive_errors++;
        if (consecutive_errors > MAX_CONSECUTIVE_ERRORS) {
            // 触发恢复流程
            printf("Too many consecutive errors, initiating recovery\n");
            return initiate_error_recovery();
        }
    } else {
        consecutive_errors = 1;
    }

    last_error_time = current_time;
    return 0;
}

// 状态验证
int validate_nacl_state(void) {
    struct sbiret ret;

    // 验证共享内存状态
    ret = sbi_nacl_get_feature(NACL_FEATURE_SHARED_MEM);
    if (ret.error != SBI_SUCCESS || ret.value == 0) {
        printf("Shared memory feature not available\n");
        return -ENOTSUP;
    }

    // 验证关键 CSR 可访问性
    ret = sbi_nacl_csr_read(NACL_CSR_VSSTATUS);
    if (ret.error != SBI_SUCCESS) {
        printf("Cannot access VSSTATUS: %ld\n", ret.error);
        return -EIO;
    }

    // 验证操作模式
    ret = sbi_nacl_get_mode();
    if (ret.error == SBI_SUCCESS && ret.value == NACL_MODE_DISABLED) {
        printf("NACL is disabled\n");
        return -EPERM;
    }

    return 0;
}
```

### 2. 调试工具

```c
// 转储 NACL 状态
void dump_nacl_state(void) {
    struct sbiret ret;

    printf("\n=== NACL State Dump ===\n");

    // 基本信息
    ret = sbi_nacl_get_version();
    printf("Version: %s\n",
           ret.error == SBI_SUCCESS ? format_version(ret.value) : "Unknown");

    ret = sbi_nacl_get_mode();
    printf("Mode: %s\n",
           ret.error == SBI_SUCCESS ? mode_to_string(ret.value) : "Unknown");

    // 关键 CSR 状态
    struct {
        unsigned long csr;
        const char *name;
    } csrs[] = {
        {NACL_CSR_VSSTATUS, "VSSTATUS"},
        {NACL_CSR_VSTVEC, "VSTVEC"},
        {NACL_CSR_VSEPC, "VSEPC"},
        {NACL_CSR_VSCAUSE, "VSCAUSE"},
        {NACL_CSR_VSATP, "VSATP"},
    };

    printf("\nCSR State:\n");
    for (int i = 0; i < ARRAY_SIZE(csrs); i++) {
        ret = sbi_nacl_csr_read(csrs[i].csr);
        printf("  %s = 0x%lx %s\n",
               csrs[i].name,
               ret.error == SBI_SUCCESS ? ret.value : 0,
               ret.error == SBI_SUCCESS ? "" : "(error)");
    }

    // 性能统计
    printf("\nPerformance Stats:\n");
    print_perf_report();
}

// 跟踪日志
struct nacl_trace_entry {
    unsigned long timestamp;
    unsigned long operation;
    unsigned long arg1;
    unsigned long arg2;
    unsigned long result;
    int error;
};

static struct nacl_trace_entry trace_buffer[1024];
static int trace_index = 0;

void trace_nacl_op(unsigned long op, unsigned long arg1,
                    unsigned long arg2, struct sbiret result) {
    if (trace_index < ARRAY_SIZE(trace_buffer)) {
        trace_buffer[trace_index] = (struct nacl_trace_entry){
            .timestamp = get_timestamp(),
            .operation = op,
            .arg1 = arg1,
            .arg2 = arg2,
            .result = result.value,
            .error = result.error
        };
        trace_index++;
    }
}

void print_trace_log(void) {
    printf("\n=== NACL Trace Log ===\n");
    for (int i = 0; i < trace_index; i++) {
        struct nacl_trace_entry *e = &trace_buffer[i];
        printf("[%lu] Op=%lu Args=(%lu,%lu) Result=%lu Error=%d\n",
               e->timestamp, e->operation, e->arg1, e->arg2,
               e->result, e->error);
    }
}
```

## 常见问题

### Q1: NACL 初始化失败怎么办？

**A**: 检查以下几点：
1. 确认硬件支持 NACL 扩展
2. 检查共享内存是否正确分配和对齐
3. 验证 SBI 实现是否支持 NACL
4. 检查权限设置

### Q2: CSR 操作返回权限错误？

**A**: 可能的原因：
1. 当前特权级别不足以访问该 CSR
2. CSR 编号无效
3. 虚拟化模式设置不正确

### Q3: 性能不如预期？

**A**: 优化建议：
1. 使用批量操作减少 SBI 调用次数
2. 实施延迟同步策略
3. 优化内存访问模式
4. 启用硬件加速功能

### Q4: 如何调试 NACL 问题？

**A**: 调试步骤：
1. 启用调试模式
2. 使用跟踪日志记录操作
3. 检查性能统计
4. 验证系统状态

### Q5: 嵌套虚拟化性能优化？

**A**: 优化策略：
1. 减少不必要的 VM Exit
2. 使用批量 HFENCE 操作
3. 优化页表更新频率
4. 合理使用缓存

## 参考资料

- [RISC-V SBI 规范](https://github.com/riscv-non-isa/riscv-sbi-doc)
- [RISC-V H 扩展规范](https://github.com/riscv/riscv-isa-manual)
- [NACL 扩展规范](https://github.com/riscv-non-isa/riscv-sbi-doc/blob/master/src/ext-nested-acceleration.adoc)