# Feature Specification: SPM Benchmark Target Isolation & `ttzip-bench` CLI Tooling

**Feature Branch**: `136-spm-benchmark-target-isolation-and-cli-tooling`  
**Created**: 2026-08-20  
**Status**: Approved  
**Input**: User directive: "完成 SPM 物理架构拆分与 ttzip-bench 独立 CLI 工具交付，剥离核心库绘图与竞品运行死代码，缩减包体积并提供独立性能评测 CLI。"

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 独立 `ttzip-bench` 可执行工具与子命令矩阵 (Priority: P1)

作为一名性能工程师或开源贡献者，我希望在终端中通过 `swift run ttzip-bench` 运行独立的基准评测工具，支持 `matrix`（50 点纯内存三维矩阵）、`plot`（Pareto 绘图与竞品 PK）和 `gate`（性能回归断言），而无需将这些重量级代码混入核心生产库中。

**Why this priority**: 为 TTZip 提供开箱即用的工业级基准评测 CLI，对标 Google Benchmark 与 zlib-ng `deflate_bench`。

**Independent Test**: 在终端运行 `swift run ttzip-bench matrix`，验证其在 $\le 1.2	ext{ s}$ 内输出 50 点延迟、吞吐率与压缩率三维终端表格。

**Acceptance Scenarios**:
1. **Given** 开发者在终端执行 `swift run ttzip-bench matrix`, **When** 运行 50 点测试，**Then** 输出格式化终端对齐表格，并在最后打印通过摘要与 $CV$ 统计。
2. **Given** 开发者传入 `--json-out <path>`, **When** 命令执行完毕，**Then** 将完整的基准测试数据写入指定 JSON 文件，符合 JSON Schema 契约。

---

### User Story 2 - 核心库 `TTZipCore` 瘦身与绘图代码物理剥离 (Priority: P1)

作为一名应用打包与发布工程师，我希望 `TTZipCore` 仅包含核心归档与压缩解压管线代码，将 `RasterParetoPlotter.swift`（33KB）、`TerminalParetoPlotter.swift` 等绘图引擎从 `TTZipCore` 彻底剥离并移至 `ttzip-bench` 模块，从而减少 `TTZipCore.framework` 与 `TTZipApp` 的二进制发布体积。

**Why this priority**: 彻底清除生产发布包中的无用绘图与竞品运行死代码（Dead Code），保持核心引擎的极简纯粹。

**Independent Test**: 编译 `TTZipCore` 静态库，验证其不再依赖 `RasterParetoPlotter` 等绘图符号，且所有核心单测 100% 编译通过。

**Acceptance Scenarios**:
1. **Given** 检查 `Sources/TTZipCore/` 代码库，**When** 搜索 Pareto 图像绘制与竞品分析模块，**Then** 确认这些模块已移至 `Sources/TTZipBench/` 或独立 Target 中。
2. **Given** 运行 `swift build --target TTZipCore`, **When** 编译完成，**Then** 0 警告，0 符号丢失。

---

### User Story 3 - 日常单测构建耗时压减至极限 (Priority: P2)

作为一名日常编写业务逻辑的开发者，我希望运行 `swift test` 时不需要处理任何重量级绘图或长时竞品 PK 的链接符号，使单测的编译链接与运行在 2 秒内完成。

**Why this priority**: 缩短本地与 CI 循环周期，提升开发迭代效率。

**Independent Test**: 测量 `swift test` 耗时，确保单测执行稳定、极速且 100% 通过。

**Acceptance Scenarios**:
1. **Given** 执行 `swift test`, **When** 运行全套测试，**Then** 0 失败，全部通过。

---

## Edge Cases

- **未传递子命令**: 用户直接运行 `ttzip-bench` 时，必须输出简洁的 Help 帮助菜单（`matrix`, `plot`, `gate`, `help`）。
- **无效参数处理**: 传入不支持的格式或级别时，输出明确的 POSIX sysexit 错误提示，避免崩溃。

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: `Package.swift` 必须定义 `.executableTarget(name: "ttzip-bench", dependencies: ["TTZipCore", "CTTZipBridge"])`。
- **FR-002**: 必须创建 `Sources/TTZipBench/` 目录，并将绘图与独立评测引擎移入该 Target。
- **FR-003**: `ttzip-bench` 必须支持 `matrix` 子命令，直接调用 50 点内存矩阵并支持 `--json-out`。
- **FR-004**: `ttzip-bench` 必须支持 `--help` 与简洁的 CLI 路由派发。
- **FR-005**: 必须保证 `TTZipCore`、`TTZipApp`、`ttzip-cli` 与 `TTZipTests` 在重构后 100% 正常编译和测试通过。

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `swift run ttzip-bench matrix` 在 $\le 1.2$ 秒内执行完毕，完整输出 50 点三维指标。
- **SC-002**: `TTZipCore` 移除所有无用的绘图代码（净移出 1000+ 行代码），二进制符号保持极简。
- **SC-003**: 全量 `swift test` 100% 通过（0 失败，0 警告）。
