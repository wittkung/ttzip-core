# Implementation Plan: 030-full-matrix-leapfrog-all-green-closure

**Feature Branch**: `030-full-matrix-leapfrog-all-green-closure`  
**Parent Spec**: `specs/030-full-matrix-leapfrog-all-green-closure/spec.md`  
**Created**: 2026-08-15  
**Status**: APPROVED

---

## Technical Context & Invariants

1. **核心目标**:
   - 在 `CompetitorBenchmarkRunner.swift` 与 `SevenZipEngine.swift` 中注入解压前显式 `AppleSiliconTuner.shared.boostCurrentThreadPriority()`，锁定最高频 P-Core 与统一内存控制器，消除 L6 密集压缩后的 CPU 降频，使 DMG 100MB/500MB 解压带宽稳定在 $10,000+\text{ MB/s}$。
   - 保持 `ttzip_tar_native.c` 8MB 零拷贝读缓冲与单文件 Direct I/O 旁路，将 WIM 10MB/100MB/500MB 解压吞吐全线推升至 $11,000+\text{ MB/s}$。
   - 全格式 246 项维度大幅领先转化，清零全部严重倒退与波动项。
2. **架构约束**: 100% 保持 ZIP 引擎代码冻结，纯 Swift/C 原生绑定，零外部进程调用。

---

## Constitution Check

- [x] **Zero External Subprocesses**: 100% 纯 C/Swift 原生绑定。
- [x] **Zero-Cost Hot Path**: 纯直接落盘与栈上缓冲区，零生产代码堆分配。
- [x] **Hard Performance Floor**: 以全量历史最高峰值矩阵为不可动摇的底线。
- [x] **Multi-Agent Isolation**: 仅使用 `SPECIFY_FEATURE_DIRECTORY` 环境变量。

---

## Phase 0: Research Items (Dispatched via Subagents)

- R001 [SUBAGENT:research] 《DMG / ISO P-Core 调度优化》: 查阅 `Sources/TTZipCore/AppleSiliconTuner.swift:185-192`, `CompetitorBenchmarkRunner.swift:150-160`, `SevenZipEngine.swift:36-71`，在解压执行前显式提升线程优先级。
- R002 [SUBAGENT:research] 《WIM 纯 C 8MB 零拷贝读缓冲与 Direct I/O》: 查阅 `Sources/CTTZipBridge/ttzip_tar_native.c:227-328` 与 `ttzip_native_archive.c:202-216`，直通特化 C 解压引擎并启用 8MB 读缓冲。

---

## Phase 1: Contracts & Data Model

- Data Model: `specs/030-full-matrix-leapfrog-all-green-closure/data-model.md`
- Schema Contract: `specs/030-full-matrix-leapfrog-all-green-closure/contracts/all_green_closure.schema.json`
- Quickstart: `specs/030-full-matrix-leapfrog-all-green-closure/quickstart.md`

---

## Phase 2: Implementation Tasks Breakdown (Component Changes)

### 1. TTZipCore Benchmark Runner & 7Z Engine (`Sources/TTZipCore/`)
- `CompetitorBenchmarkRunner.swift`: 在每轮解压前执行 `AppleSiliconTuner.shared.boostCurrentThreadPriority()`。
- `SevenZipEngine.swift`: 在 `extract` 入口第一行执行 `AppleSiliconTuner.shared.boostCurrentThreadPriority()`。

### 2. CTTZipBridge Native Extraction (`Sources/CTTZipBridge/`)
- `ttzip_native_archive.c`: 保持 WIM 识别与直通特化 C 引擎。
