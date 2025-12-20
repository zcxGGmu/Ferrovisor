# RISC-V SBI 固件特性扩展使用示例和最佳实践

本文档提供了 SBI 固件特性扩展（FWFT）的实际使用示例、性能优化建议和最佳实践指南。

## 目录

1. [快速开始](#快速开始)
2. [典型使用场景](#典型使用场景)
3. [高级示例](#高级示例)
4. [性能优化](#性能优化)
5. [调试和故障排除](#调试和故障排除)
6. [安全最佳实践](#安全最佳实践)
7. [虚拟化环境使用](#虚拟化环境使用)
8. [常见问题](#常见问题)

## 快速开始

### 基础初始化

```c
#include <stdio.h>
#include <sbi/sbi.h>
#include "fwft.h"

// FWFT 扩展基础初始化
int fwft_init(void) {
    // 1. 检查 SBI 版本兼容性
    struct sbiret ret = sbi_get_sbi_version();
    if (ret.error != SBI_SUCCESS) {
        printf("Failed to get SBI version\n");
        return -EIO;
    }

    unsigned long version = ret.value;
    if (version < 0x03000000) {
        printf("SBI version %lu.%lu.%lu does not support FWFT\n",
               (version >> 24) & 0xFF, (version >> 16) & 0xFF, (version >> 8) & 0xFF);
        return -ENOTSUP;
    }

    // 2. 检查 FWFT 扩展可用性
    ret = sbi_probe_extension(SBI_EXT_FWFT);
    if (ret.error != SBI_SUCCESS || ret.value == 0) {
        printf("FWFT extension not available\n");
        return -ENOTSUP;
    }

    printf("FWFT extension initialized successfully\n");
    return 0;
}
```

### 特性查询示例

```c
// 查询所有可用特性
void query_available_features(void) {
    printf("=== Available Firmware Features ===\n");

    const struct {
        uint32_t id;
        const char *name;
        const char *description;
    } features[] = {
        {FWFT_MISALIGNED_EXC_DELEG, "Misaligned Exception Delegate",
         "Control misaligned access exception delegation"},
        {FWFT_LANDING_PAD, "Landing Pad",
         "Control supervisor mode landing pad support"},
        {FWFT_SHADOW_STACK, "Shadow Stack",
         "Control supervisor mode shadow stack support"},
        {FWFT_DOUBLE_TRAP, "Double Trap",
         "Control double trap support mechanism"},
        {FWFT_PTE_AD_HW_UPDATING, "PTE A/D Hardware Updating",
         "Control automatic PTE A/D bit updates"},
        {FWFT_POINTER_MASKING_PMLEN, "Pointer Masking PMLEN",
         "Control pointer masking length"}
    };

    for (int i = 0; i < ARRAY_SIZE(features); i++) {
        struct sbiret ret = sbi_fwft_get(features[i].id);

        printf("\n%s (0x%08x)\n", features[i].name, features[i].id);
        printf("  Description: %s\n", features[i].description);

        if (ret.error == SBI_SUCCESS) {
            printf("  Status:       Supported\n");
            printf("  Current Value: %lu\n", ret.value);

            // 特定值解释
            switch (features[i].id) {
                case FWFT_POINTER_MASKING_PMLEN:
                    if (ret.value > 0) {
                        printf("  PMLEN:       %lu bits\n", ret.value);
                        printf("  Mask Size:   2^%lu = %lu bytes\n",
                               ret.value, 1UL << ret.value);
                    }
                    break;
            }
        } else if (ret.error == SBI_ERR_NOT_SUPPORTED) {
            printf("  Status:       Not Supported\n");
        } else {
            printf("  Status:       Error %ld\n", ret.error);
        }
    }
}
```

## 典型使用场景

### 1. 安全系统配置

```c
// 高安全系统的特性配置
int configure_secure_system(void) {
    printf("Configuring security features...\n");

    // 1. 启用影子栈
    if (fwft_is_supported(FWFT_SHADOW_STACK)) {
        int ret = fwft_set_and_lock(FWFT_SHADOW_STACK, 1);
        if (ret != 0) {
            printf("ERROR: Failed to enable shadow stack: %s\n",
                   fwft_error_string(ret));
            return ret;
        }
        printf("✓ Shadow stack enabled and locked\n");
    } else {
        printf("⚠ Shadow stack not available\n");
    }

    // 2. 启用着陆垫
    if (fwft_is_supported(FWFT_LANDING_PAD)) {
        int ret = fwft_set_and_lock(FWFT_LANDING_PAD, 1);
        if (ret != 0) {
            printf("ERROR: Failed to enable landing pad: %s\n",
                   fwft_error_string(ret));
            return ret;
        }
        printf("✓ Landing pad enabled and locked\n");
    } else {
        printf("⚠ Landing pad not available\n");
    }

    // 3. 启用双重陷阱
    if (fwft_is_supported(FWFT_DOUBLE_TRAP)) {
        int ret = fwft_set_value(FWFT_DOUBLE_TRAP, 1);
        if (ret != 0) {
            printf("ERROR: Failed to enable double trap: %s\n",
                   fwft_error_string(ret));
            return ret;
        }
        printf("✓ Double trap enabled\n");
    } else {
        printf("⚠ Double trap not available\n");
    }

    // 4. 配置指针掩码
    if (fwft_is_supported(FWFT_POINTER_MASKING_PMLEN)) {
        // 选择合适的掩码长度（16位 = 64KB 掩码）
        int ret = fwft_set_value(FWFT_POINTER_MASKING_PMLEN, 16);
        if (ret != 0) {
            printf("ERROR: Failed to set pointer masking: %s\n",
                   fwft_error_string(ret));
            return ret;
        }
        printf("✓ Pointer masking configured (PMLEN=16)\n");
    } else {
        printf("⚠ Pointer masking not available\n");
    }

    printf("Security configuration completed\n");
    return 0;
}
```

### 2. 性能优化配置

```c
// 性能优化系统的特性配置
int configure_performance_optimized(void) {
    printf("Configuring performance features...\n");

    // 1. 禁用非对齐异常委托（减少异常处理开销）
    if (fwft_is_supported(FWFT_MISALIGNED_EXC_DELEG)) {
        unsigned long current = fwft_get_value(FWFT_MISALIGNED_EXC_DELEG);
        if (current == 1) {
            int ret = fwft_set_value(FWFT_MISALIGNED_EXC_DELEG, 0);
            if (ret != 0) {
                printf("WARNING: Failed to disable misaligned exception delegate: %s\n",
                       fwft_error_string(ret));
            } else {
                printf("✓ Misaligned exception delegate disabled\n");
            }
        } else {
            printf("✓ Misaligned exception delegate already disabled\n");
        }
    }

    // 2. 启用硬件 A/D 位更新（减少软件维护开销）
    if (fwft_is_supported(FWFT_PTE_AD_HW_UPDATING)) {
        unsigned long current = fwft_get_value(FWFT_PTE_AD_HW_UPDATING);
        if (current == 0) {
            int ret = fwft_set_value(FWFT_PTE_AD_HW_UPDATING, 1);
            if (ret != 0) {
                printf("WARNING: Failed to enable hardware A/D updating: %s\n",
                       fwft_error_string(ret));
            } else {
                printf("✓ Hardware A/D updating enabled\n");
            }
        } else {
            printf("✓ Hardware A/D updating already enabled\n");
        }
    }

    // 3. 禁用影子栈（如果不需要安全特性）
    if (fwft_is_supported(FWFT_SHADOW_STACK)) {
        unsigned long current = fwft_get_value(FWFT_SHADOW_STACK);
        if (current == 1 && !security_required()) {
            int ret = fwft_set_value(FWFT_SHADOW_STACK, 0);
            if (ret == 0 || ret == SBI_ERR_DENIED_LOCKED) {
                if (ret == 0) {
                    printf("✓ Shadow stack disabled for performance\n");
                } else {
                    printf("⚠ Shadow stack locked, cannot disable\n");
                }
            }
        }
    }

    printf("Performance configuration completed\n");
    return 0;
}
```

### 3. 调试环境配置

```c
// 调试环境的特性配置
int configure_debug_environment(void) {
    printf("Configuring debug features...\n");

    // 1. 启用非对齐异常委托（便于调试）
    if (fwft_is_supported(FWFT_MISALIGNED_EXC_DELEG)) {
        int ret = fwft_set_value(FWFT_MISALIGNED_EXC_DELEG, 1);
        if (ret != 0) {
            printf("WARNING: Failed to enable misaligned exception delegate: %s\n",
                   fwft_error_string(ret));
        } else {
            printf("✓ Misaligned exception delegate enabled for debugging\n");
        }
    }

    // 2. 禁用影子栈（简化调试）
    if (fwft_is_supported(FWFT_SHADOW_STACK)) {
        int ret = fwft_set_value(FWFT_SHADOW_STACK, 0);
        if (ret != 0) {
            printf("WARNING: Failed to disable shadow stack: %s\n",
                   fwft_error_string(ret));
        } else {
            printf("✓ Shadow stack disabled for debugging\n");
        }
    }

    // 3. 禁用指针掩码（简化内存访问调试）
    if (fwft_is_supported(FWFT_POINTER_MASKING_PMLEN)) {
        int ret = fwft_set_value(FWFT_POINTER_MASKING_PMLEN, 0);
        if (ret != 0) {
            printf("WARNING: Failed to disable pointer masking: %s\n",
                   fwft_error_string(ret));
        } else {
            printf("✓ Pointer masking disabled for debugging\n");
        }
    }

    printf("Debug configuration completed\n");
    return 0;
}
```

## 高级示例

### 1. 动态特性管理

```c
// 特性配置状态
struct fwft_config {
    uint32_t feature;
    unsigned long value;
    bool lock;
    const char *name;
    const char *description;
};

// 特性配置集合
static const struct fwft_config security_config[] = {
    {FWFT_SHADOW_STACK, 1, true, "Shadow Stack", "Return address protection"},
    {FWFT_LANDING_PAD, 1, true, "Landing Pad", "Control flow protection"},
    {FWFT_DOUBLE_TRAP, 1, false, "Double Trap", "Nested exception handling"},
    {FWFT_POINTER_MASKING_PMLEN, 16, false, "Pointer Masking", "Pointer protection"},
};

static const struct fwft_config performance_config[] = {
    {FWFT_PTE_AD_HW_UPDATING, 1, false, "Hardware A/D", "Page table maintenance"},
    {FWFT_MISALIGNED_EXC_DELEG, 0, false, "Exception Delegate", "Misaligned access handling"},
};

// 应用特性配置
int apply_feature_config(const struct fwft_config *config, int count) {
    int applied = 0;
    int failed = 0;

    printf("Applying feature configuration (%d items)...\n", count);

    for (int i = 0; i < count; i++) {
        printf("\nProcessing: %s\n", config[i].name);
        printf("  Description: %s\n", config[i].description);
        printf("  Feature ID:  0x%08x\n", config[i].feature);
        printf("  Target Value: %lu\n", config[i].value);
        printf("  Lock:        %s\n", config[i].lock ? "Yes" : "No");

        // 检查特性支持
        if (!fwft_is_supported(config[i].feature)) {
            printf("  Result:      ⚠ Not supported\n");
            failed++;
            continue;
        }

        // 获取当前值
        unsigned long current = fwft_get_value(config[i].feature);
        printf("  Current:     %lu\n", current);

        // 如果值相同，跳过
        if (current == config[i].value) {
            printf("  Result:      ✓ Already configured\n");
            applied++;
            continue;
        }

        // 设置新值
        int ret = config[i].lock ?
                  fwft_set_and_lock(config[i].feature, config[i].value) :
                  fwft_set_value(config[i].feature, config[i].value);

        if (ret == 0) {
            printf("  Result:      ✓ Successfully applied%s\n",
                   config[i].lock ? " (locked)" : "");
            applied++;
        } else {
            printf("  Result:      ✗ Failed: %s\n", fwft_error_string(ret));
            failed++;
        }
    }

    printf("\nConfiguration summary:\n");
    printf("  Applied: %d\n", applied);
    printf("  Failed:  %d\n", failed);
    printf("  Total:   %d\n", count);

    return failed == 0 ? 0 : -1;
}

// 应用安全配置
int apply_security_profile(void) {
    return apply_feature_config(security_config, ARRAY_SIZE(security_config));
}

// 应用性能配置
int apply_performance_profile(void) {
    return apply_feature_config(performance_config, ARRAY_SIZE(performance_config));
}
```

### 2. 特性依赖管理

```c
// 特性依赖关系
struct feature_dependency {
    uint32_t feature;
    uint32_t dependency;
    unsigned long required_dep_value;
    const char *description;
};

// 定义特性依赖
static const struct feature_dependency dependencies[] = {
    {FWFT_SHADOW_STACK, FWFT_LANDING_PAD, 1,
     "Shadow stack requires landing pad support"},
    {FWFT_POINTER_MASKING_PMLEN, FWFT_SHADOW_STACK, 0,
     "Pointer masking may conflict with shadow stack"},
};

// 检查特性依赖
bool check_feature_dependencies(uint32_t feature, unsigned long value) {
    for (int i = 0; i < ARRAY_SIZE(dependencies); i++) {
        if (dependencies[i].feature != feature) {
            continue;
        }

        // 检查依赖特性是否支持
        if (!fwft_is_supported(dependencies[i].dependency)) {
            printf("Dependency check failed: %s\n", dependencies[i].description);
            printf("  Dependency feature not supported\n");
            return false;
        }

        // 检查依赖值
        unsigned long dep_value = fwft_get_value(dependencies[i].dependency);
        if (dep_value != dependencies[i].required_dep_value) {
            printf("Dependency check failed: %s\n", dependencies[i].description);
            printf("  Current dependency value: %lu\n", dep_value);
            printf("  Required dependency value: %lu\n",
                   dependencies[i].required_dep_value);
            return false;
        }

        printf("Dependency check passed: %s\n", dependencies[i].description);
    }

    return true;
}

// 智能特性设置
int smart_set_feature(uint32_t feature, unsigned long value, bool lock) {
    printf("Smart setting feature %s to %lu\n", fwft_feature_name(feature), value);

    // 1. 检查特性支持
    if (!fwft_is_supported(feature)) {
        printf("  Feature not supported\n");
        return -ENOTSUP;
    }

    // 2. 检查依赖关系
    if (!check_feature_dependencies(feature, value)) {
        printf("  Dependency requirements not met\n");
        return -EINVAL;
    }

    // 3. 设置特性
    int ret = lock ? fwft_set_and_lock(feature, value) : fwft_set_value(feature, value);
    if (ret != 0) {
        printf("  Failed to set feature: %s\n", fwft_error_string(ret));
        return ret;
    }

    printf("  Feature set successfully%s\n", lock ? " (locked)" : "");
    return 0;
}
```

### 3. 特性状态监控

```c
// 特性状态结构
struct feature_monitor {
    uint32_t feature;
    unsigned long last_value;
    unsigned long check_count;
    unsigned long change_count;
    unsigned long last_change_time;
    const char *name;
};

static struct feature_monitor monitored_features[10];
static int monitored_count = 0;

// 添加监控特性
void add_monitored_feature(uint32_t feature, const char *name) {
    if (monitored_count >= ARRAY_SIZE(monitored_features)) {
        printf("Maximum monitored features reached\n");
        return;
    }

    struct feature_monitor *mon = &monitored_features[monitored_count];
    mon->feature = feature;
    mon->name = name ? name : fwft_feature_name(feature);
    mon->last_value = fwft_get_value(feature);
    mon->check_count = 1;
    mon->change_count = 0;
    mon->last_change_time = get_timestamp();

    monitored_count++;
    printf("Added monitor for %s (initial value: %lu)\n", mon->name, mon->last_value);
}

// 检查特性变化
void check_feature_changes(void) {
    static unsigned long last_check_time = 0;
    unsigned long current_time = get_timestamp();

    // 每10秒检查一次
    if (current_time - last_check_time < 10000000) {
        return;
    }

    last_check_time = current_time;

    for (int i = 0; i < monitored_count; i++) {
        struct feature_monitor *mon = &monitored_features[i];

        if (!fwft_is_supported(mon->feature)) {
            continue;
        }

        unsigned long current_value = fwft_get_value(mon->feature);
        mon->check_count++;

        if (current_value != mon->last_value) {
            mon->change_count++;
            printf("Feature change detected:\n");
            printf("  Feature: %s\n", mon->name);
            printf("  Old Value: %lu\n", mon->last_value);
            printf("  New Value: %lu\n", current_value);
            printf("  Time: %lu\n", current_time);
            printf("  Change Count: %lu\n", mon->change_count);
            printf("  Check Count: %lu\n", mon->check_count);

            mon->last_value = current_value;
            mon->last_change_time = current_time;
        }
    }
}

// 生成监控报告
void generate_monitor_report(void) {
    printf("\n=== Feature Monitor Report ===\n");
    printf("Report Time: %lu\n", get_timestamp());
    printf("Monitored Features: %d\n\n", monitored_count);

    for (int i = 0; i < monitored_count; i++) {
        struct feature_monitor *mon = &monitored_features[i];

        printf("Feature: %s\n", mon->name);
        printf("  ID:           0x%08x\n", mon->feature);
        printf("  Current Value:%lu\n", mon->last_value);
        printf("  Check Count:  %lu\n", mon->check_count);
        printf("  Change Count: %lu\n", mon->change_count);

        if (mon->check_count > 0) {
            printf("  Change Rate:  %.2f%%\n",
                   100.0 * mon->change_count / mon->check_count);
        }

        if (mon->last_change_time > 0) {
            printf("  Last Change: %lu\n", mon->last_change_time);
        }

        printf("\n");
    }
}
```

## 性能优化

### 1. 批量操作优化

```c
// 批量特性设置
struct batch_feature_op {
    uint32_t feature;
    unsigned long value;
    bool lock;
    int (*pre_check)(uint32_t, unsigned long);
    void (*post_action)(uint32_t, unsigned long, int);
};

// 示例预检查函数
int check_security_impact(uint32_t feature, unsigned long value) {
    // 检查安全影响
    if (feature == FWFT_SHADOW_STACK && value == 0) {
        printf("WARNING: Disabling shadow stack may reduce security\n");
        return 0; // 允许但警告
    }
    return 1; // 允许
}

// 示例后处理函数
void log_feature_change(uint32_t feature, unsigned long value, int result) {
    printf("Feature %s set to %lu: %s\n",
           fwft_feature_name(feature), value,
           result == 0 ? "SUCCESS" : "FAILED");
}

// 批量执行特性操作
int batch_feature_operations(const struct batch_feature_op *ops, int count) {
    int success = 0;
    int failed = 0;

    printf("Executing %d feature operations...\n", count);

    for (int i = 0; i < count; i++) {
        const struct batch_feature_op *op = &ops[i];

        // 预检查
        if (op->pre_check && !op->pre_check(op->feature, op->value)) {
            printf("Pre-check failed for %s\n", fwft_feature_name(op->feature));
            failed++;
            continue;
        }

        // 执行操作
        int result = op->lock ?
                     fwft_set_and_lock(op->feature, op->value) :
                     fwft_set_value(op->feature, op->value);

        if (result == 0) {
            success++;
        } else {
            failed++;
        }

        // 后处理
        if (op->post_action) {
            op->post_action(op->feature, op->value, result);
        }
    }

    printf("Batch operation completed: %d success, %d failed\n", success, failed);
    return failed == 0 ? 0 : -1;
}

// 使用示例
int example_batch_operation(void) {
    const struct batch_feature_op ops[] = {
        {FWFT_SHADOW_STACK, 1, false, check_security_impact, log_feature_change},
        {FWFT_LANDING_PAD, 1, true, check_security_impact, log_feature_change},
        {FWFT_PTE_AD_HW_UPDATING, 1, false, NULL, log_feature_change},
    };

    return batch_feature_operations(ops, ARRAY_SIZE(ops));
}
```

### 2. 缓存优化

```c
// 特性缓存结构
struct feature_cache {
    bool valid;
    uint32_t feature;
    unsigned long value;
    unsigned long timestamp;
    bool locked;
};

#define FEATURE_CACHE_SIZE 32

static struct feature_cache feature_cache[FEATURE_CACHE_SIZE];
static int cache_initialized = 0;

// 初始化缓存
void init_feature_cache(void) {
    if (cache_initialized) {
        return;
    }

    for (int i = 0; i < FEATURE_CACHE_SIZE; i++) {
        feature_cache[i].valid = false;
    }

    cache_initialized = 1;
    printf("Feature cache initialized\n");
}

// 查找缓存
static struct feature_cache* find_in_cache(uint32_t feature) {
    for (int i = 0; i < FEATURE_CACHE_SIZE; i++) {
        if (feature_cache[i].valid && feature_cache[i].feature == feature) {
            return &feature_cache[i];
        }
    }
    return NULL;
}

// 添加到缓存
static void add_to_cache(uint32_t feature, unsigned long value) {
    static int next_slot = 0;

    struct feature_cache *cache = &feature_cache[next_slot];
    cache->valid = true;
    cache->feature = feature;
    cache->value = value;
    cache->timestamp = get_timestamp();
    cache->locked = false;

    next_slot = (next_slot + 1) % FEATURE_CACHE_SIZE;
}

// 缓存友好的特性查询
unsigned long cached_get_feature(uint32_t feature) {
    init_feature_cache();

    struct feature_cache *cache = find_in_cache(feature);
    if (cache) {
        // 简单的缓存有效性检查（1秒）
        if (get_timestamp() - cache->timestamp < 1000000) {
            return cache->value;
        }
    }

    // 缓存未命中或过期，查询 SBI
    unsigned long value = fwft_get_value(feature);
    add_to_cache(feature, value);

    return value;
}

// 缓存失效
void invalidate_cache(void) {
    for (int i = 0; i < FEATURE_CACHE_SIZE; i++) {
        feature_cache[i].valid = false;
    }
    printf("Feature cache invalidated\n");
}

// 缓存友好的特性设置
int cached_set_feature(uint32_t feature, unsigned long value, bool lock) {
    int result = lock ? fwft_set_and_lock(feature, value) : fwft_set_value(feature, value);

    if (result == 0) {
        // 更新缓存
        struct feature_cache *cache = find_in_cache(feature);
        if (cache) {
            cache->value = value;
            cache->timestamp = get_timestamp();
            if (lock) {
                cache->locked = true;
            }
        } else {
            add_to_cache(feature, value);
        }
    }

    return result;
}
```

## 调试和故障排除

### 1. 调试工具

```c
// 特性调试器
struct fwft_debugger {
    bool trace_enabled;
    bool verbose_enabled;
    FILE *log_file;
};

static struct fwft_debugger debugger = {
    .trace_enabled = false,
    .verbose_enabled = false,
    .log_file = NULL
};

// 启用调试
void enable_fwft_debugging(bool trace, bool verbose, const char *log_file_path) {
    debugger.trace_enabled = trace;
    debugger.verbose_enabled = verbose;

    if (log_file_path) {
        debugger.log_file = fopen(log_file_path, "a");
        if (debugger.log_file) {
            fprintf(debugger.log_file, "\n=== FWFT Debug Session Started ===\n");
            fprintf(debugger.log_file, "Time: %lu\n", get_timestamp());
        }
    }

    printf("FWFT debugging enabled (trace: %s, verbose: %s)\n",
           trace ? "yes" : "no", verbose ? "yes" : "no");
}

// 调试日志
void debug_log(const char *format, ...) {
    if (!debugger.trace_enabled && !debugger.verbose_enabled) {
        return;
    }

    va_list args;
    va_start(args, format);

    if (debugger.verbose_enabled) {
        vprintf(format, args);
    }

    if (debugger.log_file) {
        vfprintf(debugger.log_file, format, args);
        fflush(debugger.log_file);
    }

    va_end(args);
}

// 调试版本的特性设置
int debug_set_feature(uint32_t feature, unsigned long value, bool lock) {
    debug_log("Setting feature %s (0x%08x) to %lu%s\n",
              fwft_feature_name(feature), feature, value,
              lock ? " (lock)" : "");

    // 记录设置前状态
    unsigned long old_value = 0;
    bool supported = fwft_is_supported(feature);
    if (supported) {
        old_value = fwft_get_value(feature);
    }

    debug_log("  Supported: %s\n", supported ? "yes" : "no");
    if (supported) {
        debug_log("  Old value: %lu\n", old_value);
    }

    // 执行设置
    int result = lock ? fwft_set_and_lock(feature, value) : fwft_set_value(feature, value);

    debug_log("  Result: %s\n", result == 0 ? "SUCCESS" : fwft_error_string(result));

    // 记录设置后状态
    if (supported && result == 0) {
        unsigned long new_value = fwft_get_value(feature);
        debug_log("  New value: %lu\n", new_value);

        if (new_value != value) {
            debug_log("  WARNING: Value verification failed!\n");
        }
    }

    debug_log("\n");

    return result;
}
```

### 2. 错误恢复

```c
// 特性回滚点
struct rollback_point {
    uint32_t feature;
    unsigned long original_value;
    bool was_modified;
    const char *name;
};

#define MAX_ROLLBACK_POINTS 16

static struct rollback_point rollback_points[MAX_ROLLBACK_POINTS];
static int rollback_count = 0;

// 创建回滚点
int create_rollback_point(uint32_t feature, const char *name) {
    if (rollback_count >= MAX_ROLLBACK_POINTS) {
        printf("Maximum rollback points reached\n");
        return -1;
    }

    if (!fwft_is_supported(feature)) {
        printf("Feature not supported for rollback: %s\n",
               name ? name : fwft_feature_name(feature));
        return -1;
    }

    struct rollback_point *rp = &rollback_points[rollback_count];
    rp->feature = feature;
    rp->original_value = fwft_get_value(feature);
    rp->was_modified = false;
    rp->name = name ? name : fwft_feature_name(feature);

    rollback_count++;
    return rollback_count - 1;
}

// 回滚特性
int rollback_feature(int rollback_id) {
    if (rollback_id < 0 || rollback_id >= rollback_count) {
        printf("Invalid rollback ID: %d\n", rollback_id);
        return -1;
    }

    struct rollback_point *rp = &rollback_points[rollback_id];

    if (!rp->was_modified) {
        printf("Feature %s was not modified, no rollback needed\n", rp->name);
        return 0;
    }

    int ret = fwft_set_value(rp->feature, rp->original_value);
    if (ret == 0) {
        printf("Successfully rolled back %s to %lu\n", rp->name, rp->original_value);
    } else {
        printf("Failed to rollback %s: %s\n", rp->name, fwft_error_string(ret));
    }

    return ret;
}

// 回滚所有修改
int rollback_all(void) {
    int success = 0;
    int failed = 0;

    printf("Rolling back all modifications (%d points)...\n", rollback_count);

    for (int i = rollback_count - 1; i >= 0; i--) {
        struct rollback_point *rp = &rollback_points[i];

        if (!rp->was_modified) {
            continue;
        }

        int ret = fwft_set_value(rp->feature, rp->original_value);
        if (ret == 0) {
            printf("✓ Rolled back %s\n", rp->name);
            success++;
        } else {
            printf("✗ Failed to rollback %s: %s\n", rp->name, fwft_error_string(ret));
            failed++;
        }
    }

    printf("Rollback completed: %d success, %d failed\n", success, failed);
    rollback_count = 0;

    return failed == 0 ? 0 : -1;
}

// 带回滚的特性设置
int rollback_set_feature(uint32_t feature, unsigned long value, bool lock,
                        const char *name) {
    // 创建回滚点
    int rollback_id = create_rollback_point(feature, name);
    if (rollback_id < 0) {
        return rollback_id;
    }

    // 尝试设置特性
    int ret = lock ? fwft_set_and_lock(feature, value) : fwft_set_value(feature, value);

    if (ret != 0) {
        // 设置失败，清理回滚点
        rollback_count--;
        printf("Failed to set %s: %s\n", name, fwft_error_string(ret));
        return ret;
    }

    // 标记已修改
    rollback_points[rollback_id].was_modified = true;
    printf("Set %s to %lu (rollback_id: %d)\n", name, value, rollback_id);

    return rollback_id;
}
```

## 安全最佳实践

### 1. 安全配置模板

```c
// 安全级别定义
enum security_level {
    SECURITY_LEVEL_MINIMAL,    // 最小安全保护
    SECURITY_LEVEL_STANDARD,   // 标准安全保护
    SECURITY_LEVEL_HIGH,       // 高安全保护
    SECURITY_LEVEL_MAXIMUM     // 最大安全保护
};

// 安全配置模板
struct security_profile {
    enum security_level level;
    bool enable_shadow_stack;
    bool enable_landing_pad;
    bool enable_double_trap;
    bool enable_pointer_masking;
    unsigned long pointer_masking_pmlen;
    bool lock_features;
    const char *name;
};

static const struct security_profile security_profiles[] = {
    [SECURITY_LEVEL_MINIMAL] = {
        .level = SECURITY_LEVEL_MINIMAL,
        .enable_shadow_stack = false,
        .enable_landing_pad = false,
        .enable_double_trap = false,
        .enable_pointer_masking = false,
        .pointer_masking_pmlen = 0,
        .lock_features = false,
        .name = "Minimal Security"
    },
    [SECURITY_LEVEL_STANDARD] = {
        .level = SECURITY_LEVEL_STANDARD,
        .enable_shadow_stack = true,
        .enable_landing_pad = true,
        .enable_double_trap = true,
        .enable_pointer_masking = false,
        .pointer_masking_pmlen = 0,
        .lock_features = true,
        .name = "Standard Security"
    },
    [SECURITY_LEVEL_HIGH] = {
        .level = SECURITY_LEVEL_HIGH,
        .enable_shadow_stack = true,
        .enable_landing_pad = true,
        .enable_double_trap = true,
        .enable_pointer_masking = true,
        .pointer_masking_pmlen = 16,
        .lock_features = true,
        .name = "High Security"
    },
    [SECURITY_LEVEL_MAXIMUM] = {
        .level = SECURITY_LEVEL_MAXIMUM,
        .enable_shadow_stack = true,
        .enable_landing_pad = true,
        .enable_double_trap = true,
        .enable_pointer_masking = true,
        .pointer_masking_pmlen = 32,
        .lock_features = true,
        .name = "Maximum Security"
    }
};

// 应用安全配置
int apply_security_profile(enum security_level level) {
    if (level < 0 || level >= ARRAY_SIZE(security_profiles)) {
        printf("Invalid security level: %d\n", level);
        return -EINVAL;
    }

    const struct security_profile *profile = &security_profiles[level];

    printf("Applying security profile: %s\n", profile->name);

    // 1. 配置影子栈
    if (profile->enable_shadow_stack && fwft_is_supported(FWFT_SHADOW_STACK)) {
        int ret = profile->lock_features ?
                  fwft_set_and_lock(FWFT_SHADOW_STACK, 1) :
                  fwft_set_value(FWFT_SHADOW_STACK, 1);
        if (ret != 0) {
            printf("Failed to enable shadow stack: %s\n", fwft_error_string(ret));
            return ret;
        }
        printf("✓ Shadow stack enabled%s\n", profile->lock_features ? " (locked)" : "");
    }

    // 2. 配置着陆垫
    if (profile->enable_landing_pad && fwft_is_supported(FWFT_LANDING_PAD)) {
        int ret = profile->lock_features ?
                  fwft_set_and_lock(FWFT_LANDING_PAD, 1) :
                  fwft_set_value(FWFT_LANDING_PAD, 1);
        if (ret != 0) {
            printf("Failed to enable landing pad: %s\n", fwft_error_string(ret));
            return ret;
        }
        printf("✓ Landing pad enabled%s\n", profile->lock_features ? " (locked)" : "");
    }

    // 3. 配置双重陷阱
    if (profile->enable_double_trap && fwft_is_supported(FWFT_DOUBLE_TRAP)) {
        int ret = fwft_set_value(FWFT_DOUBLE_TRAP, 1);
        if (ret != 0) {
            printf("Failed to enable double trap: %s\n", fwft_error_string(ret));
            return ret;
        }
        printf("✓ Double trap enabled\n");
    }

    // 4. 配置指针掩码
    if (profile->enable_pointer_masking && fwft_is_supported(FWFT_POINTER_MASKING_PMLEN)) {
        int ret = profile->lock_features ?
                  fwft_set_and_lock(FWFT_POINTER_MASKING_PMLEN, profile->pointer_masking_pmlen) :
                  fwft_set_value(FWFT_POINTER_MASKING_PMLEN, profile->pointer_masking_pmlen);
        if (ret != 0) {
            printf("Failed to set pointer masking: %s\n", fwft_error_string(ret));
            return ret;
        }
        printf("✓ Pointer masking set to %lu bits%s\n",
               profile->pointer_masking_pmlen,
               profile->lock_features ? " (locked)" : "");
    }

    printf("Security profile %s applied successfully\n", profile->name);
    return 0;
}
```

## 虚拟化环境使用

### 1. 虚拟机特性管理

```c
// 虚拟机特性配置
struct vm_feature_config {
    unsigned long vm_id;
    bool enable_shadow_stack;
    bool enable_landing_pad;
    bool enable_misaligned_delegate;
    unsigned long pointer_masking_pmlen;
    bool lock_after_config;
};

// 配置虚拟机特性
int configure_vm_features(const struct vm_feature_config *config) {
    printf("Configuring features for VM %lu\n", config->vm_id);

    // 验证虚拟机权限
    if (!has_vm_permission(config->vm_id)) {
        printf("No permission to configure VM %lu\n", config->vm_id);
        return -EPERM;
    }

    // 记录配置操作
    printf("VM %lu configuration:\n", config->vm_id);
    printf("  Shadow Stack: %s\n", config->enable_shadow_stack ? "Enable" : "Disable");
    printf("  Landing Pad: %s\n", config->enable_landing_pad ? "Enable" : "Disable");
    printf("  Misaligned Delegate: %s\n", config->enable_misaligned_delegate ? "Enable" : "Disable");
    printf("  Pointer Masking PMLEN: %lu\n", config->pointer_masking_pmlen);

    // 应用配置
    int errors = 0;

    if (fwft_is_supported(FWFT_SHADOW_STACK)) {
        int ret = fwft_set_value(FWFT_SHADOW_STACK, config->enable_shadow_stack ? 1 : 0);
        if (ret != 0) {
            printf("Failed to configure shadow stack for VM %lu: %s\n",
                   config->vm_id, fwft_error_string(ret));
            errors++;
        }
    }

    if (fwft_is_supported(FWFT_LANDING_PAD)) {
        int ret = fwft_set_value(FWFT_LANDING_PAD, config->enable_landing_pad ? 1 : 0);
        if (ret != 0) {
            printf("Failed to configure landing pad for VM %lu: %s\n",
                   config->vm_id, fwft_error_string(ret));
            errors++;
        }
    }

    if (fwft_is_supported(FWFT_MISALIGNED_EXC_DELEG)) {
        int ret = fwft_set_value(FWFT_MISALIGNED_EXC_DELEG,
                                 config->enable_misaligned_delegate ? 1 : 0);
        if (ret != 0) {
            printf("Failed to configure misaligned delegate for VM %lu: %s\n",
                   config->vm_id, fwft_error_string(ret));
            errors++;
        }
    }

    if (fwft_is_supported(FWFT_POINTER_MASKING_PMLEN)) {
        int ret = fwft_set_value(FWFT_POINTER_MASKING_PMLEN,
                                 config->pointer_masking_pmlen);
        if (ret != 0) {
            printf("Failed to configure pointer masking for VM %lu: %s\n",
                   config->vm_id, fwft_error_string(ret));
            errors++;
        }
    }

    // 如果需要，锁定配置
    if (config->lock_after_config && errors == 0) {
        printf("Locking VM %lu configuration\n", config->vm_id);
        // 这里可以实现特性锁定逻辑
    }

    return errors == 0 ? 0 : -1;
}
```

## 常见问题

### Q1: 特性设置失败怎么办？

**A**: 首先检查错误码：
- `SBI_ERR_NOT_SUPPORTED`: 特性不支持
- `SBI_ERR_DENIED_LOCKED`: 特性已锁定
- `SBI_ERR_INVALID_PARAM`: 参数无效
- `SBI_ERR_DENIED`: 权限不足

```c
// 处理特性设置失败
int handle_feature_set_failure(uint32_t feature, int error) {
    switch (error) {
        case SBI_ERR_NOT_SUPPORTED:
            printf("Feature not supported by hardware/firmware\n");
            printf("Consider alternative implementation\n");
            return -ENOTSUP;

        case SBI_ERR_DENIED_LOCKED:
            printf("Feature is locked, cannot modify\n");
            printf("System restart may be required\n");
            return -EPERM;

        case SBI_ERR_INVALID_PARAM:
            printf("Invalid parameter value\n");
            printf("Check feature documentation for valid values\n");
            return -EINVAL;

        case SBI_ERR_DENIED:
            printf("Permission denied\n");
            printf("Check privilege level and security policies\n");
            return -EPERM;

        default:
            printf("Unknown error occurred: %d\n", error);
            return -EIO;
    }
}
```

### Q2: 如何处理特性依赖？

**A**: 实现依赖检查和管理：

```c
// 自动处理特性依赖
int auto_handle_dependencies(uint32_t feature, unsigned long value) {
    // 如果启用影子栈，确保着陆垫也启用
    if (feature == FWFT_SHADOW_STACK && value == 1) {
        if (fwft_is_supported(FWFT_LANDING_PAD)) {
            unsigned long landing_pad = fwft_get_value(FWFT_LANDING_PAD);
            if (landing_pad == 0) {
                printf("Enabling landing pad for shadow stack compatibility\n");
                int ret = fwft_set_value(FWFT_LANDING_PAD, 1);
                if (ret != 0) {
                    return ret;
                }
            }
        }
    }

    return 0;
}
```

### Q3: 如何优化性能？

**A**: 使用缓存和批量操作：

```c
// 性能优化建议
void performance_optimization_tips(void) {
    printf("=== FWFT Performance Optimization Tips ===\n");
    printf("1. Use caching for frequently accessed features\n");
    printf("2. Batch multiple feature operations together\n");
    printf("3. Avoid redundant queries\n");
    printf("4. Consider the impact of security features on performance\n");
    printf("5. Monitor feature usage patterns\n");
}
```

## 参考资料

- [RISC-V SBI 规范](https://github.com/riscv-non-isa/riscv-sbi-doc)
- [FWFT 扩展规范](https://github.com/riscv-non-isa/riscv-sbi-doc/blob/master/src/ext-firmware-features.adoc)
- [RISC-V 安全扩展](https://github.com/riscv/riscv-isa-manual)
- [虚拟化最佳实践](https://riscv.org/virtualization/)