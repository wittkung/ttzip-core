# Data Model: Upstream Contribution Methodology, Lessons Learned, and Engineering Governance

**Feature Directory**: `specs/133-upstream-contribution-lessons-and-governance`  
**Target Subject**: 强类型审计门禁、统计模型与治理契约实体定义  

---

## 1. Core Entities Definition

### UpstreamAuditReport (Top-level Audit Payload)
| Field | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `schema_version` | `string` (enum: ["1.0.0"]) | Yes | 契约版本号 |
| `audit_timestamp` | `string` (date-time) | Yes | 审计执行 UTC 时间戳 |
| `target_upstream` | `string` | Yes | 目标上游仓库标识（如 `zlib-ng`, `libarchive`） |
| `target_function` | `string` | Yes | 优化目标核心函数名（如 `compare256_neon`） |
| `worktree_path` | `string` | Yes | 候选分支所在的物理 worktree 路径 |
| `baseline_branch` | `string` | Yes | 基线分支名称（默认 `develop` 或 `master`） |
| `candidate_branch`| `string` | Yes | 待测候选优化分支名称 |
| `compiler_audit` | `CompilerParityAudit` | Yes | 编译器与编译选项一致性审计结果 |
| `dual_build_audit`| `DualBuildAudit` | Yes | CMake / Autotools 双构建系统与警告审计 |
| `micro_results` | `Array<BenchmarkPoint>` | Yes | 微观各匹配长度纳秒级评测结果数组 |
| `macro_results` | `Array<BenchmarkPoint>` | Yes | 宏观全工作负载端到端压缩评测结果数组 |
| `cv_statistics` | `CvSummary` | Yes | 全矩阵变异系数统计汇总 |
| `overall_verdict` | `AuditVerdict` | Yes | 最终审计通过裁决与门禁结论 |

---

### CompilerParityAudit (编译器等价性实体)
| Field | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `compiler_id` | `string` | Yes | 编译器类型（如 `AppleClang`, `GNU`, `Clang`） |
| `compiler_version` | `string` | Yes | 编译器语义版本（如 `21.0.0`） |
| `baseline_c_flags` | `string` | Yes | Baseline 的完整 C 编译标志字串 |
| `candidate_c_flags`| `string` | Yes | Candidate 的完整 C 编译标志字串 |
| `flags_identical` | `boolean` | Yes | 关键构建参数（-O3, -DNDEBUG 等）是否 100% 对齐 |

---

### DualBuildAudit (双构建系统审计实体)
| Field | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `cmake_build_passed` | `boolean` | Yes | CMake Release 构建与零警告编译是否通过 |
| `autotools_build_passed` | `boolean` | Yes | Autotools/Makefile.am 构建与零警告是否通过 |
| `ctest_passed` | `boolean` | Yes | CTest 单元测试是否 100% PASS |
| `disassembly_instruction_count` | `integer` | Yes | 目标函数反汇编机器码指令总数 |
| `stack_spill_detected` | `boolean` | Yes | 是否检测到寄存器溢出到栈内存 |

---

### BenchmarkPoint (单基准测试点实体)
| Field | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `benchmark_name` | `string` | Yes | 测试点完整名称（如 `deflate_bench/level/text/131072/1`） |
| `workload_type` | `string` (enum) | Yes | `text`, `striped_rgb`, `dna`, `mixed`, `short_match`, `random`, `literals`, `realistic_rgb` |
| `payload_size_bytes`| `integer` | Yes | 待测数据包大小（如 131072, 1048576） |
| `compression_level` | `integer` | Yes | 压缩级别（1, 3, 6, 9 等） |
| `baseline_median_ns` | `number` | Yes | Baseline 5 轮双向交错中位数耗时（纳秒） |
| `candidate_median_ns`| `number` | Yes | Candidate 5 轮双向交错中位数耗时（纳秒） |
| `delta_percentage` | `number` | Yes | 相对耗时变化百分比（负值代表加速） |
| `cv_percentage` | `number` | Yes | 该测试点的变异系数 CV 百分比 |
| `is_regression` | `boolean` | Yes | 耗时增加是否超过门禁允许的 +2.0% |

---

### CvSummary (变异系数统计实体)
| Field | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `median_cv_percentage` | `number` | Yes | 全样本中位数变异系数（门禁阈值 <= 1.50%） |
| `mean_cv_percentage` | `number` | Yes | 全样本平均变异系数 |
| `max_cv_percentage` | `number` | Yes | 全样本最大变异系数 |
| `high_variance_point_count` | `integer` | Yes | CV > 3.0% 的高抖动点数量 |

---

### AuditVerdict (最终门禁裁决实体)
| Field | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `gate_passed` | `boolean` | Yes | 是否完全通过上游 Pre-Flight 门禁 |
| `verdict_level` | `string` (enum) | Yes | `PASS`, `BLOCK_REGRESSION`, `BLOCK_HIGH_VARIANCE`, `BLOCK_BUILD_MISMATCH` |
| `blocking_reasons` | `Array<string>` | Yes | 阻断原因详细清单（通过时为空数组） |
| `recommended_action` | `string` | Yes | 下一步行动建议（允许提交 PR 或回退重构） |
