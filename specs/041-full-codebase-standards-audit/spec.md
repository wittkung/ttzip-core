# Feature Specification: 基于项目规范与最高行业标准的 TTZip 全代码库深度审计 (Full Codebase Standards & Architectural Audit)

**Feature Branch**: `041-full-codebase-standards-audit`  
**Feature Directory**: `specs/041-full-codebase-standards-audit`  
**Created**: 2026-08-17  
**Status**: Draft  
**Input**: "详细基于项目规范和最高标准的代码规范完整审计所有代码 /speckit-specify"

---

## Clarifications

### Session 2026-08-17
- **Q1 (审计标准体系与覆盖范围)**: 审计体系覆盖哪些维度与代码范围？
  - **Resolution**: 覆盖四大系统工程铁律（Stream-First, Invariant-First, Bounds-First, Oracle-First）、28 大设计模式规范、C 桥接层内存/溢出/生命周期安全、Swift 6 并发与热路径零成本抽象、Apple Silicon 硬件加速旁路、测试预言机完整性以及 UI/AppKit 线程与 IME 规范，100% 覆盖全库各层（C 桥接层、Swift 核心引擎层、应用与 CLI 层、测试套件与脚本工具）。
- **Q2 (缺陷判定与风险分级模型)**: 缺陷如何进行风险定级与验收闭环？
  - **Resolution**: 采用 P0（致命安全与阻断性崩溃）、P1（严重稳定性与内存/资源失控）、P2（性能倒退与热路径架构违规）、P3（低危代码异味与命名/文档规范）四级模型，输出全景缺陷审计矩阵与针对性重构路线图。

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 底层 C 桥接与汇编硬件加速层系统级安全与确界审计 (Priority: P1)

作为系统安全与底层架构负责人，我需要对底层 C 桥接层与汇编微内核（`Sources/CTTZipBridge/`、`Vendor/` 绑定及头文件体系）执行逐行物理安全与架构审查，排查：(1) 路径穿越与符号链接安全标志缺失；(2) 敏感凭据（密码、密钥、IV）未进行物理强制安全擦除；(3) 结构体生命周期缺少魔数（Magic）哨兵保护；(4) 64 位整型与跨语言接口缺少上限 Clamp 与硬件防溢出保护；(5) 假设内存无限的一次性全量分配。

**Why this priority**: 底层 C 桥接与硬件加速层是直接与操作系统内核及硬件寄存器交互的物理边界，任何指针悬垂、整型溢出或内存泄漏都会直接引发段错误崩溃或 CVE 级安全隐患。

**Independent Test**: 产出底层 C 桥接层深度审计清单，定位每一个违规项的文件路径、代码行号、缺陷危害与修复建议，确保 0 处漏报。

**Acceptance Scenarios**:
1. **Given** C 桥接与汇编层代码库，**When** 依据系统工程铁律执行边界与生命周期审计，**Then** 完整识别出所有违背 Stream-First、Invariant-First 与 Bounds-First 的隐患点，精确定位行号并给出修复方案。
2. **Given** 涉及密码派生与加解密的 C 源文件，**When** 扫描内存清理逻辑，**Then** 100% 识别出普通 `memset` 或未清理的敏感缓冲区并提供 `memset_s` / volatile 擦除改写方案。

---

### User Story 2 - Swift 6 核心管道与 28 大设计模式数据平面合规审计 (Priority: P2)

作为性能架构师，我需要对 Swift 核心引擎层（`Sources/TTZipCore/` 下 28 大设计模式、并行管道、编解码适配器与目录扫描器）进行热路径与内存模型合规审查，排查：(1) 热循环中是否存在 `Data(count:)` 导致的内核零填充缺页中断；(2) 并发闭包与 `concurrentPerform` 内部是否存在加锁借还、动态对象树（Composite/Visitor/Decorator）动态堆分配；(3) 结构体 Builder 是否存在丢失状态或返回值未捕获的缺陷；(4) 跨语言指针传递是否 100% 走安全适配中枢且生命周期闭环。

**Why this priority**: 捍卫热路径零成本抽象与各格式历史最优性能硬门禁，杜绝在并发数据平面引入阻塞式锁竞争或动态分配，保持设计模式仅在调度层与冷路径运作。

**Independent Test**: 产出 Swift Core 核心管道与设计模式专项审计报告，逐一核对 28 大设计模式映射表与反模式禁令。

**Acceptance Scenarios**:
1. **Given** Swift 核心管道与设计模式实现，**When** 执行热路径零成本抽象审查，**Then** 完整识别出数据平面中违规的堆分配、锁竞争与动态树构造。
2. **Given** 涉及 C 指针交互与生命周期的 Swift 代码，**When** 执行跨语言安全审查，**Then** 验证所有指针操作是否严格通过安全适配器且无异步逃逸。

---

### User Story 3 - 应用交互层架构规范与测试套件真实预言机审计 (Priority: P3)

