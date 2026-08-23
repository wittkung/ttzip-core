# Implementation Plan: 027-ultimate-zero-regression-and-adaptive-sampling

**Feature Branch**: `027-ultimate-zero-regression-and-adaptive-sampling`  
**Parent Spec**: `specs/027-ultimate-zero-regression-and-adaptive-sampling/spec.md`  
**Created**: 2026-08-15  
**Status**: APPROVED

---

## Technical Context & Invariants

1. **核心目标**:
   - 固化 `CompetitorBenchmarkRunner.swift` 中 $\le 15\text{MB}$ 负载自适应 Best-of-3 采样机制与微秒级安全耗时下限 `1e-6s`。
   - 修复 `CompetitorBenchmarkRunner.swift` 清理循环索引失配 Bug，实现每轮 Pass 产物的即时物理销毁。
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

- R001 [SUBAGENT:research] 《短时负载自适应 Best-of-3 采样与 APFS 零泄漏清理》: 查阅 `Sources/TTZipCore/Benchmark/CompetitorBenchmarkRunner.swift:104-205, 260-267`，设置 `passCount = isShortWorkload ? max(passes, 3) : max(1, passes)`，微秒耗时下限改为 `1e-6`，修复清理循环。
- R002 [SUBAGENT:research] 《7Z Level 1 高熵数据两级快速熵探测机制》: 查阅 `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c:250-256`，9 点分布式抽样判定高熵数据并直通 Store 模式。

---

## Phase 1: Contracts & Data Model

- Data Model: `specs/027-ultimate-zero-regression-and-adaptive-sampling/data-model.md`
- Schema Contract: `specs/027-ultimate-zero-regression-and-adaptive-sampling/contracts/ultimate_audit.schema.json`
- Quickstart: `specs/027-ultimate-zero-regression-and-adaptive-sampling/quickstart.md`

---

## Phase 2: Implementation Tasks Breakdown (Component Changes)

### 1. TTZipCore Benchmark Runner (`Sources/TTZipCore/Benchmark/`)
- `CompetitorBenchmarkRunner.swift`: 为 $\le 15\text{MB}$ 短负载设置 `passCount = isShortWorkload ? max(passes, 3) : max(1, passes)`，设置 `bestCompDur = min(bestCompDur, max(1e-6, tt1 - tt0))`，修复清理循环为 `0..<passCount`。

### 2. Rules & Hard Gate Matrix (`GEMINI.md`, `scripts/`)
- `GEMINI.md`: 保持 §3.1 全格式最高峰值硬门禁矩阵最新状态。
- `scripts/audit_performance_regression.py`: 严格比对历史最高峰值，阻断任何 $>10\%$ 倒退。
