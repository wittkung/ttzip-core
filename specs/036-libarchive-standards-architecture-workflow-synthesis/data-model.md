# Data Model: libarchive 全方位工程卓越性与体系改进模型

**Feature Directory**: `specs/036-libarchive-standards-architecture-workflow-synthesis`  
**Date**: 2026-08-16  
**Status**: Ready for Planning

---

## 1. 实体定义与字段规范

### 1.1 `LibarchiveArchitectureSpec` (架构抽象与流式管道模型)
描述 libarchive 核心面向对象多态、流式过滤器链与微缓冲的工业级抽象。

| 字段名 | 类型 | 必填 | 约束与说明 |
| :--- | :--- | :--- | :--- |
| `spec_version` | `string` | 是 | 规范版本号，如 `"1.0.0"` |
| `oop_polymorphism` | `OOPPolymorphismConfig` | 是 | C 语言单根继承与双层虚表派发配置 |
| `filter_pipeline` | `FilterPipelineConfig` | 是 | 双向流式过滤器流水线与自动竞标配置 |
| `micro_buffering` | `MicroBufferingConfig` | 是 | Lookahead 与 Consume 解耦的微缓冲配置 |
| `state_machine` | `StateMachineConfig` | 是 | 位掩码状态机与错误单调传播配置 |

- **`OOPPolymorphismConfig`**:
  - `inheritance_pattern` (`string`, 必填): 首成员物理继承模式，如 `"STRUCT_FIRST_MEMBER_BASE"`
  - `vtable_tiers` (`integer`, 必填): 虚表分层数，固定为 `2`（顶层引擎虚表与格式/过滤器策略虚表）
  - `registration_strategy` (`string`, 必填): 注册策略，固定为 `"STATIC_SLOT_ARRAY_16"`
- **`FilterPipelineConfig`**:
  - `max_filter_depth` (`integer`, 必填): 过滤器链最大嵌套深度，固定为 `25`
  - `bidding_protocol` (`string`, 必填): 竞标协议模式，固定为 `"BIDIRECTIONAL_PEEK_SCORE"`
  - `bid_score_unit` (`string`, 必填): 竞标置信度打分单位，固定为 `"MATCHED_BIT_COUNT"`
- **`MicroBufferingConfig`**:
  - `lookahead_api` (`string`, 必填): 预读 API，固定为 `"__archive_read_ahead"`
  - `consume_api` (`string`, 必填): 消费 API，固定为 `"__archive_read_consume"`
  - `zero_copy_fast_path` (`boolean`, 必填): 是否启用块内零拷贝直通，固定为 `true`
  - `growth_factor` (`number`, 必填): 跨块微缓冲扩容乘数，固定为 `2.0`
- **`StateMachineConfig`**:
  - `state_representation` (`string`, 必填): 状态表示方式，固定为 `"BITMASK_FLAGS"`
  - `fatal_state_mask` (`string`, 必填): 致命错误掩码，固定为 `"0x8000"`
  - `monotonic_error_chaining` (`boolean`, 必填): 错误合并单调性，固定为 `true`

---

### 1.2 `SecurityDefenseMatrix` (安全防御与漏洞免疫矩阵)
描述工业级解压与归档引擎在路径安全、整型算术、解压炸弹和内存生命周期的硬性防御标准。

| 字段名 | 类型 | 必填 | 约束与说明 |
| :--- | :--- | :--- | :--- |
| `matrix_version` | `string` | 是 | 矩阵版本号，如 `"1.0.0"` |
| `path_security` | `PathSecuritySpec` | 是 | 路径清洗与符号链接穿透防御规约 |
| `integer_safety` | `IntegerSafetySpec` | 是 | 跨架构算术溢出与 Clamp 截断保护规约 |
| `decompression_bomb` | `DecompressionBombSpec` | 是 | 解压炸弹、无限递归与内存耗尽熔断规约 |
| `memory_lifecycle` | `MemoryLifecycleSpec` | 是 | 魔数哨兵、析构清零与凭据安全擦除规约 |

