# Implementation Plan: 028-final-two-regressions-zero-closure

**Feature Branch**: `028-final-two-regressions-zero-closure`  
**Parent Spec**: `specs/028-final-two-regressions-zero-closure/spec.md`  
**Created**: 2026-08-15  
**Status**: APPROVED

---

## Technical Context & Invariants

1. **核心目标**:
   - 在 `Sources/CTTZipBridge/ttzip_tar_native.c` 中为单文件/根目录普通文件（`AE_IFREG`）实施直接 POSIX `open` + `write` 写盘旁路，绕过 `archive_write_disk` 磁盘抽象层，将 TAR 单文件日志解压推升至 $\ge 8,654.9\text{ MB/s}$。
   - 在 `Sources/TTZipCore/ArchiveExtractor+Dispatch.swift` 中优化 DMG / ISO 无密码解压原生 C 路由直通，在 `BaseArchiveEngineTemplate.swift` 中实现临时目录懒分配，消除 0.15ms 调度延迟并将 DMG 解压推升至 $\ge 7,721.8\text{ MB/s}$。
   - 彻底清零最后 2 项严重倒退，实现全量 262 项历史最高峰值硬门禁 100% 达成。
2. **架构约束**: 100% 保持 ZIP 引擎代码冻结，纯 Swift/C 内部优化，零外部依赖。

---

## Constitution Check

- [x] **Zero External Subprocesses**: 100% 纯 C/Swift 原生绑定。
- [x] **Zero-Cost Hot Path**: 纯直接落盘与栈上缓冲区，零生产代码堆分配。
- [x] **Hard Performance Floor**: 以全量历史最高峰值矩阵为不可动摇的底线。
- [x] **Multi-Agent Isolation**: 仅使用 `SPECIFY_FEATURE_DIRECTORY` 环境变量。

---

## Phase 0: Research Items (Dispatched via Subagents)

- R001 [SUBAGENT:research] 《TAR 单文件直接 POSIX 写盘旁路》: 查阅 `Sources/CTTZipBridge/ttzip_tar_native.c:227-328`，直接 `open` + `write` 绕过 `archive_write_disk`。
- R002 [SUBAGENT:research] 《DMG / ISO 无密码原生 C 路由直通与临时目录懒分配》: 查阅 `Sources/TTZipCore/ArchiveExtractor+Dispatch.swift:16-22` 与 `BaseArchiveEngineTemplate.swift:111-130`，直通 C 原生引擎并按需懒分配临时目录。

---

## Phase 1: Contracts & Data Model

- Data Model: `specs/028-final-two-regressions-zero-closure/data-model.md`
- Schema Contract: `specs/028-final-two-regressions-zero-closure/contracts/final_closure_audit.schema.json`
- Quickstart: `specs/028-final-two-regressions-zero-closure/quickstart.md`

---

## Phase 2: Implementation Tasks Breakdown (Component Changes)

### 1. CTTZipBridge TAR Native Extraction (`Sources/CTTZipBridge/`)
- `ttzip_tar_native.c`: 为根目录普通文件实施直接 POSIX `open` + `write` 旁路。

### 2. TTZipCore Extraction Routing & Template (`Sources/TTZipCore/`)
- `ArchiveExtractor+Dispatch.swift`: 确保无密码 DMG / ISO 直通原生 C 引擎。
- `BaseArchiveEngineTemplate.swift`: 实施临时目录按需懒分配。
