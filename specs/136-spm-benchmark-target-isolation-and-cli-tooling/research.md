# Phase 0 Technical Research: SPM Benchmark Target Isolation & `ttzip-bench` CLI Tooling

**Feature Directory**: `specs/136-spm-benchmark-target-isolation-and-cli-tooling`  
**Status**: Completed  
**Author**: Research Subagent `7362c08a-1e05-4fb1-9b19-1474dcfb6f4f`  

---

## 1. R001: Package.swift 中新增 `ttzip-bench` 可执行 Target 的依赖拓扑与编译配置

### 决策 (Decision)
在 `Package.swift` 中新增独立可执行产品 `ttzip-bench`，并配置对应的可执行 Target `TTZipBench`：

```swift
// Package.swift products:
.executable(
    name: "ttzip-bench",
    targets: ["TTZipBench"]
)

// Package.swift targets:
.executableTarget(
    name: "TTZipBench",
    dependencies: [
        "TTZipCore",
        "CTTZipBridge"
    ],
    swiftSettings: [
        .unsafeFlags(["-no-whole-module-optimization", "-enable-batch-mode"])
    ]
)
```

跨平台编译与时钟适配策略：
- **macOS 环境**：支持全功能运行。光栅绘图模块 `RasterParetoPlotter.swift` 使用 `#if canImport(AppKit)` 条件编译；高精度时钟通过 `PlatformMonotonicTimer` 绑定 Apple Silicon `mach_absolute_time`。
- **Linux 环境**：支持无头（Headless）服务化与 CI 评测。`SVGParetoPlotter.swift`（纯 Swift 矢量渲染）和 `TerminalParetoPlotter.swift`（Unicode Braille 终端点阵渲染）实现 100% 跨平台免 GUI 依赖运行；时钟通过 C 层 `CTTZipPlatformTimer.h` 调用 `clock_gettime(CLOCK_MONOTONIC_RAW)`。
- **编译参数**：继承现有项目的 `.unsafeFlags(["-no-whole-module-optimization", "-enable-batch-mode"])` 设置，保持增量构建性能与符号一致性。

### 理由 (Rationale)
- **职责清晰与二进制精简**：生产核心库 `TTZipCore` 应当保持极简，杜绝引入 AppKit 图形渲染与复杂样条曲线插值逻辑。
- **双层依赖拓扑支持**：`TTZipBench` 既需要调用 `TTZipCore` 现有的高层组件（如 `ArchivePipelineBuilder`、`ArchiveEngineFactory`、`PlatformMonotonicTimer`），也需要直接访问 `CTTZipBridge` 的底层硬件加速 C 函数（如 `ttzip_libdeflate_compress`、`ttzip_zstd_compress`、`LZ4_compress_default`）进行微基准测量，显式声明两项依赖可实现零层级跨越开销。
- **编译与构建隔离**：独立 Target 允许开发者或 CI 流水线单独执行 `swift run ttzip-bench` 或运行针对性的测试，无需引入 App 视图层或外部非必要依赖（如 Sparkle）。

### 被否决的替代方案 (Alternatives Considered)
- **方案 A：将基准与绘图功能继续保留在 `TTZipCLI` 中（即 `ttzip-cli bench`）**
  - *否决理由*：`ttzip-cli` 是面向最终用户的日常生产归档工具（涵盖压缩、解压、校验、Diff、密码恢复等）。集成重型学术绘图和全矩阵评测会导致 CLI 二进制体积膨胀、职责混淆，且无法为 CI 门禁提供专属的轻量化参数规范。
- **方案 B：仅通过 XCTest (`TTZipTests`) 执行评测，不单独发布可执行二进制**
  - *否决理由*：XCTest 强依赖测试宿主运行器，无法在独立容器或非测试环境中直接作为 CLI 工具输出标准 exit code、实时 NDJSON 流或导出 SVG/PNG 图像文件，不便于与外部自动化脚本集成。

