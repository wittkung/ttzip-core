# Feature Specification: libarchive 全方位工程卓越性学习与体系改进规范 (libarchive Standards, Architecture & Workflow Synthesis)

**Feature Directory**: `specs/036-libarchive-standards-architecture-workflow-synthesis`  
**Created**: 2026-08-16  
**Status**: Draft  
**Input**: "libarchive，我想要全方位的学习这个项目，他们的很多标准，架构，流程，安全等等等都非常好，很多是我们的薄弱项，希望你好好学习，总结经验，看看我们的提示词和相关 skill 与工作流应该怎么样改进，还有我们各个仓库的代码应该怎么组织和改进"

---

## Clarifications

### Session 2026-08-16
- **Q1 (深度分析范围)**: libarchive 深度研究聚焦于哪些核心维度？
  - **Resolution**: 聚焦于 6 大核心维度：(1) 流式双向流水线与过滤器设计模式，(2) 内存管理、微缓冲与零拷贝机制，(3) 错误分级与严密状态机规范，(4) 安全防御与漏洞免疫体系（整数安全、路径遍历、压缩炸弹防护），(5) 黄金预言机与测试套件体系，(6) Upstream 社区治理与原子提交标准。
- **Q2 (落地交付产物)**: 最终的沉淀产物如何组织以指导实际开发与 Agent 行为？
  - **Resolution**: (1) 产出系统的工业级架构与工程方法论报告；(2) 升级 Agent Skills（`upstream-contribution`、`code-review`、`design-patterns-guide`）和 Rules；(3) 制定多仓库与 C/Swift 跨语言代码组织架构蓝图。
- **Q3 (性能与架构兼容性)**: 新的标准与防御规范如何与现有性能铁律共存？
  - **Resolution**: 严格遵循零成本抽象原则，防御性边界断言与类型安全防护前置于调度层与边界层，热路径保持无锁无分配，兼顾极致性能与工业级鲁棒性。

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 全面萃取 libarchive 核心架构、安全与工程标准规范 (Priority: P1)

作为系统架构师和核心开发者，我需要对 `libarchive` 开源项目进行多维度的深度解构，涵盖其流式架构抽象、过滤器管道、内存/缓冲区管理、错误状态机、安全防御准则（防路径遍历、整数溢出、炸弹攻击）、测试预言机体系以及上游治理规范，从而形成一套系统化、可执行的工程卓越性基准文档。

**Why this priority**: 这是全流程改进的认知基石与知识源泉。必须先彻底解构并精准沉淀 libarchive 的核心精髓与工业级标准，才能有效指导后续提示词、Skill、工作流及代码组织的针对性升级。

**Independent Test**: 产出系统性解构研究报告与知识库，能够独立查阅并指导 C/Swift 跨语言系统级开发中的具体设计决策与安全编码。

**Acceptance Scenarios**:
1. **Given** `Vendor/libarchive-upstream` 源码与官方架构规范，**When** 执行架构与代码规范解构，**Then** 输出包含流式处理模型、过滤器链抽象、错误分级状态机、内存微缓冲与安全防御体系的结构化知识体系。
2. **Given** libarchive 的测试用例组织方式，**When** 分析其测试哲学，**Then** 提取其黄金预言机对齐、平台独立断言与模糊测试兼容模式的完整 SOP。
3. **Given** libarchive 的 Upstream 贡献与维护标准，**When** 总结其治理流程，**Then** 提炼出原子提交序列、向后兼容契约与安全审查清障标准。

---

### User Story 2 - 提示词 (Prompts)、Rules 与 Agent Skills 体系全量升级 (Priority: P2)

作为 AI 辅助研发工程师，我需要将从 libarchive 中萃取的架构标准、防御性编程准则、审查清单及上游贡献规约，沉淀并更新到我们的提示词模板、全局/项目级 Rules（如 `GEMINI.md`、`speckit-multiagent.md`）以及相关 Agent Skills（如 `code-review`、`upstream-contribution`、`design-patterns-guide` 等）中，使 AI Agent 在日常代码编写、代码审查和架构设计中能够自动践行这些高标准。

**Why this priority**: 将知识转化为 Agent 的执行约束与自动化能力，是实现研发质量跃迁的关键杠杆，避免知识只停留在文档层面。

**Independent Test**: 审查更新后的 Skills 和 Rules，验证其在代码审查与工程任务中能够准确识别出不符合工业级标准的潜在漏洞、反模式与组织缺陷。

**Acceptance Scenarios**:
1. **Given** 提炼的安全与防御准则，**When** 更新 `code-review` 与 `upstream-contribution` Skills，**Then** 增加针对整数溢出防护、NULL/短读取防御、状态机一致性、资源泄漏与零裸输出的硬性审查项。
2. **Given** 萃取的架构设计模式，**When** 扩展 `design-patterns-guide`，**Then** 增加流式管道（Pipeline/Filter）、抽象 I/O 适配器（Adapter/Strategy）等工业级 C/Swift 混合模式的最佳实践与禁忌。
3. **Given** 全局与项目级 Prompt/Rule 配置，**When** 注入 libarchive 体系的工程纪律，**Then** 确保 AI 代理在处理系统级与底层 C 库任务时具备主动寻猎与防御式设计本能。

