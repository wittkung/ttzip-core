# Implementation Plan: 基于四大系统工程铁律的 TTZip 全仓库深度代码审计

**Feature Directory**: `specs/040-comprehensive-systemic-invariants-codebase-audit`  
**Created**: 2026-08-16  
**Status**: In Progress  
**Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/040-comprehensive-systemic-invariants-codebase-audit/spec.md)

---

## Technical Context

- **审计基准**: 四大系统工程铁律（Stream-First, Invariant-First, Bounds-First, Oracle-First）与 `.specify/memory/constitution.md`。
- **扫描范围**:
  1. Layer 1: `Sources/CTTZipBridge/` (20+ C 源文件)
  2. Layer 2: `Sources/TTZipCore/` (28 大设计模式、并发管道、编解码适配器)
  3. Layer 3 & Tests: `Sources/TTZipApp/`、`Sources/TTZipCLI/`、`Tests/TTZipTests/` (525+ 测试用例)
- **交付工件**:
  - `specs/040-comprehensive-systemic-invariants-codebase-audit/research.md` (汇总 3 个子 Agent 深度扫描结果)
  - `docs/architecture/comprehensive_systemic_audit_report.md` (全景缺陷审计报告、风险矩阵与排期图)

---

## Constitution Check

- [x] **Zero-Cost Abstraction**: 严格基于热路径零开销、零中间分配与零锁竞争准则进行审计。
- [x] **Stream-First**: 审查全部模块是否坚守微缓冲拉取模型与零内存假设。
- [x] **Invariant-First**: 审查安全是否严格下沉至 POSIX AT-API 与延后 Fixup。
- [x] **Bounds-First**: 审查 Magic 哨兵、memset_s 与 Clamp 确界。
- [x] **Oracle-First**: 审查测试套件是否具备真实历史缺陷与系统 CLI 差分预言机。

---

## Phase 0: Research Items (Subagent Dispatches)

- - R001 [SUBAGENT:research] 《C 桥接层与底层引擎全量系统不变量审查》：针对 `Sources/CTTZipBridge/` 下全部文件，扫描全量内存分配、安全选项、Magic 状态与敏感清零。
- - R002 [SUBAGENT:research] 《Swift 核心引擎热路径与设计模式数据平面合规审查》：针对 `Sources/TTZipCore/`，扫描 `Data(count:)`、锁使用、路径清洗与跨语言转换。
- - R003 [SUBAGENT:research] 《测试套件客观预言机覆盖率与 UI 隔离架构审查》：针对 `Tests/` 与 `Sources/TTZipApp/`，扫描同义反复测试、差分测试覆盖率与分层边界。

---

## Phase 1: Artifacts & Contracts

- `data-model.md`: 定义 `CodebaseAuditReportSpec` 与 `DefectItem` 数据模型。
- `contracts/codebase_audit_spec.json`: 强类型全仓库审计契约。
- `quickstart.md`: 审计报告完整性与缺陷矩阵自测验证指南。

---

## Phase 2: Implementation Checklist

### 1. 执行全量子 Agent 深度物理扫描
- [ ] 派遣子 Agent 1：C 桥接层不变量深度扫描
- [ ] 派遣子 Agent 2：Swift Core 核心管道与模式热路径扫描
- [ ] 派遣子 Agent 3：测试套件预言机与 UI 层隔离扫描

### 2. 整合生成全景综合审计报告
- [ ] 汇总生成 `specs/040-comprehensive-systemic-invariants-codebase-audit/research.md`
- [ ] 编纂生成 `docs/architecture/comprehensive_systemic_audit_report.md`

### 3. 契约固化与质量门禁
- [ ] 生成 `contracts/codebase_audit_spec.json`
- [ ] 执行自测验证
