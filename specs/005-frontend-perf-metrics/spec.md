# Feature Specification: 前端性能指标监控与基准测试体系 (Frontend Performance Metrics & Benchmark System)

**Feature Branch**: `005-frontend-perf-metrics`

**Created**: 2026-08-15

**Status**: Draft

**Input**: User description: "前端性能我觉得也需要有指标"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 前端核心操作量化指标采集与自动化门禁 (Priority: P1) 🎯 MVP

开发者与 CI 流水线能够对前端核心交互（超大目录树异步构建、海量条目防抖搜索匹配、分栏 LRU 缓存存取、高频进度调度）进行毫秒级高精度的性能指标采集，并在单测中设立不可跌破的硬性能门禁。

**Why this priority**: 没有可量化的指标与硬门禁，前端优化极易在后续迭代中发生静默性能倒退。自动化门禁是保障前端流畅度的第一道防线。

**Independent Test**: 运行专用前端性能基准测试套件，验证 10k/50k 目录树构建、20k 搜索过滤、高频进度节流的延迟与吞吐全部输出结构化指标并达标。

**Acceptance Scenarios**:

1. **Given** 包含 50,000 个条目的扁平归档数据，**When** 触发 `ArchiveTreeStore` 树构建性能指标采集，**Then** 输出结构化指标包（构建耗时、节点总数、内存增量），且 50k 条目构建耗时 $\le 80\text{ ms}$。
2. **Given** 包含 20,000 个条目的搜索数据集，**When** 触发多轮不同长度的关键词过滤测试，**Then** 输出搜索单次过滤耗时（$\le 10\text{ ms}$）与过滤吞吐指标（$\ge 2,000,000\text{ 条目/秒}$）。
3. **Given** 10,000 次超高频进度回调，**When** 经由 `ThrottledProgressPublisher` 处理，**Then** 统计节流拦截率（$\ge 95\%$）与主线程派发间隔（稳定在 $16.6\text{ ms} \pm 1\text{ ms}$）。

---

### User Story 2 - GUI 性能测试中心增加前端与渲染指标面板 (Priority: P2)

用户与测试人员在应用内的性能测试控制台（`BenchmarkView`）中，可以直接一键运行前端性能基准，直观查看目录树构建延迟、搜索吞吐、LRU 缓存命中率以及实时帧率评级。

**Why this priority**: 提供直观的可视化体验，让用户和开发者即时感知前端在当前硬件下的极限表现与优化效果。

**Independent Test**: 在 TTZip GUI 打开 "性能测试" 页面，切换至 "前端性能指标" 分区，点击开始测试，观察图表与指标实时刷新。

**Acceptance Scenarios**:

1. **Given** 处于 `BenchmarkView` 页面，**When** 用户点击 "前端性能矩阵"，**Then** 界面展示目录树构建、模糊搜索、LRU 缓存命中、UI 刷新节流等 4 大维度的量化卡片。
2. **Given** 正在运行前端基准测试，**When** 测试逐项执行，**Then** 速度仪表盘实时转动并输出当前硬件评级（如 "Apple Silicon 极致流畅"）。
3. **Given** 测试执行完毕，**When** 用户导出报告，**Then** 生成包含详细百分位延迟（P50/P90/P99）的结构化 JSON/Markdown 结果。

---

### User Story 3 - 运行时 UI 掉帧与卡顿实时监测器 (Priority: P3)

在应用正常运行与大归档浏览过程中，轻量级帧率与卡顿监测器（`UIFrameMetricsSampler`）在 Debug/诊断模式下以零开销采样主线程卡顿（Jank），自动记录单帧渲染超阈值事件（如 $> 16.6\text{ ms}$）。

**Why this priority**: 捕获真实用户交互场景中的偶发掉帧，精准定位瓶颈组件。

**Independent Test**: 模拟极速滑动大列表，验证采样器精准捕获掉帧次数与平均帧率。

**Acceptance Scenarios**:

