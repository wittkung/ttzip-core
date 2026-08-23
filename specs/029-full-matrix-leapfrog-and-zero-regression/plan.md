# Implementation Plan: 029-full-matrix-leapfrog-and-zero-regression

**Feature Branch**: `029-full-matrix-leapfrog-and-zero-regression`  
**Parent Spec**: `specs/029-full-matrix-leapfrog-and-zero-regression/spec.md`  
**Created**: 2026-08-15  
**Status**: APPROVED

---

## Technical Context & Invariants

1. **核心目标**:
   - 在 `CompetitorBenchmarkRunner.swift` 中实行场景级延迟集中清理：各 Pass 分配完全独立的 `\(scenarioPrefix)_arc_p\(p)` 与 `\(scenarioPrefix)_out_p\(p)`，运行期间严禁 `removeItem`，所有删除延迟至场景结束后一次性释放，消除 APFS 异步空间回收锁争用。
   - 在 `ttzip_native_archive.c` 与 `ArchiveExtractor+Dispatch.swift` 中实施 WIM 纯 C 原生极速直通，消除 0.2ms 调度延迟，推升 WIM 解压至 $\ge 10,000\text{ MB/s}$。
   - 实现全量 262 项历史最高峰值硬门禁 100% 达成与大幅超越。
2. **架构约束**: 100% 保持 ZIP 引擎代码冻结，纯 Swift/C 内部优化，零外部依赖。

---

## Constitution Check

- [x] **Zero External Subprocesses**: 100% 纯 C/Swift 原生绑定。
- [x] **Zero-Cost Hot Path**: 纯直接落盘与栈上缓冲区，零生产代码堆分配。
- [x] **Hard Performance Floor**: 以全量历史最高峰值矩阵为不可动摇的底线。
- [x] **Multi-Agent Isolation**: 仅使用 `SPECIFY_FEATURE_DIRECTORY` 环境变量。

---

## Phase 0: Research Items (Dispatched via Subagents)

- R001 [SUBAGENT:research] 《APFS 场景级延迟集中清理》: 查阅 `Sources/TTZipCore/Benchmark/CompetitorBenchmarkRunner.swift:115-202, 264-272`，消除 Pass 间 `removeItem`，统一延迟集中释放。
- R002 [SUBAGENT:research] 《WIM 纯 C 原生极速直通与调度延迟消除》: 查阅 `Sources/CTTZipBridge/ttzip_native_archive.c:34-76` 与 `BaseArchiveEngineTemplate.swift:111-130`，直通 C 原生引擎并按需懒分配临时目录。

---

## Phase 1: Contracts & Data Model

- Data Model: `specs/029-full-matrix-leapfrog-and-zero-regression/data-model.md`
- Schema Contract: `specs/029-full-matrix-leapfrog-and-zero-regression/contracts/leapfrog_audit.schema.json`
- Quickstart: `specs/029-full-matrix-leapfrog-and-zero-regression/quickstart.md`

---

## Phase 2: Implementation Tasks Breakdown (Component Changes)

### 1. TTZipCore Benchmark Runner (`Sources/TTZipCore/Benchmark/`)
- `CompetitorBenchmarkRunner.swift`: 实施场景级延迟集中清理，消除 Pass 间 `removeItem`，为各轮 Pass 分配完全独立路径。

### 2. CTTZipBridge Native Extraction (`Sources/CTTZipBridge/`)
- `ttzip_native_archive.c`: 增加 WIM Magic Header 识别与直接特化流式解压。
