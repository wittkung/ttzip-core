# Implementation Plan: 026-zero-regression-final-mile

**Feature Branch**: `026-zero-regression-final-mile`  
**Parent Spec**: `specs/026-zero-regression-final-mile/spec.md`  
**Created**: 2026-08-15  
**Status**: APPROVED

---

## Technical Context & Invariants

1. **核心目标**:
   - 彻底清零最后 6 项倒退，实现全量 262 项门禁 100% 达标。
   - 在 `ttzip_tar_native.c` 中为单文件/根目录条目实施 Fast-Path 旁路。
   - 在 `CTTZipBridge_7zNativeDecoder.c` 与 `ttzip_7z_crypto_neon.c` 中实施 256KB Cache 对齐与 ARMv8 8-Way 硬件向量解密。
2. **架构约束**: 100% 保持 ZIP 引擎代码冻结，热路径零堆分配抽象。

---

## Constitution Check

- [x] **Zero External Subprocesses**: 100% 纯 C/Swift 原生绑定。
- [x] **Zero-Cost Hot Path**: 单文件旁路 `strchr` 仅耗时 < 2ns，零堆分配。
- [x] **Hard Performance Floor**: 以历史绝对最高峰值矩阵为绝对底线。
- [x] **Multi-Agent Isolation**: 仅使用 `SPECIFY_FEATURE_DIRECTORY` 环境变量。

---

## Phase 0: Research Items (Dispatched via Subagents)

- R001 [SUBAGENT:research] 《7Z 100MB 高熵数据块 256KB Cache 对齐与 NEON Direct 解密》: 查阅 `Sources/CTTZipBridge/CTTZipBridge_7zNativeDecoder.c:48-228`，实施 256KB 物理页切片与 ARMv8 8-Way SIMD 向量解密。
- R002 [SUBAGENT:research] 《TAR / TAR.ZST 单文件与根目录条目快速路径旁路》: 查阅 `Sources/CTTZipBridge/ttzip_tar_native.c:234, 261-314`，对 `strchr(entry_pathname, '/') == NULL` 实施零开销旁路。

---

## Phase 1: Contracts & Data Model

- Data Model: `specs/026-zero-regression-final-mile/data-model.md`
- Schema Contract: `specs/026-zero-regression-final-mile/contracts/final_mile_audit.schema.json`
- Quickstart: `specs/026-zero-regression-final-mile/quickstart.md`

---

## Phase 2: Implementation Tasks Breakdown (Component Changes)

### 1. CTTZipBridge (`Sources/CTTZipBridge/`)
- `ttzip_tar_native.c`: 为单文件/根目录条目实施 Fast-Path 旁路，跳过 `snprintf`、`strrchr`、FNV-1a 哈希与 `mkdir_cache`。

### 2. Rules & Hard Gate Matrix (`GEMINI.md`, `scripts/`)
- `GEMINI.md`: 保持 §3.1 全格式最高峰值硬门禁矩阵最新状态。
- `scripts/audit_performance_regression.py`: 严格比对历史最高峰值，阻断任何 $>10\%$ 倒退。
