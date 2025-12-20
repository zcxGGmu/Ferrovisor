# RISC-V SBI 固件特性扩展函数接口

本文档详细描述了 SBI 固件特性扩展（FWFT）提供的所有函数接口，包括参数定义、返回值、错误处理和使用示例。

## 扩展标识符

```
SBI_EXT_FWFT = 0x46574654  // "FWFT"
```

## 函数概览

| 函数 ID | 函数名 | 描述 |
|---------|--------|------|
| 0 | sbi_fwft_set | 设置特性值 |
| 1 | sbi_fwft_get | 查询特性值 |

## 核心函数接口

### 1. sbi_fwft_set() - 设置特性值

设置指定的固件特性值，并可选择是否锁定该特性。

#### 函数原型

```c
struct sbiret sbi_fwft_set(uint32_t feature,
                          unsigned long value,
                          unsigned long flags);
```

#### 参数说明

**feature (uint32_t)**
- 特性标识符
- 标准特性范围：0x00000000 - 0x00000005
- 平台特性范围：0x40000000 - 0x7FFFFFFF
- 其他值：保留

**value (unsigned long)**
- 要设置的特性值
- 具体含义和取值范围取决于特性
- 值为 0 通常表示禁用该特性

**flags (unsigned long)**
- 控制标志位
- 目前只定义了锁定标志
- 其他位保留，必须为 0

#### 标志位定义

```c
#define FWFT_FLAG_LOCK    (1UL << 0)  // 锁定特性
```

- **BIT[0] (LOCK)**：设置后特性不能再修改
- **BIT[1:63]**：保留位，必须为 0

#### 返回值

**struct sbiret**
- **error**：错误码
- **value**：保留，必须为 0

#### 错误码

| 错误码 | 数值 | 描述 |
|--------|------|------|
| SBI_SUCCESS | 0 | 设置成功 |
| SBI_ERR_NOT_SUPPORTED | -2 | 平台不支持该特性 |
| SBI_ERR_INVALID_PARAM | -3 | 无效的 value 或 flags |
| SBI_ERR_DENIED | -4 | SBI 实现拒绝设置 |
| SBI_ERR_DENIED_LOCKED | -4 | 特性已被锁定 |
| SBI_ERR_FAILED | -1 | 其他未指定错误 |

#### 使用示例

```c
#include <sbi/sbi.h>

// 启用影子栈
void enable_shadow_stack(void) {
    struct sbiret ret;

    // 查询影子栈是否支持
    ret = sbi_fwft_get(SHADOW_STACK);
    if (ret.error == SBI_SUCCESS) {
        printf("Shadow stack supported, current value: %lu\n", ret.value);

        // 启用影子栈
        ret = sbi_fwft_set(SHADOW_STACK, 1, 0);
        if (ret.error == SBI_SUCCESS) {
            printf("Shadow stack enabled successfully\n");
        } else if (ret.error == SBI_ERR_DENIED_LOCKED) {
            printf("Shadow stack is locked, cannot modify\n");
        } else {
            printf("Failed to enable shadow stack: %ld\n", ret.error);
        }
    } else {
        printf("Shadow stack not supported\n");
    }
}

// 锁定特性设置
void lock_shadow_stack(void) {
    struct sbiret ret = sbi_fwft_set(SHADOW_STACK, 1, FWFT_FLAG_LOCK);

    if (ret.error == SBI_SUCCESS) {
        printf("Shadow stack enabled and locked\n");
    } else {
        printf("Failed to enable and lock shadow stack: %ld\n", ret.error);
    }
}
```

### 2. sbi_fwft_get() - 查询特性值

查询指定的固件特性当前值和状态。

#### 函数原型

```c
struct sbiret sbi_fwft_get(uint32_t feature);
```

#### 参数说明

**feature (uint32_t)**
- 特性标识符
- 支持 sbi_fwft_set() 中的所有特性 ID

#### 返回值

**struct sbiret**
- **error**：错误码
- **value**：特性当前值

#### 错误码

| 错误码 | 数值 | 描述 |
|--------|------|------|
| SBI_SUCCESS | 0 | 查询成功 |
| SBI_ERR_NOT_SUPPORTED | -2 | 平台不支持该特性 |
| SBI_ERR_DENIED | -4 | 特性未实现 |
| SBI_ERR_FAILED | -1 | 其他未指定错误 |

#### 使用示例

