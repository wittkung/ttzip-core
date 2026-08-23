# Implementation Plan: SPM Benchmark Target Isolation & `ttzip-bench` CLI Tooling

**Feature Directory**: `specs/136-spm-benchmark-target-isolation-and-cli-tooling`  
**Target Subject**: SPM 物理拆分、`TTZipBench` 独立可执行模块与核心库瘦身  
**Status**: Approved  

---

## 1. Technical Context & Constitution Check

### 1.1 Technical Context
- **现有问题**：
  1. `Sources/TTZipCore/Benchmark/RasterParetoPlotter.swift` (33KB)、`SVGParetoPlotter.swift`、`TerminalParetoPlotter.swift` 存放在核心库中，引入了 AppKit/GUI 图形渲染逻辑，属于生产环境中的死代码。
  2. 缺乏独立的基准测试 CLI 工具，外部 CI 和性能开发者无法直接通过 `swift run ttzip-bench` 获得一键式 50 点纯内存压测与 JSON 导出能力。
- **治理目标**：
  1. 在 `Package.swift` 中增加 `.executableTarget(name: "TTZipBench")` 并暴露 `ttzip-bench` 命令行工具。
  2. 将 4 个绘图与宏观评测文件从 `Sources/TTZipCore/` 物理迁移至 `Sources/TTZipBench/`。
  3. 构建轻量、快速的 `@main struct TTZipBenchApp` 路由器（支持 `matrix`, `plot`, `gate`, `help`）。

### 1.2 Constitution Check
- ✅ **Upstream Invariant 1 (Zero-Overhead Hot Path)**：核心库 `TTZipCore` 摆脱 GUI 依赖，二进制符号保持极简。
- ✅ **Upstream Invariant 2 (Deterministic Grounded Benchmarks)**：`ttzip-bench` 承载 50 点三维纯内存矩阵，输出标准 JSON Schema 契约。
- ✅ **Upstream Invariant 5 (100% Zero-Warning & Clean Hook Policy)**：0 编译警告，单测 100% 通过。

---

## 2. Phase 0: Technical Research Index

- R001 [SUBAGENT:research] 《Package.swift 中新增 `ttzip-bench` 可执行 Target 的依赖拓扑与编译配置》：独立 Executable Target 定义与跨平台时钟适配。
- R002 [SUBAGENT:research] 《绘图引擎与宏观评测代码从 `TTZipCore` 物理迁移至 `Sources/TTZipBench/` 的符号解耦》：物理迁移 4 个文件并消除 `QuartzCore` 异味。
- R003 [SUBAGENT:research] 《`ttzip-bench` 统一 CLI 交互架构与子命令设计》：实现 `@main` 路由与四大核心子命令。

---

## 3. Phase 1: Design Artifacts & Contracts

- `data-model.md`: `BenchmarkCliCommand`, `BenchmarkCliReport` 实体定义。
- `contracts/bench_cli_matrix_schema.json`: 50 点矩阵遥测 JSON 契约。
- `contracts/bench_cli_gate_verdict.json`: 性能门禁判决契约。
- `quickstart.md`: 2 大核心场景验收手册。

---

## 4. Component Changes & Architecture

### 4.1 Component 1: SPM Package Configuration
- [MODIFY] `Package.swift`: 添加 `ttzip-bench` 可执行产品与 `TTZipBench` target。

### 4.2 Component 2: Physics Relocation & Decoupling
- [NEW] `Sources/TTZipBench/Plotters/RasterParetoPlotter.swift`: 迁移自 `TTZipCore`。
- [NEW] `Sources/TTZipBench/Plotters/SVGParetoPlotter.swift`: 迁移自 `TTZipCore`。
- [NEW] `Sources/TTZipBench/Plotters/TerminalParetoPlotter.swift`: 迁移自 `TTZipCore`。
- [NEW] `Sources/TTZipBench/Runners/ExhaustiveBenchmarkRunner.swift`: 迁移并改用 `PlatformMonotonicTimer`。
- [DELETE] `Sources/TTZipCore/Benchmark/RasterParetoPlotter.swift`
- [DELETE] `Sources/TTZipCore/Benchmark/SVGParetoPlotter.swift`
- [DELETE] `Sources/TTZipCore/Benchmark/TerminalParetoPlotter.swift`
- [DELETE] `Sources/TTZipCore/ExhaustiveBenchmarkRunner.swift`

### 4.3 Component 3: `ttzip-bench` Entry Point & Router
- [NEW] `Sources/TTZipBench/main.swift`: `@main` 入口与子命令解析。