### 查阅源 (Source)
- `Package.swift`: Lines 1-98（现有的 `products`, `targets`, `CTTZipBridge`, `TTZipCore`, `TTZipCLI` 配置）。
- `Sources/TTZipCore/Platform/PlatformMonotonicTimer.swift`: Lines 1-100（跨平台单调纳秒时钟实现）。
- `Sources/CTTZipBridge/include/CTTZipPlatformTimer.h`: Lines 1-35（跨平台底盘时钟接口）。

---

## 2. R002: 绘图引擎与宏观评测代码从 `TTZipCore` 物理迁移至 `Sources/TTZipBench/` 的符号解耦

### 决策 (Decision)
在物理目录结构中建立 `Sources/TTZipBench/`，将以下 4 个高内聚评测/绘图文件移出 `Sources/TTZipCore/`：
1. `Sources/TTZipCore/Benchmark/RasterParetoPlotter.swift` ➔ `Sources/TTZipBench/Plotters/RasterParetoPlotter.swift`
2. `Sources/TTZipCore/Benchmark/SVGParetoPlotter.swift` ➔ `Sources/TTZipBench/Plotters/SVGParetoPlotter.swift`
3. `Sources/TTZipCore/Benchmark/TerminalParetoPlotter.swift` ➔ `Sources/TTZipBench/Plotters/TerminalParetoPlotter.swift`
4. `Sources/TTZipCore/ExhaustiveBenchmarkRunner.swift` ➔ `Sources/TTZipBench/Runners/ExhaustiveBenchmarkRunner.swift`

迁移与解耦步骤：
1. **解除系统框架异味**：修改 `ExhaustiveBenchmarkRunner.swift`，移除 `import QuartzCore` 以及 `CACurrentMediaTime()` 调用，统一替换为 `PlatformMonotonicTimer.nowSeconds()`。
2. **符号边界划分**：`TTZipCore` 保留基础计时器（`PlatformMonotonicTimer`）、内存缓冲池（`BenchmarkBufferPool`）与语料枚举（`BenchmarkCorpusType`）；所有图表生成与全矩阵测试逻辑全部封装在 `TTZipBench` 中。
3. **测试引用对齐**：更新针对绘图器和全矩阵的单元测试（如 `SVGParetoPlotterTests.swift` 等），改用 `@testable import TTZipBench`。

### 理由 (Rationale)
- **清除 Core 中的 GUI 符号绑定**：`RasterParetoPlotter.swift` 拥有 747 行基于 `AppKit` 与 `CoreGraphics` 的绘图实现（`CGContext`, `NSBitmapImageRep`, `NSAttributedString`）。将其移出后，`TTZipCore` 完全摆脱了 GUI 框架依赖。
- **验证零破坏性**：代码库全量 Grep 扫描结果表明，`TTZipApp` 的 UI 界面仅引用了 `BenchmarkEngine` 与 `FrontendBenchmarkRunner`，**从未引用**上述 4 个文件中的任何类型（`RasterParetoPlotter`, `SVGParetoPlotter`, `TerminalParetoPlotter`, `ExhaustiveBenchmarkRunner`）。迁移操作对生产 App 零影响。

### 被否决的替代方案 (Alternatives Considered)
- **方案 A：将绘图器保留在 `TTZipCore` 中，仅使用 `#if canImport(AppKit)` 进行条件编译隔离**
  - *否决理由*：虽然条件编译可保证 Linux 编译通过，但 Core 模块的语义边界仍然被污染，增加了核心库的代码维护负担和单元测试依赖。
- **方案 B：将绘图逻辑封装为独立的三方动态库（Dynamic Framework）**
  - *否决理由*：过度设计。TTZip 是独立的 Swift 6.0 现代化单体架构项目，通过 SPM Target 物理隔离即已获得最清晰的编译屏障，无需引入外部动态库的分发与加载成本。

