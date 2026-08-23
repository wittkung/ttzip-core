# Implementation Plan: 基于项目规范与最高行业标准的 TTZip 全代码库深度审计 (Full Codebase Standards Audit Plan)

**Feature Branch**: `041-full-codebase-standards-audit`  
**Feature Directory**: `specs/041-full-codebase-standards-audit`  
**Created**: 2026-08-17  
**Status**: Completed  
**Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/041-full-codebase-standards-audit/spec.md)

---

## Technical Context

- **审计基准**: 四大系统工程铁律（Stream-First, Invariant-First, Bounds-First, Oracle-First）、28 大设计模式指南（`design-patterns-guide`）、代码审查规范（`code-review`）以及系统宪章（`.specify/memory/constitution.md`）。
- **扫描全景**:
  1. **Layer 1 (C Bridge & SIMD)**: `Sources/CTTZipBridge/` (20+ C 源文件与头文件体系)
  2. **Layer 2 (Swift 6 Core)**: `Sources/TTZipCore/` (28 大设计模式、并发管道、编解码适配器、密码恢复引擎)
  3. **Layer 3 & Tests (UI/CLI/Tests)**: `Sources/TTZipApp/`、`Sources/TTZipCLI/`、`Tests/TTZipTests/` (525+ 测试用例、GoldenCorpus、差分与变异测试)
- **交付工件**:
  - `specs/041-full-codebase-standards-audit/research.md`: 汇总 3 个专项子 Agent 对全库 170+ 源文件的逐行静态与动态物理扫描证据。
  - `specs/041-full-codebase-standards-audit/data-model.md`: 审计元数据模型。
  - `specs/041-full-codebase-standards-audit/contracts/codebase_audit_spec.json`: Draft-07 强类型审计契约。
  - `specs/041-full-codebase-standards-audit/quickstart.md`: 校验与自测指南。
  - `docs/architecture/comprehensive_systemic_audit_report.md`: 《TTZip 全仓库系统级不变量深度审计综合全景报告》（含 41 项缺陷清单、风险分级矩阵与四阶段重构路线图）。

---

## Constitution Check

- [x] **Zero-Cost Abstraction**: 严格审查热路径是否杜绝中间堆分配、锁竞争与动态对象树构造。
- [x] **Stream-First**: 严格审查 Solid/LZFSE 是否废除全量内存假设并采用微缓冲拉取管道。
- [x] **Invariant-First**: 严格审查解压写盘是否全面开启 POSIX AT-API 标志、`O_NOFOLLOW` 与延后 Fixup。
- [x] **Bounds-First**: 严格审查敏感密码/密钥是否全面使用 `memset_s` 物理清零、C 句柄是否嵌入 `magic` 哨兵。
- [x] **Oracle-First**: 严格审查测试套件是否消除同义反复、覆盖 GoldenCorpus 真实解压与系统 CLI 差分。

---

## Phase 0: Research Items (Subagent Dispatches)

- - R001 [SUBAGENT:research] 《C 桥接层与底层引擎系统级不变量审查》：针对 `Sources/CTTZipBridge/` 下全部文件，扫描全量内存分配、安全选项、Magic 状态与敏感清零。
- - R002 [SUBAGENT:research] 《Swift 核心引擎热路径与 28 大设计模式数据平面合规审查》：针对 `Sources/TTZipCore/`，扫描 `Data(count:)`、锁使用、路径清洗、CBC 算法及跨语言转换。
- - R003 [SUBAGENT:research] 《测试套件客观预言机覆盖率与 UI 隔离架构审查》：针对 `Tests/` 与 `Sources/TTZipApp/`，扫描同义反复测试、差分测试覆盖率、输入法死锁及分层边界。

---

## Phase 1: Artifacts & Contracts

- `data-model.md`: 定义 `DefectSeverity`、`InvariantCategory`、`AuditDefectItem` 与 `CodebaseAuditReport`。
- `contracts/codebase_audit_spec.json` [SUBAGENT:research]: Draft-07 强类型全仓库审计契约。
- `quickstart.md`: 审计契约与全景报告验证指南。

---

## Phase 2: Implementation Checklist

### 1. 执行全量子 Agent 深度物理扫描
- [x] 派遣子 Agent 1：C 桥接层不变量深度扫描 (R001)
- [x] 派遣子 Agent 2：Swift Core 核心管道与模式热路径扫描 (R002)
- [x] 派遣子 Agent 3：测试套件预言机与 UI 层隔离扫描 (R003)

### 2. 整合生成全景综合审计报告
- [x] 汇总生成 `specs/041-full-codebase-standards-audit/research.md`
- [x] 编纂与同步 `docs/architecture/comprehensive_systemic_audit_report.md`

### 3. 契约固化与质量门禁
- [x] 生成 `contracts/codebase_audit_spec.json`
- [x] 执行 `quickstart.md` 自动化校验
