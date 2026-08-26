# Implementation Plan: 024-full-matrix-peak-consolidation-and-zero-regression

**Feature Branch**: `024-full-matrix-peak-consolidation-and-zero-regression`  
**Parent Spec**: `specs/024-full-matrix-peak-consolidation-and-zero-regression/spec.md`  
**Created**: 2026-08-15  
**Status**: APPROVED

---

## Technical Context & Invariants

1. **核心目标**:
   - 固化全 16 种格式 262 项维度的历史绝对最高峰值门禁（包含 ZIP 12.3 GB/s、TAR.ZST 23.9 GB/s、WIM 11.8 GB/s 等所有已突破纪录）。
   - 修复 DMG AES 加密分发路由与 TAR 栈上双层内联目录缓存，彻底清零最后 11 项倒退。
2. **架构约束**: 100% In-Process C 静态库绑定，零外部 CLI 进程调用，热路径零堆分配抽象。

---

## Constitution Check

- [x] **Zero External Subprocesses**: 100% 纯 C/Swift 原生绑定。
- [x] **Zero-Cost Hot Path**: TAR 目录缓存使用栈上局部数组，零 `malloc`/`free`。
- [x] **Hard Performance Floor**: 覆盖全部 16 种格式 262 项测试维度，以历史最高峰值为绝对基准。
- [x] **Multi-Agent Isolation**: 仅使用 `SPECIFY_FEATURE_DIRECTORY` 环境变量。

---

## Phase 0: Research Items (Dispatched via Subagents)

- R001 [SUBAGENT:research] 《DMG 加密分发与硬件 AES-256 解压直通》: 查阅 `Sources/TTZipCore/ArchiveExtractor+Dispatch.swift:16-30`，仅在 `password == nil` 时走 C 直通，有密码时路由至 `SevenZipEngine`（ARM64 NEON AES-256），消除 4 项 DMG AES 倒退。
- R002 [SUBAGENT:research] 《TAR 原生解压栈上双层零分配内联目录缓存》: 查阅 `Sources/CTTZipBridge/ttzip_tar_native.c:263-296`，移植 `last_parent_dir` L1 + L2 64-Slot hash，将系统调用压降 $>99.5\%$。

---

## Phase 1: Contracts & Data Model

- Data Model: `specs/024-full-matrix-peak-consolidation-and-zero-regression/data-model.md`
- Schema Contract: `specs/024-full-matrix-peak-consolidation-and-zero-regression/contracts/peak_matrix_consolidation.schema.json`
- Quickstart: `specs/024-full-matrix-peak-consolidation-and-zero-regression/quickstart.md`

---

## Phase 2: Implementation Tasks Breakdown (Component Changes)

### 1. TTZipCore (`Sources/TTZipCore/`)
- `ArchiveExtractor+Dispatch.swift`: 修复 DMG 分发条件，仅当 `password == nil || password!.isEmpty` 时直通 C 引擎，有密码时走 `SevenZipEngine`。

### 2. CTTZipBridge (`Sources/CTTZipBridge/`)
- `ttzip_tar_native.c`: 在 `ttzip_extract_tar_native_c` 中引入栈上双层内联目录缓存池 (`last_parent_dir` + L2 64-Slot hash)。

### 3. Rules & Gates (`GEMINI.md`, `scripts/`)
- `GEMINI.md`: 保持 §3.1 全格式最高峰值硬门禁矩阵最新状态。
- `scripts/audit_performance_regression.py`: 严格比对历史最高峰值，阻断任何 $>10\%$ 倒退。
