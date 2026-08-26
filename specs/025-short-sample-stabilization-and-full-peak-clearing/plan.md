# Implementation Plan: 025-short-sample-stabilization-and-full-peak-clearing

**Feature Branch**: `025-short-sample-stabilization-and-full-peak-clearing`  
**Parent Spec**: `specs/025-short-sample-stabilization-and-full-peak-clearing/spec.md`  
**Created**: 2026-08-15  
**Status**: APPROVED

---

## Technical Context & Invariants

1. **核心目标**:
   - 在 `CompetitorBenchmarkRunner.swift` 中引入自适应微基准多轮迭代采样（短时负载 $\le 10\text{MB}$ 采用 1 轮预热 + 3 轮采样取最佳值），消除 0.2ms 系统中断引起的虚假倒退。
   - 解除 `max(0.001, ...)` 的 1ms 下限硬截断，调整为 `1e-6`（1 微秒）。
   - 固化全 16 种格式 262 项维度的历史绝对最高纪录硬门禁。
2. **架构约束**: 100% 保持 ZIP 引擎代码冻结，纯 Swift/C 内部优化，零外部依赖。

---

## Constitution Check

- [x] **Zero External Subprocesses**: 100% 纯 C/Swift 原生绑定。
- [x] **Zero-Cost Hot Path**: 纯栈上度量与自适应控制，零生产代码堆分配。
- [x] **Hard Performance Floor**: 以全量历史最高峰值矩阵为不可动摇的底线。
- [x] **Multi-Agent Isolation**: 仅使用 `SPECIFY_FEATURE_DIRECTORY` 环境变量。

---

## Phase 0: Research Items (Dispatched via Subagents)

- R001 [SUBAGENT:research] 《短时负载自适应多轮采样与耗时下限修正》: 查阅 `Sources/TTZipCore/Benchmark/CompetitorBenchmarkRunner.swift:104-177`，引入 1 轮预热 + 3 轮采样取 `min(durations)`，耗时下限改为 `1e-6`。
- R002 [SUBAGENT:research] 《WinZip AES-256 派生密钥与上下文最优调用路径》: 查阅 `Sources/CTTZipBridge/CTTZipBridge_Crypto.c:44-459` 与 `Sources/TTZipCore/Zip/ZipCryptoEngine.swift:74-228`，保持 ZIP 引擎冻结，复用栈分配零拷贝直解与 C 层线程局部 Key 缓存。

---

## Phase 1: Contracts & Data Model

- Data Model: `specs/025-short-sample-stabilization-and-full-peak-clearing/data-model.md`
- Schema Contract: `specs/025-short-sample-stabilization-and-full-peak-clearing/contracts/benchmark_sampling.schema.json`
- Quickstart: `specs/025-short-sample-stabilization-and-full-peak-clearing/quickstart.md`

---

## Phase 2: Implementation Tasks Breakdown (Component Changes)

### 1. TTZipCore Benchmark Runner (`Sources/TTZipCore/Benchmark/`)
- `CompetitorBenchmarkRunner.swift`: 为 $\le 10\text{MB}$ 短时负载实施 1 轮预热 + 3 轮采样取 `min(durations)`，并将 `max(0.001, ...)` 修正为 `max(1e-6, ...)`。
- `CompetitorBenchmarkRunner+Executors.swift`: 同步对齐竞品多轮测试度量逻辑。

### 2. Rules & Hard Gate Matrix (`GEMINI.md`, `scripts/`)
- `GEMINI.md`: 保持 §3.1 全格式最高峰值硬门禁矩阵最新状态。
- `scripts/audit_performance_regression.py`: 严格比对历史最高峰值，阻断任何 $>10\%$ 倒退。