- **`PathSecuritySpec`**:
  - `normalization_algorithm` (`string`, 必填): 原地单遍清洗算法，固定为 `"IN_PLACE_SINGLE_PASS_CLEANUP"`
  - `symlink_traversal_check` (`string`, 必填): 符号链接逐级探测机制，固定为 `"AT_API_STEPWISE_INODE_CHECK"`
  - `toctou_mitigation` (`string`, 必填): TOCTOU 权限竞态缓解机制，固定为 `"DEFERRED_FIXUP_DEPTH_FIRST_REVERSE"`
- **`IntegerSafetySpec`**:
  - `overflow_check_engine` (`string`, 必填): 防溢出运算引擎，固定为 `"COMPILER_BUILTIN_HARDWARE_OVERFLOW"`
  - `narrowing_clamp_mask` (`string`, 必填): 64 位整型窄化保护，固定为 `"SSIZE_MAX_CLAMP"`
- **`DecompressionBombSpec`**:
  - `max_compression_ratio` (`number`, 必填): 异常解压比率上限，如 `1000.0`
  - `rar5_max_window_bytes` (`integer`, 必填): RAR5 窗口最大字节数，固定为 `67108864` (64MB)
  - `meta_stream_consistency_check` (`boolean`, 必填): 条目数与头部流大小交叉一致性断言，固定为 `true`
- **`MemoryLifecycleSpec`**:
  - `magic_header_field` (`string`, 必填): 结构体首部魔数字段，如 `"magic"`
  - `magic_invalidation_on_free` (`boolean`, 必填): 析构前魔数清零，固定为 `true`
  - `passphrase_secure_zeroing` (`string`, 必填): 密码擦除函数，固定为 `"EXPLICIT_BZERO_OR_MEMSET_S"`

---

### 1.3 `WorkflowSkillEvolution` (技能演进与工程工作流规范)
描述针对 Agent Skills、Prompts 与审查流程的升级规约。

| 字段名 | 类型 | 必填 | 约束与说明 |
| :--- | :--- | :--- | :--- |
| `evolution_version` | `string` | 是 | 演进版本号，如 `"1.0.0"` |
| `code_review_enhancements` | `Array<string>` | 是 | `code-review` 技能新增审查项清单（至少 4 项） |
| `upstream_contribution_rules` | `Array<string>` | 是 | `upstream-contribution` 技能强化规则清单（至少 4 项） |
| `design_patterns_additions` | `Array<string>` | 是 | `design-patterns-guide` 技能新增系统级模式清单（至少 3 项） |
| `prompt_invariants` | `Array<string>` | 是 | 全局/项目级 Prompts 注入的系统级铁律（至少 4 项） |

---

### 1.4 `RepoLayoutBlueprint` (多仓库组织与分层隔离蓝图)
描述 C 桥接层、Swift 引擎、Upstream 源码与 App 的物理分层和隔离约束。

| 字段名 | 类型 | 必填 | 约束与说明 |
| :--- | :--- | :--- | :--- |
| `blueprint_version` | `string` | 是 | 蓝图版本号，如 `"1.0.0"` |
| `tiers` | `Array<ArchitectureTier>` | 是 | 4 层物理架构分层定义 |
| `header_isolation_rules` | `Array<string>` | 是 | C 头文件与模块映射暴露隔离规则 |
| `upstream_patch_workflow` | `UpstreamPatchWorkflow` | 是 | Upstream 补丁开发与双向同步机制 |

- **`ArchitectureTier`**:
  - `tier_level` (`integer`, 必填): 分层层级（0 到 3）
  - `name` (`string`, 必填): 分层名称，如 `"Layer 0: Pristine Upstream"`
  - `path` (`string`, 必填): 对应目录路径，如 `"Vendor/libarchive-upstream/"`
  - `responsibilities` (`Array<string>`, 必填): 该层核心职责列表
  - `allowed_dependencies` (`Array<string>`, 必填): 允许依赖的下层列表
- **`UpstreamPatchWorkflow`**:
  - `worktree_directory` (`string`, 必填): 隔离开发工作树目录，如 `"Vendor/worktrees/"`
  - `commit_convention` (`string`, 必填): 提交序列范式，固定为 `"INFRA_FEAT_TEST_THREE_STAGE"`
  - `bisect_guarantee` (`boolean`, 必填): 独立可二分编译保证，固定为 `true`