1. **Given** 开启诊断采样模式，**When** 在 10,000 行列表快速滚动，**Then** 采样器以 $< 0.1\%$ 的极低 CPU 负载记录帧时间分布与掉帧计数。
2. **Given** 检测到持续卡顿（单帧 $> 50\text{ ms}$），**When** 触发诊断日志，**Then** 通过 `TTLogger.warning` 输出结构化事件而不阻塞当前帧。

---

### Edge Cases

- **极小与空数据集**: 0 节点或 1 个节点的目录树在采集指标时正常处理，耗时统计不出现除零异常（`NaN` / `Infinite`）。
- **极端海量节点压测**: 面对 200,000 节点的超限压力测试，指标收集器安全设定超时保护（5秒上限），防止主线程挂起。
- **后台省电模式 / 窗口最小化**: 当应用处于后台或显示器休眠时，帧率采样器自动暂停时钟挂起，不产生无谓的唤醒功耗。

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001 (前端性能指标模型定义)**: 定义统一的前端性能指标数据结构（`FrontendPerformanceReport`、`TreeBuildMetric`、`SearchFilterMetric`、`LRUCacheMetric`、`ProgressThrottleMetric`），支持多指标聚合、均值计算与 JSON 序列化。
- **FR-002 (前端基准测试执行引擎)**: 构建轻量级、无外部依赖的 `FrontendBenchmarkRunner`，支持生成 1k ~ 100k 规模的标准测试数据集并执行目录树构建、搜索过滤、LRU 淘汰的压测。
- **FR-003 (前端性能硬门禁测试套件)**: 建立 `FrontendPerformanceGateTests`，将 50k 树构建耗时 $\le 100\text{ ms}$、20k 搜索耗时 $\le 15\text{ ms}$、LRU 10,000 次操作耗时 $\le 5\text{ ms}$ 设定为自动化门禁。
- **FR-004 (GUI Benchmark 前端性能控制台集成)**: 在 `BenchmarkViewModel` 和 `BenchmarkView` 中扩展前端性能测试模式，提供可视化卡片与测试执行能力。
- **FR-005 (零侵入性能采样)**: 采样器与收集逻辑在非测试生产模式下保持零中间堆分配与零主线程阻塞。

### Key Entities

- **FrontendPerformanceReport**: 前端性能测试汇总报告实体，包含硬件环境、测试时间戳与各项指标明细。
- **TreeBuildMetric**: 目录树构建指标（包含条目数、总层数、构建耗时 ms、每秒处理条目数 items/s、内存消耗）。
- **SearchFilterMetric**: 搜索过滤指标（包含数据集规模、搜索词长度、匹配命中数、过滤耗时 ms、过滤吞吐 items/s）。
- **ProgressThrottleMetric**: 进度更新节流指标（总事件数、放行数、节流拦截率、平均派发间隔 ms）。
- **FrontendBenchmarkRunner**: 前端性能基准执行中枢。

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001 (自动化门禁覆盖率)**: 前端核心 4 大操作（树构建、搜索过滤、LRU 缓存、进度节流）100% 纳入自动化单测门禁。
- **SC-002 (树构建性能底线)**: 50,000 条目目录树构建指标测试通过且耗时 $\le 80\text{ ms}$。
- **SC-003 (搜索吞吐性能底线)**: 20,000 条目实时搜索指标测试通过且吞吐 $\ge 2,000,000\text{ 条目/秒}$。
- **SC-004 (进度节流精度)**: 10,000 次高频事件下，节流拦截率 $\ge 95\%$，UI 刷新派发频率严格收敛在 $60\text{Hz} \pm 5\%$。
- **SC-005 (测试套件回归)**: 全量测试 100% 通过，不影响原有 C 引擎与核心归档门禁。

## Assumptions

- 前端基准测试在 macOS 14.0+ 原生环境下运行，测试数据集在内存中模拟构造，不产生磁盘临时文件 I/O 干扰。
- GUI 基准测试视图与 CLI 基准复用底层指标实体与测试执行引擎。
