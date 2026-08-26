# Feature Specification: Test Suite Architecture Governance, Target Isolation & Unified Corpus Infrastructure

**Feature Branch**: `135-test-suite-governance-and-target-isolation`  
**Created**: 2026-08-20  
**Status**: Approved  
**Input**: User request: "好好看看我们现在的测试体系呢" -> 测试体系架构治理、独立 Benchmark 目标物理隔离、统一语料基础设施与 CI 性能门禁集成。

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 统一语料基础设施与零冗余数据源 (Priority: P1)

作为一名底层核心开发者，我希望整个测试体系使用统一的 8 大确定性语料生成器（`BenchmarkCorpusGenerator` / `CTTZipCorpusGen`），彻底消除散落在各个测试文件中的临时 PRNG、硬编码字符串合成与冗余 fixture 文件，从而保证测试数据的物理确定性与代码库的极简整洁。

**Why this priority**: 消除多套语料逻辑的维护分歧，确保微基准、宏观测试与差分预言机测试运行在完全一致的负载标准之上。

**Independent Test**: 运行依赖语料的单测（如 `SilesiaCorpusIntegrityTests` 等），验证所有合成数据均通过 `BenchmarkCorpusType` 生成且 100% 通过。

**Acceptance Scenarios**:
1. **Given** 需要获取文本、源码、结构化或高熵测试语料，**When** 调用 `BenchmarkCorpusGenerator` 或 `BenchmarkCorpusType`，**Then** 在内存中原地生成对应尺寸的标准负载，耗时 $\le 1	ext{ ms}$。
2. **Given** 移除了 `SilesiaFixtureLoader` 与 `EnwikFixtureCacheManager` 的私有合成逻辑，**When** 执行全部语料相关单测，**Then** 0 编译错误，0 数据校验失败。

---

### User Story 2 - 重型宏观基准测试与 Pareto 绘图逻辑物理隔离 (Priority: P1)

作为一名日常迭代业务功能的工程师，我希望执行 `swift test` 时只编译并运行毫秒级的功能单测、架构解耦测试与轻量级微基准（如 50 点纯内存矩阵），而将需要几分钟的重量级多工具宏观 PK 与图表生成逻辑隔离至专用 Target（如 `ttzip-bench`）或环境门禁中，使日常开发反馈循环控制在 2 秒以内。

**Why this priority**: 编译与链接 146 个测试文件消耗了 80% 的本地等待时间，物理拆分可以极大释放日常本地开发与 CI 流程的吞吐量。

**Independent Test**: 在终端执行 `swift test`，测量链接与执行总耗时，验证其在 2.5 秒内完成全量单测通过。

**Acceptance Scenarios**:
1. **Given** 开发者运行 `swift test`，**When** 在默认模式下执行，**Then** 所有测试均在 RAM 中瞬时完成，不产生磁盘图表写入与外部工具阻塞。
2. **Given** 性能工程师需要生成 Pareto 散点图与全矩阵竞品对比报表，**When** 显式触发基准测试模式，**Then** 完整执行多工具 PK 并输出结构化报表。

---

### User Story 3 - 外部工具动态探测与干净 CI 跨平台兼容 (Priority: P2)

作为一名 CI/CD 维护人员，我希望所有与系统外部工具（`unzip`, `bsdtar`, `7zz`, `zstd`）交互的测试均采用动态 `PATH` / `ProcessInfo` 探测机制，而非硬编码 macOS Homebrew 绝对路径，从而在 Linux、macOS 以及精简容器环境中均能平滑运行或优雅跳过。

**Why this priority**: 避免在无 Homebrew 权限或纯 Linux 环境中产生假阳性失败或警告。

**Independent Test**: 在不同机器或未安装某工具的环境中运行差异化测试，验证其自动识别并根据存在性执行或记录跳过。

**Acceptance Scenarios**:
1. **Given** 环境中安装了 `zstd` 或 `7zz`，**When** 执行差分预言机测试，**Then** 自动探测二进制路径并完成回环断言。
2. **Given** 环境中未安装某可选工具，**When** 运行相关测试，**Then** 产生清晰的 `XCTSkip`，不导致整体 CI 红灯。

---

### User Story 4 - 自动化 CI 性能衰减回归门禁集成 (Priority: P2)

作为一名架构师，我希望代码提交前能通过自动化门禁脚本快速调用 50 点内存矩阵，当任何核心算法吞吐衰减超过 $2.0\%$ 且变异系数 $CV > 1.5\%$ 时自动阻断提交，防止隐性性能退化合入主干。

**Why this priority**: 将性能守卫从“事后发现”转变为“前置阻断”，实现无摩擦的性能治理闭环。

**Independent Test**: 运行 `scripts/upstream_audit_gate.py` 或集成脚本，验证对基准矩阵输出的自动化合规断言。

**Acceptance Scenarios**:
1. **Given** 运行基准门禁脚本，**When** 50 点矩阵全部通过且 $CV \le 1.5\%$，**Then** 输出全绿通过摘要，退出码为 0。

---

## Edge Cases

- **未安装外部工具的环境**：如果系统缺少 `7zz` 或 `bsdtar`，必须安全抛出 `XCTSkip` 并附带说明，严禁抛出致命断言失败。
- **低内存或受限容器环境**：语料生成池必须限制在固定大小（如 128KB / 1MB），严禁在无预警情况下分配大于系统可用 RAM 的单块内存。

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: 语料生成基础设施必须以 `Sources/CTTZipBridge/CTTZipCorpusGen.c` 与 `BenchmarkCorpusGenerator.swift` 为唯一权威真实数据源。
- **FR-002**: 必须移除或重构 `SilesiaFixtureLoader.swift`、`EnwikFixtureCacheManager.swift` 中的重复合成逻辑，接入 `BenchmarkCorpusType`。
- **FR-003**: 必须确保 `swift test` 默认执行时间 $\le 2.5$ 秒，单测通过率 100%。
- **FR-004**: 外部二进制调用必须通过 `SystemBinaryResolver`（或类似动态查找器）解析，严禁在测试代码中直接出现硬编码绝对路径（如 `/opt/homebrew/bin/zstd`）。
- **FR-005**: 必须提供自动化的性能回归审计指令，与 50 点内存基准矩阵保持 1:1 格式对接。

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 全量 `swift test` 本地执行时间由原先的 30+ 秒进一步缩短至 $\le 2.5$ 秒。
- **SC-002**: 消除测试代码中 100% 的外部工具绝对路径硬编码。
- **SC-003**: 50 点内存基准测试全量参数矩阵在 $\le 1.1$ 秒内执行完毕，变异系数 $CV \le 1.5\%$。
- **SC-004**: 单元测试、预言机差分测试与架构解耦测试 100% 通过（0 失败，0 意外崩溃）。

---

## Assumptions

- 开发者与 CI 运行环境支持 macOS 13+ 或现代 Linux 发行版。
- 基础系统工具（`unzip`, `tar`）在标准 POSIX 环境下通常默认提供。