```c
#include <sbi/sbi.h>

// 查询所有标准特性
void query_all_features(void) {
    const struct {
        uint32_t id;
        const char *name;
    } features[] = {
        {MISALIGNED_EXC_DELEG, "Misaligned Exception Delegate"},
        {LANDING_PAD, "Landing Pad"},
        {SHADOW_STACK, "Shadow Stack"},
        {DOUBLE_TRAP, "Double Trap"},
        {PTE_AD_HW_UPDATING, "PTE A/D Hardware Updating"},
        {POINTER_MASKING_PMLEN, "Pointer Masking PMLEN"}
    };

    printf("=== Firmware Features Status ===\n");

    for (int i = 0; i < ARRAY_SIZE(features); i++) {
        struct sbiret ret = sbi_fwft_get(features[i].id);

        if (ret.error == SBI_SUCCESS) {
            printf("%-30s: %lu\n", features[i].name, ret.value);
        } else if (ret.error == SBI_ERR_NOT_SUPPORTED) {
            printf("%-30s: Not Supported\n", features[i].name);
        } else {
            printf("%-30s: Error %ld\n", features[i].name, ret.error);
        }
    }
}

// 检查特性是否支持
bool is_feature_supported(uint32_t feature) {
    struct sbiret ret = sbi_fwft_get(feature);
    return (ret.error == SBI_SUCCESS);
}

// 获取特性值
unsigned long get_feature_value(uint32_t feature) {
    struct sbiret ret = sbi_fwft_get(feature);
    return (ret.error == SBI_SUCCESS) ? ret.value : 0;
}
```

## 标准特性 ID 定义

```c
// SBI 固件特性扩展 ID
#define SBI_EXT_FWFT                    0x46574654

// 函数 ID
#define SBI_FWFT_SET                    0
#define SBI_FWFT_GET                    1

// 标准特性 ID
#define FWFT_MISALIGNED_EXC_DELEG       0x00000000
#define FWFT_LANDING_PAD                0x00000001
#define FWFT_SHADOW_STACK               0x00000002
#define FWFT_DOUBLE_TRAP                0x00000003
#define FWFT_PTE_AD_HW_UPDATING         0x00000004
#define FWFT_POINTER_MASKING_PMLEN      0x00000005

// 标志位定义
#define FWFT_FLAG_LOCK                  (1UL << 0)
```

## 特性值定义

### MISALIGNED_EXC_DELEG

```c
#define MISALIGNED_EXC_DELEG_DISABLED   0
#define MISALIGNED_EXC_DELEG_ENABLED    1
```

### LANDING_PAD

```c
#define LANDING_PAD_DISABLED            0
#define LANDING_PAD_ENABLED             1
```

### SHADOW_STACK

```c
#define SHADOW_STACK_DISABLED           0
#define SHADOW_STACK_ENABLED            1
```

### DOUBLE_TRAP

```c
#define DOUBLE_TRAP_DISABLED            0
#define DOUBLE_TRAP_ENABLED             1
```

### PTE_AD_HW_UPDATING

```c
#define PTE_AD_HW_UPDATING_DISABLED     0
#define PTE_AD_HW_UPDATING_ENABLED      1
```

### POINTER_MASKING_PMLEN

```c
#define POINTER_MASKING_DISABLED        0
#define POINTER_MASKING_PMLEN_MIN       8
#define POINTER_MASKING_PMLEN_MAX       32  // 平台相关
```

## 完整接口封装示例

```c
// fwft.h - 固件特性扩展接口封装
#ifndef FWFT_H
#define FWFT_H

#include <sbi/sbi.h>

// 特性查询
static inline bool fwft_is_supported(uint32_t feature) {
    struct sbiret ret = sbi_fwft_get(feature);
    return ret.error == SBI_SUCCESS;
}

static inline unsigned long fwft_get_value(uint32_t feature) {
    struct sbiret ret = sbi_fwft_get(feature);
    return (ret.error == SBI_SUCCESS) ? ret.value : 0;
}

// 特性设置
static inline int fwft_set_value(uint32_t feature, unsigned long value) {
    struct sbiret ret = sbi_fwft_set(feature, value, 0);
    return (ret.error == SBI_SUCCESS) ? 0 : (int)ret.error;
}

static inline int fwft_set_and_lock(uint32_t feature, unsigned long value) {
    struct sbiret ret = sbi_fwft_set(feature, value, FWFT_FLAG_LOCK);
    return (ret.error == SBI_SUCCESS) ? 0 : (int)ret.error;
}

// 错误处理
static inline const char* fwft_error_string(long error) {
    switch (error) {
        case SBI_SUCCESS:              return "Success";
        case SBI_ERR_NOT_SUPPORTED:    return "Not supported";
        case SBI_ERR_INVALID_PARAM:    return "Invalid parameter";
        case SBI_ERR_DENIED:           return "Denied";
        case SBI_ERR_DENIED_LOCKED:    return "Denied (locked)";
        case SBI_ERR_FAILED:           return "Failed";
        default:                       return "Unknown error";
    }
}

// 特性名称
static inline const char* fwft_feature_name(uint32_t feature) {
    switch (feature) {
        case FWFT_MISALIGNED_EXC_DELEG:   return "Misaligned Exception Delegate";
        case FWFT_LANDING_PAD:             return "Landing Pad";
        case FWFT_SHADOW_STACK:            return "Shadow Stack";
        case FWFT_DOUBLE_TRAP:             return "Double Trap";
        case FWFT_PTE_AD_HW_UPDATING:      return "PTE A/D Hardware Updating";
        case FWFT_POINTER_MASKING_PMLEN:   return "Pointer Masking PMLEN";
        default:                           return "Unknown Feature";
    }
}

#endif // FWFT_H
```

