# Implementation Plan: 023-last-mile-zero-regression-and-adaptive-peak-gates

**Feature Branch**: `023-last-mile-zero-regression-and-adaptive-peak-gates`  
**Parent Spec**: `specs/023-last-mile-zero-regression-and-adaptive-peak-gates/spec.md`  
**Created**: 2026-08-15  
**Status**: APPROVED

---

## Technical Context & Invariants

1. **核心目标**: 彻底清零全格式 262 项细分维度的最后 4 项倒退（WIM 500MB 大文件解压、DMG 10MB 拟真日志解压、DMG 100MB 高熵解压、7Z 100 小文件解压），并将门禁矩阵固化在历史最高峰值。
2. **架构约束**: 100% In-Process C 静态库绑定，零外部 CLI 进程调用，热路径零堆分配抽象。
3. **门禁铁律**: 严禁以任何理由下调 `docs/benchmarks/peak_performance_matrix.json` 与 `GEMINI.md` 中的门禁指标。

---

## Constitution Check

- [x] **Zero External Subprocesses**: 100% 纯 C/Swift 原生绑定。
- [x] **Zero-Cost Hot Path**: 7Z 小文件目录缓存使用栈上局部数组，零 `malloc`/`free`。
- [x] **Hard Performance Floor**: 覆盖全部 16 种格式 262 项测试维度，以历史最高峰值为绝对基准。
- [x] **Multi-Agent Isolation**: 仅使用 `SPECIFY_FEATURE_DIRECTORY` 环境变量。

---

## Phase 0: Research Items (Dispatched via Subagents)

- R001 [SUBAGENT:research] 《WIM 500MB 大文件解压与 APFS 脏页排队消除方案》: 查阅 `Sources/CTTZipBridge/ttzip_native_archive.c:65-76, 202-216`，通过 `.wim` 探测直通与 `fcntl(F_RDAHEAD)` / `posix_madvise(MADV_WILLNEED)` 预热，配合 16KB 页对齐写入达成 11,000+ MB/s。
- R002 [SUBAGENT:research] 《DMG 镜像挂载与解压调度抖动消除》: 查阅 `Sources/TTZipCore/ArchiveExtractor+Dispatch.swift:16-21`，消除 7Z 引擎盲探 Header 失败回退开销，使 10MB 日志达 6,562+ MB/s，100MB 高熵达 9,556+ MB/s。
- R003 [SUBAGENT:research] 《7Z 海量小文件栈上双层零分配内联目录缓存池》: 查阅 `Sources/CTTZipBridge/CTTZipBridge_7zNativeDecoder.c:150-209`，实现 `last_parent_dir` L1 热缓存 + L2 64-Slot 哈希槽，将系统调用压降 $>98\%$，恢复至 1,450+ MB/s。

---

## Phase 1: Contracts & Data Model

- Data Model: `specs/023-last-mile-zero-regression-and-adaptive-peak-gates/data-model.md`
- Schema Contract: `specs/023-last-mile-zero-regression-and-adaptive-peak-gates/contracts/last_mile_audit.schema.json`
- Quickstart: `specs/023-last-mile-zero-regression-and-adaptive-peak-gates/quickstart.md`

---

## Phase 2: Implementation Tasks Breakdown (Component Changes)

### 1. CTTZipBridge (`Sources/CTTZipBridge/`)
- `CTTZipBridge_7zNativeDecoder.c`: 实现栈上双层内联目录缓存池 (`last_parent_dir` + L2 64-Slot hash)，短路同目录重复 `ttzip_common_mkdir_p`。
- `ttzip_native_archive.c`: 完善 `.wim` 探测与 `F_RDAHEAD` 预热，消除冷页缺页中断。
- `ttzip_tar_native.c`: 确保大文件解压写盘对齐 16KB 边界。

### 2. TTZipCore (`Sources/TTZipCore/`)
- `ArchiveExtractor+Dispatch.swift`: 确保 DMG 直通分发隔离 7z 试探路径。
- `CompetitorBenchmarkRunner.swift`: 确保每轮 Pass 结束时及时 close 释放写回句柄，避免 APFS 脏页堆积。

### 3. Scripts & Rules (`scripts/`, `GEMINI.md`)
- `scripts/audit_performance_regression.py`: 严格比对历史最高峰值，阻断任何 $>10\%$ 倒退。
- `GEMINI.md`: 保持 §3.1 全矩阵门禁最新状态。
