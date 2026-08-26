# Feature Specification: 基于四大系统工程铁律的 TTZip 全仓库深度代码审计 (Comprehensive Codebase Audit on Systemic Invariants)

**Feature Directory**: `specs/040-comprehensive-systemic-invariants-codebase-audit`  
**Created**: 2026-08-16  
**Status**: Draft  
**Input**: "基于新确立的四大系统工程铁律（Stream-First, Invariant-First, Bounds-First, Oracle-First），对 TTZip 完整仓库（CTTZipBridge, TTZipCore, TTZipApp, TTZipCLI, Tests, Scripts）进行全面、深度、系统级物理审计，输出全景缺陷清单、风险分级矩阵与重构排期路线图"

---

## Clarifications

### Session 2026-08-16
- **Q1 (审计报告粒度与标准)**: 审计报告需要细化到什么粒度？
  - **Resolution**: 细化到具体文件、行号区间、所属铁律维度（Stream-First, Invariant-First, Bounds-First, Oracle-First）、风险级别（P0 阻塞安全 / P1 稳定性防 OOM / P2 热路径性能 / P3 架构确界规范）及具体修复方案。
- **Q2 (多 Agent 并行扫描策略)**: 针对 C Bridge、Swift Core、Tests/App 采用何种扫描机制？
  - **Resolution**: 根据 §1.8 调度规则，通过子 Agent 隔离并行执行源码级 grep 与 AST/正则静态分析，汇总生成全局审计报告。


## User Scenarios & Testing *(mandatory)*

### User Story 1 - C 桥接层与底层引擎系统级不变量全量扫描 (Priority: P1)

作为核心安全与架构负责人，我需要对 `Sources/CTTZipBridge/` 下全部 20+ 个 C 源文件进行逐行静态扫描与模式审查，排查所有涉及：(1) 假设内存无限的 `malloc` / `posix_memalign`；(2) 遗漏 `ARCHIVE_EXTRACT_SECURE_*` 标志位的落盘点；(3) 析构前未清零 `magic = 0` 的结构体；(4) 释放前未调用 `memset_s` 的敏感缓冲区；(5) 裸整型强转与缺少 `SSIZE_MAX` Clamp 的跨语言接口。

**Why this priority**: C 桥接层是系统底层与硬件交互的物理边界，任何未定义行为均会导致进程崩溃或 CVE 级安全漏洞。

**Independent Test**: 生成 `docs/architecture/audit_c_bridge_layer.md`，逐文件定位风险行号、缺陷类型与修复建议。

**Acceptance Scenarios**:
1. **Given** `Sources/CTTZipBridge/*.c`，**When** 执行系统不变量审计，**Then** 完整识别出所有违背 Stream-First、Invariant-First、Bounds-First 的代码点并给出精准行号。

---

### User Story 2 - Swift 核心管道与设计模式数据平面合规审计 (Priority: P2)

作为性能架构师，我需要对 `Sources/TTZipCore/`（涵盖 28 大设计模式、并发管道、解压缩与扫描引擎）进行热路径与内存模型审查，排查：(1) 热循环中是否存在 `Data(count:)` 导致的内核零填充中断；(2) 并发闭包内是否存在加锁借还享元池或动态分配；(3) 路径清洗是否与 POSIX 原语防御形成完整闭环。

**Why this priority**: 捍卫热路径零成本抽象与吞吐门禁，防止应用层思维侵入数据平面。

**Independent Test**: 生成 `docs/architecture/audit_swift_core_layer.md`，对核心管道与设计模式进行系统级合规评审。

**Acceptance Scenarios**:
1. **Given** `Sources/TTZipCore/**/*.swift`，**When** 执行热路径与内存审计，**Then** 识别出所有潜在内核页中断与动态堆分配点。

---

### User Story 3 - 测试套件预言机有效性与应用层防御审计 (Priority: P3)

作为质量保证与前端架构师，我需要对 `Tests/` 套件（525+ 测试用例）与 `Sources/TTZipApp/`（UI 树构建、IME 兼容、@MainActor 调度）进行有效性审计，排查：(1) 测试套件中是否存在“自产自销”的同义反复用例；(2) 是否缺少关键格式的系统 CLI 双向差分测试与 `.uu` 历史缺陷覆盖；(3) UI 层与 Bridge 层是否存在违规跨层依赖。

**Why this priority**: 确保测试套件具备客观预言机属性，UI 层与引擎层严格物理隔离。

**Independent Test**: 生成 `docs/architecture/audit_testing_and_app_layer.md`，输出测试预言机成熟度评估与 UI 架构合规报告。

**Acceptance Scenarios**:
1. **Given** 测试与 UI 代码库，**When** 执行预言机有效性审计，**Then** 明确指出哪些格式缺乏差分测试并给出补充计划。

---

## Edge Cases

- **已冻结文件的审计边界**：对于处于完全冻结状态的文件（如 `ZipParallelExtractor.swift`、`CTTZipExtract.c`），审计应记录其现状并对比不变量规范，若需修改需标明需 `FORCE UNFREEZE ZIP`。
- **第三方 Vendor 库边界**：区分自研桥接代码与纯净 upstream 源码，避免对 upstream 提出违背其自身架构的修改。

---

## Requirements *(mandatory)*

- **FR-001**: 必须对 `Sources/CTTZipBridge/` 执行逐文件物理扫描，输出 C 桥接层不变量审计清单。
- **FR-002**: 必须对 `Sources/TTZipCore/` 执行逐模块扫描，输出 Swift 核心管道与模式合规审计清单。
- **FR-003**: 必须对 `Tests/` 与 `Sources/TTZipApp/` 执行扫描，输出测试预言机与 UI 隔离审计清单。
- **FR-004**: 必须在 `docs/architecture/comprehensive_systemic_audit_report.md` 中汇总生成《TTZip 全仓库系统级不变量审计综合全景报告》，包含缺陷分级矩阵与重构排期路线图。
- **FR-005**: 必须在 `contracts/` 中固化全仓库审计元数据规范契约。

---

## Success Criteria *(mandatory)*

- **SC-001**: 100% 覆盖 C 桥接层 20+ 个源文件的逐行系统级审查。
- **SC-002**: 100% 覆盖 Swift Core 28 大设计模式与并发管道的热路径审查。
- **SC-003**: 产出包含精准文件名、代码行号、缺陷危害与修复建议的综合全景审计报告。
- **SC-004**: 契约文件通过 Draft-07 强约束与零裸通配校验。