---

### User Story 3 - 代码仓库组织与跨语言架构模式改进方案 (Priority: P3)

作为多仓库与底层库维护者，我需要参考 libarchive 的模块解耦方式与构建体系，制定我们各个仓库（TTZipCore、CTTZipBridge、TTZipApp、Vendor 上游子模块）的代码组织规范、跨语言抽象层（C/Swift Bridge）隔离标准以及第三方依赖治理策略。

**Why this priority**: 规范化的代码结构和清晰的层级边界能够降低模块耦合，提升跨平台可移植性，确保高性能热路径与外围业务逻辑的清晰隔离。

**Independent Test**: 形成各仓库架构组织蓝图与重构迁移指导指南，能够指导具体模块的目录重组与接口边界治理。

**Acceptance Scenarios**:
1. **Given** 当前仓库中 CTTZipBridge 与 Vendor 依赖的组织现状，**When** 对照 libarchive 的分层组织哲学，**Then** 制定清晰的 Native Bridge、Vendor Upstream、Swift Core 边界隔离方案与头文件暴露规范。
2. **Given** 多仓库开发与上游同步场景，**When** 规划依赖管理与 Patch 维护流，**Then** 建立规范的 Upstream Git Worktree、Subtree/Submodule 协同与 CI 构建隔离机制。

---

## Edge Cases

- **跨语言类型差异**：C 语言 64-bit 偏移（`int64_t` / `la_int64_t`）与 Swift `Int` / `Int64` / `size_t` 之间的安全转换与溢出截断处理。
- **构建环境与分发渠道冲突**：MAS 沙盒环境（`-DMAS_BUILD`）与 Direct 独立分发渠道下系统底层 API 调用与静态库链接的差异化兼容。
- **性能热路径与防御性开销的平衡**：在严苛的吞吐底线（如 10,000+ MB/s）下，如何在零额外堆分配的前提下实施必要的安全边界断言。

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: 必须全方位剖析 `libarchive` 的 6 大核心支柱：流式读写管道架构（Stream Pipeline）、过滤器链（Filter Chain）、内存微缓冲与零拷贝机制、分级错误与状态机体系、防御性安全模型（整数安全/路径规范化/炸弹防护）、以及黄金预言机测试哲学。
- **FR-002**: 必须形成一份详尽的《libarchive 工业级工程卓越性与设计哲学沉淀报告》（包含具体 C 源码引证与对比分析）。
- **FR-003**: 必须审视并升级现有的 Agent Skills（包括但不限于 `upstream-contribution`、`code-review`、`design-patterns-guide`），补充系统级编程与安全审查硬指标。
- **FR-004**: 必须梳理并优化我们的提示词 (System Prompts / Project Rules)，建立面向系统编程与基础设施开发的高标准防御约束。
- **FR-005**: 必须制定代码仓库组织与跨语言架构演进方案，明确各层目录职责、公共与内部头文件隔离、Vendor 静态库打补丁流程与自动化验证机制。
- **FR-006**: 所有新规范与标准必须与 TTZip 的宪法（`constitution.md`）及最高性能铁律（零成本抽象、无锁无分配热路径）保持 100% 兼容。

### Key Entities

- **ArchCoreModel (架构核心模型)**: 抽象读写句柄、过滤器流水线节点、格式解析器、底层 I/O 回调适配器。
- **SecurityInvariantMatrix (安全不变量矩阵)**: 整数溢出截断防护、缓冲区边界检查、路径穿越过滤、畸形输入防御、资源耗尽防护。
- **SkillEvolutionBlueprint (技能演进蓝图)**: 针对 Agent Skills 的更新项、规则增强点、审查 Checklists。
- **RepoLayoutStandard (仓库布局标准)**: C 桥接层目录结构、头文件暴露分层、第三方 Upstream 同步与 Patch 治理规范。

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 产出包含 6 大工程支柱的完整深度解析文档，覆盖代码解构、哲学提炼与落地映射。
- **SC-002**: 现有核心 Skills（`upstream-contribution`、`code-review`、`design-patterns-guide`）和相关规则文档完成 100% 的对齐升级与扩充。
- **SC-003**: 制定清晰的代码组织与架构重构蓝图，覆盖 4 个核心层次（Vendor、Native Bridge、Core Engine、App UI）。
- **SC-004**: 所有提炼的改进措施与规范均具备可操作的落地检验手段，与现有 525+ 单元测试和性能门禁无冲突。

---

## Assumptions

- 研究主要基于项目内置的 `Vendor/libarchive-upstream` 源码树以及业界成熟的 libarchive 官方工程规范。
- 改进方案同时面向当前 TTZip 仓库以及未来可能派生的基础设施/系统级仓库。
- 所有关于性能、安全与架构的改进建议严格遵守当前项目的 Swift 6.0 + macOS 14+ 平台约束。