## 高级使用模式

### 批量特性查询

```c
#include "fwft.h"

struct feature_status {
    uint32_t id;
    const char *name;
    bool supported;
    unsigned long value;
    bool locked;
};

int query_all_features(struct feature_status *status, int count) {
    int queried = 0;

    for (int i = 0; i < count && queried < 6; i++) {
        status[queried].id = feature_ids[i];
        status[queried].name = fwft_feature_name(feature_ids[i]);
        status[queried].supported = fwft_is_supported(feature_ids[i]);

        if (status[queried].supported) {
            status[queried].value = fwft_get_value(feature_ids[i]);

            // 检查是否锁定（通过尝试修改来检测）
            if (status[queried].value != 0) {
                int ret = fwft_set_value(feature_ids[i], status[queried].value);
                status[queried].locked = (ret == SBI_ERR_DENIED_LOCKED);
            } else {
                status[queried].locked = false;
            }
        } else {
            status[queried].value = 0;
            status[queried].locked = false;
        }

        queried++;
    }

    return queried;
}
```

### 安全特性配置

```c
#include "fwft.h"

// 配置安全特性
int configure_security_features(void) {
    // 1. 启用影子栈
    if (fwft_is_supported(FWFT_SHADOW_STACK)) {
        int ret = fwft_set_and_lock(FWFT_SHADOW_STACK, 1);
        if (ret != 0) {
            printf("Failed to enable shadow stack: %s\n", fwft_error_string(ret));
            return ret;
        }
        printf("Shadow stack enabled and locked\n");
    }

    // 2. 启用着陆垫
    if (fwft_is_supported(FWFT_LANDING_PAD)) {
        int ret = fwft_set_and_lock(FWFT_LANDING_PAD, 1);
        if (ret != 0) {
            printf("Failed to enable landing pad: %s\n", fwft_error_string(ret));
            return ret;
        }
        printf("Landing pad enabled and locked\n");
    }

    // 3. 启用双重陷阱
    if (fwft_is_supported(FWFT_DOUBLE_TRAP)) {
        int ret = fwft_set_value(FWFT_DOUBLE_TRAP, 1);
        if (ret != 0) {
            printf("Failed to enable double trap: %s\n", fwft_error_string(ret));
            return ret;
        }
        printf("Double trap enabled\n");
    }

    // 4. 配置指针掩码
    if (fwft_is_supported(FWFT_POINTER_MASKING_PMLEN)) {
        int ret = fwft_set_value(FWFT_POINTER_MASKING_PMLEN, 16);
        if (ret != 0) {
            printf("Failed to set pointer masking: %s\n", fwft_error_string(ret));
            return ret;
        }
        printf("Pointer masking PMLEN set to 16\n");
    }

    return 0;
}
```

### 性能优化配置

```c
#include "fwft.h"

// 配置性能优化特性
int configure_performance_features(void) {
    // 1. 禁用非对齐异常委托（如果不需要）
    if (fwft_is_supported(FWFT_MISALIGNED_EXC_DELEG)) {
        int ret = fwft_set_value(FWFT_MISALIGNED_EXC_DELEG, 0);
        if (ret != 0) {
            printf("Failed to disable misaligned exception delegate: %s\n",
                   fwft_error_string(ret));
        } else {
            printf("Misaligned exception delegate disabled\n");
        }
    }

    // 2. 启用硬件 A/D 位更新
    if (fwft_is_supported(FWFT_PTE_AD_HW_UPDATING)) {
        int ret = fwft_set_value(FWFT_PTE_AD_HW_UPDATING, 1);
        if (ret != 0) {
            printf("Failed to enable PTE A/D hardware updating: %s\n",
                   fwft_error_string(ret));
            return ret;
        }
        printf("PTE A/D hardware updating enabled\n");
    }

    return 0;
}
```

## 调试和诊断

### 特性状态报告

