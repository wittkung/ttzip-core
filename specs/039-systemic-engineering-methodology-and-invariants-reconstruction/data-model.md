# Data Model: 系统工程方法论与分块流式压缩实体模型

**Feature Directory**: `specs/039-systemic-engineering-methodology-and-invariants-reconstruction`  
**Date**: 2026-08-16  
**Status**: Ready for Planning

---

## 1. 实体定义与字段规范

### 1.1 `SystemicMethodologySpec` (系统工程方法论契约模型)
定义四大系统工程铁律的构成与约束。

| 字段名 | 类型 | 必填 | 约束与说明 |
| :--- | :--- | :--- | :--- |
| `spec_version` | `string` | 是 | 规范版本，固定为 `"1.0.0"` |
| `core_invariants` | `Array<string>` | 是 | 四大系统工程铁律名称列表 |
| `stream_first_constraints` | `Array<string>` | 是 | 流式第一性约束清单（微缓冲、零内存假设、零零填充） |
| `invariant_first_constraints` | `Array<string>` | 是 | 纵深防御约束清单（AT-API、延后 Fixup、防溢出） |
| `bounds_first_constraints` | `Array<string>` | 是 | 确定性确界约束清单（Magic 首字段、memset_s、Clamp） |
| `oracle_first_constraints` | `Array<string>` | 是 | 真实预言机约束清单（UU 语料库、系统差分、崩溃优先 Fuzz） |

---

### 1.2 `ChunkedSolidStreamSpec` (7z 分块流式 Solid 压缩架构模型)
定义 7z Solid 压缩分块管道与内存配额。

| 字段名 | 类型 | 必填 | 约束与说明 |
| :--- | :--- | :--- | :--- |
| `spec_version` | `string` | 是 | 规范版本，固定为 `"1.0.0"` |
| `chunk_size_mb` | `integer` | 是 | 单个 Solid 块的目标窗口大小（32 或 64） |
| `max_memory_budget_mb` | `integer` | 是 | 单线程工作区最大内存上限（<= 128MB） |
| `substreams_info_enabled` | `boolean` | 是 | 是否在 7z 头中写入 SubStreamsInfo 结构，固定为 `true` |
| `streaming_write_enabled` | `boolean` | 是 | 是否在每个块压缩完成后立即流式写盘，固定为 `true` |
