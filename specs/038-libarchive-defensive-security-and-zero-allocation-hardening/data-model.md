# Data Model: 防御性安全与零分配热路径加固模型

**Feature Directory**: `specs/038-libarchive-defensive-security-and-zero-allocation-hardening`  
**Date**: 2026-08-16  
**Status**: Ready for Planning

---

## 1. 实体定义与字段规范

### 1.1 `HardeningSpec` (加固规范契约模型)
描述解压管道安全标志位、路径清洗规则与热路径零分配配置。

| 字段名 | 类型 | 必填 | 约束与说明 |
| :--- | :--- | :--- | :--- |
| `spec_version` | `string` | 是 | 规范版本，固定为 `"1.0.0"` |
| `security_flags` | `Array<string>` | 是 | 必须启用的安全标志位清单（至少 4 项） |
| `zero_allocation_enabled` | `boolean` | 是 | 是否启用热路径未初始化裸指针优化，固定为 `true` |
| `password_wipe_function` | `string` | 是 | 密码安全擦除函数，固定为 `"memset_s"` |

- **`security_flags` 必选项**：
  - `"ARCHIVE_EXTRACT_SECURE_SYMLINKS"`
  - `"ARCHIVE_EXTRACT_SECURE_NODOTDOT"`
  - `"ARCHIVE_EXTRACT_SECURE_NOABSOLUTEPATHS"`
  - `"ARCHIVE_EXTRACT_UNLINK"`