### 查阅源 (Source)
- `Sources/TTZipCore/Benchmark/RasterParetoPlotter.swift`: Lines 1-747（AppKit / CoreGraphics 渲染实现）。
- `Sources/TTZipCore/Benchmark/SVGParetoPlotter.swift`: Lines 1-290（SVG 矢量生成器）。
- `Sources/TTZipCore/Benchmark/TerminalParetoPlotter.swift`: Lines 1-177（Unicode Braille 终端点阵实现）。
- `Sources/TTZipCore/ExhaustiveBenchmarkRunner.swift`: Lines 1-250（包含 `CACurrentMediaTime()`）。
- `Sources/TTZipApp/Views/Benchmark/BenchmarkViewModel.swift`: Lines 1-400（审计确认：完全未引用上述 4 个模块）。

---

## 3. R003: `ttzip-bench` 统一 CLI 交互架构与子命令设计

### 决策 (Decision)
在 `Sources/TTZipBench/` 中实现 `@main struct TTZipBenchApp` 入口，构建结构化的子命令路由器，包含四大核心子命令：

1. **`matrix`（矩阵评测）**：
   - 运行 50 点纯内存多算法压测矩阵（对接 `TTZipCoreCodecBenchmarks.run50PointMatrix()`）。
   - 支持参数：`--json-out <path>`。
2. **`plot`（图表生成）**：
   - 基于评测结果生成 2D 帕累托前沿图表（终端 / SVG / PNG）。
3. **`gate`（CI/CD 性能门禁）**：
   - 执行自动化回归检验，对比吞吐（MB/s）、压缩率（%）、变异系数（CV %）与数据完整性。
   - 若指标未达标或发生显著性能退化，直接以 POSIX 非零状态码退出（如 `EX_SOFTWARE = 70`），并输出格式化的诊断报告。
4. **`help`（帮助文档）**：
   - 打印标准 UNIX 格式的命令说明、参数选项与使用示例。

**JSON Schema 契约设计**：
遵循 `http://json-schema.org/draft-07/schema#` 元规范，定义严格的强类型报表模型（杜绝裸 `object`）：
- 包含 `timestamp` (epoch seconds)、`hardware` (OS, CPU)、`summary` (totalPoints, totalDurationMs, medianCvPercentage, gatePassed) 以及结构化的 `points` 数组。

### 理由 (Rationale)
- **CI 自动化与可观测性**：通过 `gate` 命令与标准 exit code / JSON 契约，CI 流水线能够无需依赖正则匹配终端文本，直接获取结构化评测指标。
- **微秒级即时反馈**：`TTZipCoreCodecBenchmarks.run50PointMatrix()` 在纯内存 0 额外堆分配模式下执行 50 个基准测试点，耗时通常 <1.2s，适合作为 pre-commit 或 local hook 的极速回归门禁。
- **轻量无外部依赖**：沿用项目内成熟的轻量化参数解析模式，冷启动时间控制在 3ms 以内，无需引入庞大的外部参数库。

### 被否决的替代方案 (Alternatives Considered)
- **方案 A：引入 `swift-argument-parser` 外部依赖包**
  - *否决理由*：TTZip 工程坚持核心链路零非必要外部依赖的原则。手写轻量级 POSIX 参数解析器逻辑清晰、启动迅速，且完全满足 `matrix`, `plot`, `gate`, `help` 的参数解析需求。
- **方案 B：仅输出控制台格式化表格，不提供 JSON Schema 契约导出**
  - *否决理由*：终端 ASCII 表格无法被自动化运维工具、Grafana 仪表盘或 CI 评测流水线稳定消费，必须提供强类型的 JSON Schema 契约。

### 查阅源 (Source)
- `Sources/TTZipCore/Benchmark/TTZipCoreCodecBenchmarks.swift`: Lines 1-247（`CodecBenchmarkPointResult`, `CodecBenchmarkMatrixSummary`, `run50PointMatrix()`）。
- `Sources/TTZipCore/Benchmark/ParetoFrontierCalculator.swift`: Lines 1-120（帕累托前沿计算引擎）。
- `Sources/TTZipCLI/CLICommandRouter.swift`: Lines 21-73（`CLIEventAndProgressConsoleObserver`, NDJSON 输出模式参考）。