作为质量保证与桌面端架构师，我需要对应用表现层（`Sources/TTZipApp/`、`Sources/TTZipCLI/`）与测试套件（`Tests/TTZipTests/`）执行全面审计，排查：(1) UI 交互中是否存在违规跨层依赖底层 C 库；(2) Popover 与 Sheet 中是否存在导致 macOS 输入法阻塞的输入框组件；(3) 主线程调度与异步取消机制是否完善；(4) 测试套件中是否存在“自产自销”伪通过用例，是否覆盖黄金历史缺陷语料库（Golden Corpus）与系统原生 CLI 双向差分测试。

**Why this priority**: 保证桌面端与命令行终端的交互极致流畅与稳定性，同时确保测试套件具备客观预言机效力，真实拦截历史与未来可能出现的回归缺陷。

**Independent Test**: 产出应用层与测试套件审计报告，明确指出 UI 规范合规度与测试预言机成熟度评估。

**Acceptance Scenarios**:
1. **Given** UI 视图与状态管理代码，**When** 扫描架构依赖与组件规范，**Then** 识别出所有跨层引用、非 `@MainActor` 调度隐患及 IME 兼容性问题。
2. **Given** 测试套件与变异模糊测试，**When** 审计断言有效性与预言机设计，**Then** 识别出空跑、同义反复与缺乏跨工具差分的测试用例。

---

## Edge Cases

- **冻结文件（Frozen Files）的处理规范**：对于处于完全冻结状态的文件（如 `ZipParallelExtractor.swift`、`CTTZipExtract.c` 等），审计应客观列出其现状与改进建议，但严格标明需要 `FORCE UNFREEZE ZIP` 授权后方可改动。
- **第三方纯净上游代码边界**：对于 `Vendor/libarchive-upstream` 等外部依赖，严格区分自研桥接代码与纯净 upstream 源码，审查依据应符合 upstream-contribution 规范而非强加项目特化约束。
- **不同硬件架构下的条件编译审计**：审查所有 `#if arch(arm64)` 与 `#if !MAS_BUILD` 条件分支，确保在 Apple Silicon 与 Intel、MAS 沙盒与独立分发全矩阵下均有对齐的 fallback 与自解释注释。

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: 审计系统 MUST 覆盖 `Sources/CTTZipBridge/`、`Sources/TTZipCore/`、`Sources/TTZipApp/`、`Sources/TTZipCLI/`、`Tests/` 与构建脚本，执行全量静态与架构级逐行审查。
- **FR-002**: 审计系统 MUST 依据四大系统工程铁律（Stream-First、Invariant-First、Bounds-First、Oracle-First）对全库代码进行分类诊断并输出行号级证据。
- **FR-003**: 审计系统 MUST 严格审查 28 大设计模式在 TTZip 中的使用边界，核查是否存在侵入热路径数据平面的反模式。
- **FR-004**: 审计系统 MUST 审查所有跨语言 C-Swift 接口、整型窄化转换、Magic 魔数哨兵与敏感内存清零逻辑。
- **FR-005**: 审计系统 MUST 审查 UI 层线程模型、输入法兼容性与分层依赖隔离。
- **FR-006**: 审计系统 MUST 审查测试套件真实预言机效力（Golden Corpus 覆盖率、差分测试、模糊测试有效性）。
- **FR-007**: 审计系统 MUST 产出结构化的综合审计全景报告与 P0/P1/P2/P3 风险缺陷分级矩阵，并给出明确的重构修复路线图。

---

## Key Entities *(include if feature involves data)*

- **DefectEntry (缺陷实体)**:
  - `id`: 唯一标识符（如 P0-01, P1-03）
  - `module`: 归属模块与源码相对路径
  - `lines`: 涉及代码行号区间
  - `invariantCategory`: 归属铁律/规范维度（Stream-First, Invariant-First, Bounds-First, Oracle-First, DesignPattern, ThreadSafety）
  - `severity`: 严重级别（P0, P1, P2, P3）
  - `description`: 缺陷描述与潜在危害
  - `remediation`: 针对性修复建议与技术方案
- **AuditReport (审计全景报告)**:
  - `summary`: 总体审计数据与缺陷统计概览
  - `defectMatrix`: 按严重程度分类的结构化缺陷表
  - `roadmap`: 分阶段系统重构与修复排期

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% 覆盖全库 170+ 源文件的逐行代码规范与架构审计。
- **SC-002**: 完整输出包含 P0/P1/P2/P3 四级缺陷矩阵与精确源码位置的综合审计报告。
- **SC-003**: 审计报告中针对每个缺陷均提供明确、可落地且符合架构规范的修复决议。
- **SC-004**: 确立清晰可执行的四阶段系统重构与缺陷闭环路线图。

---

## Assumptions

- 审计工作采用静态扫描、代码审查与架构分析相结合的方式，不破坏现有功能基线。
- 审计报告作为后续重构与质量提升的唯一权威输入源。
- 修复涉及的冻结文件需单独提请解冻授权。
