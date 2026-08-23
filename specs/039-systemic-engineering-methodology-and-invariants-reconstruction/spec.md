# Feature Specification: 系统性工程方法论与底层不变量体系重塑规范 (Systemic Engineering Methodology & Invariants Reconstruction)

**Feature Directory**: `specs/039-systemic-engineering-methodology-and-invariants-reconstruction`  
**Created**: 2026-08-16  
**Status**: Draft  
**Input**: "将从 libarchive 提炼的四大系统工程心法（流式第一性 Stream-First、纵深防御 Invariant-First、确定性确界 Bounds-First、真实预言机 Oracle-First）全面注入项目宪法、架构规范、方法论指南与代码库治理体系，形成不可逆的工程防御闭环"

---

## Clarifications

### Session 2026-08-16
- **Q1 (四大铁律的层级与效力)**: 四大铁律在项目中的法律效力层级如何？
  - **Resolution**: 四大系统工程铁律直接作为最高宪法条款（Constitution Level 0），任何违反 Stream-First / Invariant-First / Bounds-First / Oracle-First 的 PR 或代码修改，视为红灯阻断，一票否决。
- **Q2 (7z 流式分块架构窗口尺寸)**: 分块流式 Solid 压缩的默认块大小如何设定？
  - **Resolution**: 默认 Solid Chunk 窗口设为 32MB（极速模式 Level 1）或 64MB（高压缩比 Level 5~9），单线程峰值内存锁定在 $64\text{MB} \sim 128\text{MB}$ 恒定区间，彻底消除内存随归档总大小线性暴涨的问题。


## User Scenarios & Testing *(mandatory)*

### User Story 1 - 宪法与项目级规则注入四大系统工程铁律 (Priority: P1)

作为团队架构师与 AI 协作指导者，我需要将“四大系统工程铁律”（流式第一性、纵深防御、确定性确界、真实预言机）深度注入 `.specify/memory/constitution.md`、`GEMINI.md` 与项目规范中，明确禁止“假定内存无限的批处理”、“仅靠上层字符串正则的假安全”、“无 Magic 哨兵的 C 句柄”以及“自产自销的同义反复测试”，使所有未来代码编写与审查拥有最高优先级的硬性法律依据。

**Why this priority**: 制度与宪法是防止系统性退化的最高防线，确保任何后续功能开发均在正确的方法论轨道上运行。

**Independent Test**: 检查 `constitution.md` 与 `GEMINI.md`，断言四大系统工程铁律与不可违背的红线已 100% 显式定义。

**Acceptance Scenarios**:
1. **Given** 宪法文件，**When** 审查系统不变量章节，**Then** 包含清晰的 Stream-First、Invariant-First、Bounds-First、Oracle-First 物理约束。
2. **Given** 开发/审查指令，**When** 出现 `Data(count:)` 堆清零或无保护 C 指针分配时，**Then** 规则引擎立即阻断。

---

### User Story 2 - 编写《TTZip 工业级系统工程方法论与心智模型指南》 (Priority: P2)

作为核心开发者与贡献者，我需要一份系统性、全景式的方法论指南文档（`docs/architecture/systemic_engineering_methodology.md`），详尽对比“高级语言批处理思维”与“工业级系统底层思维”的本质差异，给出数据流设计模式、POSIX 级安全落盘、防御性内存模型与客观预言机测试的落地范式与反模式案例库。

**Why this priority**: 提供直观、可落地的心智模型迁移图谱，消除开发者的认知盲区。

**Independent Test**: 验证文档的完整性与深度，包含数据流、安全、内存、测试 4 大维度的正反代码对照与架构推导。

**Acceptance Scenarios**:
1. **Given** 新成员或 AI Agent 接手开发，**When** 查阅该方法论指南，**Then** 能清晰识别出什么是系统级反模式并获得正确实现范式。

---

### User Story 3 - 7z 实体流分块压缩架构重构计划与接口契约固化 (Priority: P3)

作为底层引擎架构师，针对 `CTTZipBridge_7zSolid.c` 现存的一次性全量内存分配痛点，我需要在规范与契约层明确将其重构为 **基于 32MB/64MB 滑动窗口与分块流式写入（Chunked Solid Streaming Pipeline）** 的标准架构设计与数据契约，彻底消除超大归档下的 OOM 隐患。

**Why this priority**: 解决当前代码库中最大的内存失控隐患，践行 Stream-First 原则。

**Independent Test**: 验证 `contracts/` 中固化的 7z 分块流式压缩接口契约与数据模型。

**Acceptance Scenarios**:
1. **Given** 7z Solid 压缩配置，**When** 传入 100GB 目录，**Then** 契约强制要求分块流式处理，单任务峰值内存被严格 Clamp 在 $\le 128\text{MB}$。

---

## Edge Cases

- **超大单文件流式切片**：单个文件超过 4GiB 时的分块与 Zip64/7z 头部流式更新。
- **并发任务下的内存上限**：多线程并发时，通过全局内存配额控制器（`MemoryBudgetController`）防止并发分块累加超限。

---

## Requirements *(mandatory)*

- **FR-001**: 必须在 `.specify/memory/constitution.md` 中增加《四大系统工程铁律》专属章节。
- **FR-002**: 必须在 `GEMINI.md` 中同步更新系统工程方法论与审查准则。
- **FR-003**: 必须在 `docs/architecture/systemic_engineering_methodology.md` 中编写系统性工程方法论与心智模型全景指南。
- **FR-004**: 必须在 `specs/039-systemic-engineering-methodology-and-invariants-reconstruction/contracts/` 中固化流式分块压缩与系统工程契约。

---

## Success Criteria *(mandatory)*

- **SC-001**: `constitution.md` 与 `GEMINI.md` 完整注入四大系统工程铁律。
- **SC-002**: `systemic_engineering_methodology.md` 深度解构 4 大代差与落地范式。
- **SC-003**: 契约文件通过 Draft-07 强约束与零裸通配校验。
