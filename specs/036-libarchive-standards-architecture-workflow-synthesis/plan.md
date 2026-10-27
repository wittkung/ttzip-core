# Implementation Plan: libarchive 全方位工程卓越性学习与体系改进规范

**Feature Directory**: `specs/036-libarchive-standards-architecture-workflow-synthesis`  
**Created**: 2026-08-16  
**Status**: In Progress  
**Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/036-libarchive-standards-architecture-workflow-synthesis/spec.md)

---

## Technical Context

- **项目基线**: Swift 6.0 + macOS 14.0+ (Apple Silicon NEON / Intel x86_64) + C11 / POSIX。
- **核心对标**: `Vendor/libarchive-upstream` (libarchive v3.7.x+ 工业级流式归档/解压引擎)。
- **核心挑战**:
  1. 解构 libarchive 经过数十年沉淀的 C 语言面向对象流式管道、状态机与内存模型，提取其核心架构精髓。
  2. 萃取其工业级防御性安全准则（防整数溢出、路径穿越、解压炸弹、畸形文件解析），形成系统性防御规范。
  3. 将 libarchive 的黄金预言机测试哲学与治理标准转化为自动化 Agent Skills、审查规则与工程工作流。
  4. 规划多仓库与跨语言（C/Swift）的代码布局与接口隔离体系，在确保工业级安全与可维护性的同时捍卫 TTZip 吞吐底线（零成本抽象、无锁无分配）。

---

## Constitution Check

- [x] **Zero-Cost Abstraction**: 提炼的架构抽象与安全防御规则严格区分调度层与热路径，热路径保持零中间分配与无锁化。
- [x] **No Subprocess**: 核心引擎坚持 100% In-process C 静态绑定，不引入任何外部 CLI 进程。
- [x] **Zero Bare Logging**: 所有日志规范强制使用 `TTLogger`，严禁在生产或桥接代码中出现裸 `printf`/`NSLog`。
- [x] **Frozen Subsystems**: 绝不擅自侵入已冻结的 ZIP 核心引擎（遵循 `zip-engine-freeze.md`）。
- [x] **Platform Integrity**: 兼容 MAS 沙盒 (`-DMAS_BUILD`) 与 Direct 独立分发渠道。

---

## Phase 0: Research Items (Subagent Dispatched)

- R001 [SUBAGENT:research] 《libarchive 流式架构、状态机与内存模型解构》：深入分析 `libarchive/archive_read.c`, `archive_write.c`, `archive_entry.c` 中的 C 语言面向对象多态机制、过滤器链管道、状态迁移与微缓冲机制。
- R002 [SUBAGENT:research] 《libarchive 防御性编程与安全漏洞免疫模型》：深入分析 `archive_write_disk.c`, `archive_read_support_format_*.c` 中的整数溢出防护、路径遍历拦截、解压炸弹限制及内存边界检查。
- R003 [SUBAGENT:research] 《libarchive 黄金预言机测试与质量验证哲学》：深入解构 libarchive 的轻量级独立测试框架 (`libarchive/test/test_main.c`)、黄金预言机对齐策略与回归防御体系。
- R004 [SUBAGENT:research] 《Upstream 治理规范与 Prompt/Skills/Workflows 升级映射》：深入分析 libarchive 社区治理与贡献标准，制定 Prompt、Skills (`code-review`, `upstream-contribution`, `design-patterns-guide`) 与多仓库代码组织规范的具体升级方案。

---

## Phase 1: Artifacts & System Boundaries

### Data Model & Contracts
- `data-model.md`: 涵盖架构实体模型、安全断言契约、技能演进模型与多仓库结构规范。
- `contracts/libarchive_architecture_spec.json` [SUBAGENT:research]: 架构抽象与接口规范契约。
- `contracts/security_defense_matrix.json` [SUBAGENT:research]: 安全防御准则与校验契约。
- `contracts/workflow_skill_evolution.json` [SUBAGENT:research]: Prompt/Skill/Workflow 升级规范契约。
- `contracts/repo_layout_blueprint.json` [SUBAGENT:research]: 代码组织与模块布局契约。

### Validation
- `quickstart.md`: 规范落地验证与 Skills/Rules 审查自测指南。

---

## Phase 2: Implementation & Component Modifications

### 1. 深度研究与知识库沉淀
- [ ] 产出系统性报告 `docs/architecture/libarchive_engineering_excellence.md`
- [ ] 产出多仓库组织蓝图 `docs/architecture/repository_organization_guide.md`

### 2. Agent Skills 体系全面升级
- [ ] 升级 `.agents/skills/upstream-contribution/SKILL.md`（注入 libarchive 标准治理、原子提交、跨架构安全）
- [ ] 升级 `.agents/skills/code-review/SKILL.md`（注入 C/Swift 系统编程安全审查、防御性编程检查项）
- [ ] 升级 `.agents/skills/design-patterns-guide/SKILL.md`（注入 C 语言面向对象流式管道、零拷贝适配器模式）

### 3. 全局与项目级 Rules 优化
- [ ] 增强 `GEMINI.md` 与全局工程规范（固化系统编程安全防御不变量）
- [ ] 校验并统一多仓库与跨语言协作标准
