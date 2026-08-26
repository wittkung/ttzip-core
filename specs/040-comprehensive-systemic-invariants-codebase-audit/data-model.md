# Data Model: 全仓库系统级不变量审计模型

**Feature Directory**: `specs/040-comprehensive-systemic-invariants-codebase-audit`  
**Date**: 2026-08-16  
**Status**: Ready for Planning

---

## 1. 实体定义与字段规范

### 1.1 `CodebaseAuditReportSpec` (全仓库审计契约模型)
定义全仓库审计覆盖维度、缺陷分级与指标。

| 字段名 | 类型 | 必填 | 约束与说明 |
| :--- | :--- | :--- | :--- |
| `spec_version` | `string` | 是 | 规范版本，固定为 `"1.0.0"` |
| `audited_layers` | `Array<string>` | 是 | 审计层级列表，包含 C Bridge, Swift Core, App, Tests |
| `total_files_scanned` | `integer` | 是 | 扫描文件总数，最小 50 |
| `risk_categories` | `Array<RiskCategorySpec>` | 是 | 风险分级矩阵列表 |

- **`RiskCategorySpec`**:
  - `level` (`string`, 必填): 风险级别 (`"P0_CRITICAL"`, `"P1_HIGH"`, `"P2_MEDIUM"`, `"P3_LOW"`)
  - `dimension` (`string`, 必填): 所属四大铁律维度 (`"Stream-First"`, `"Invariant-First"`, `"Bounds-First"`, `"Oracle-First"`)
  - `defect_count` (`integer`, 必填): 发现的缺陷总数
  - `description` (`string`, 必填): 风险与危害概述