```c
#include "fwft.h"

// 生成特性状态报告
void generate_feature_report(void) {
    printf("=== SBI Firmware Features Report ===\n");
    printf("Extension ID: 0x%08x\n", SBI_EXT_FWFT);
    printf("Timestamp: %lu\n", get_timestamp());
    printf("\n");

    const uint32_t features[] = {
        FWFT_MISALIGNED_EXC_DELEG,
        FWFT_LANDING_PAD,
        FWFT_SHADOW_STACK,
        FWFT_DOUBLE_TRAP,
        FWFT_PTE_AD_HW_UPDATING,
        FWFT_POINTER_MASKING_PMLEN
    };

    for (int i = 0; i < ARRAY_SIZE(features); i++) {
        printf("Feature: %s (0x%08x)\n",
               fwft_feature_name(features[i]), features[i]);

        if (fwft_is_supported(features[i])) {
            unsigned long value = fwft_get_value(features[i]);
            printf("  Status:   Supported\n");
            printf("  Value:    %lu\n", value);

            // 特性特定信息
            switch (features[i]) {
                case FWFT_POINTER_MASKING_PMLEN:
                    printf("  PMLEN:    %lu bits\n", value);
                    break;
                case FWFT_PTE_AD_HW_UPDATING:
                    printf("  Hardware Updating: %s\n", value ? "Enabled" : "Disabled");
                    break;
            }
        } else {
            printf("  Status:   Not Supported\n");
        }

        printf("\n");
    }
}
```

## 版本兼容性

### SBI 版本检查

```c
#include "fwft.h"

// 检查 SBI 版本兼容性
bool check_sbi_version(void) {
    struct sbiret ret = sbi_get_sbi_version();

    if (ret.error != SBI_SUCCESS) {
        printf("Failed to get SBI version\n");
        return false;
    }

    // SBI 3.0+ 支持 FWFT
    unsigned long version = ret.value;
    if (version < 0x03000000) {
        printf("SBI version %lu.%lu.%lu does not support FWFT (requires 3.0+)\n",
               (version >> 24) & 0xFF,
               (version >> 16) & 0xFF,
               (version >> 8) & 0xFF);
        return false;
    }

    printf("SBI version %lu.%lu.%lu supports FWFT\n",
           (version >> 24) & 0xFF,
           (version >> 16) & 0xFF,
           (version >> 8) & 0xFF);

    return true;
}
```

## 错误处理最佳实践

### 综合错误处理

```c
#include "fwft.h"

// 健壮的特性设置函数
int safe_set_feature(uint32_t feature, unsigned long value, bool lock) {
    // 1. 检查特性支持
    if (!fwft_is_supported(feature)) {
        printf("Feature %s not supported\n", fwft_feature_name(feature));
        return -ENOTSUP;
    }

    // 2. 获取当前值
    unsigned long current_value = fwft_get_value(feature);
    printf("Current value of %s: %lu\n", fwft_feature_name(feature), current_value);

    // 3. 如果值相同，无需设置
    if (current_value == value) {
        printf("Feature %s already has value %lu\n", fwft_feature_name(feature), value);
        return 0;
    }

    // 4. 设置新值
    int ret = lock ? fwft_set_and_lock(feature, value) : fwft_set_value(feature, value);

    if (ret != 0) {
        printf("Failed to set %s to %lu: %s\n",
               fwft_feature_name(feature), value, fwft_error_string(ret));

        // 5. 错误恢复策略
        switch (ret) {
            case SBI_ERR_DENIED_LOCKED:
                printf("Feature is locked, may require system reset\n");
                break;
            case SBI_ERR_INVALID_PARAM:
                printf("Invalid value %lu for feature\n", value);
                break;
            case SBI_ERR_DENIED:
                printf("SBI implementation denied the request\n");
                break;
            default:
                printf("Unexpected error occurred\n");
                break;
        }

        return ret;
    }

    printf("Successfully set %s to %lu%s\n",
           fwft_feature_name(feature), value, lock ? " (locked)" : "");

    // 6. 验证设置
    unsigned long new_value = fwft_get_value(feature);
    if (new_value != value) {
        printf("Warning: Feature value verification failed (expected %lu, got %lu)\n",
               value, new_value);
        return -EIO;
    }

    return 0;
}
```

## 参考资料

- [RISC-V SBI 规范](https://github.com/riscv-non-isa/riscv-sbi-doc)
- [SBI 3.0 规范](https://github.com/riscv-non-isa/riscv-sbi-doc/blob/master/src/sbi.adoc)
- [FWFT 扩展规范](https://github.com/riscv-non-isa/riscv-sbi-doc/blob/master/src/ext-firmware-features.adoc)
- [RISC-V 特权架构规范](https://github.com/riscv/riscv-isa-manual)