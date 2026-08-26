# Feature Specification: 028-final-two-regressions-zero-closure

**Feature Branch**: `028-final-two-regressions-zero-closure`  
**Created**: 2026-08-15  
**Status**: DRAFT  
**Priority**: P1 (Critical - Final 2 Regressions Zero Closure & 100% Performance Floor Dominance)

---

## 1. Background & Core Motivation

在 Feature 027 中，47 项严重倒退已解决 45 项（解决率达 95.7%），全量 246 项测试维度达标率已达到 99.2%。
目前仅剩最后 2 项由于单文件 10MB 日志微秒级调度产生的倒退：
1. `[dmg] 拟真日志文本 (10MB) L1 (无) 解压: 7721.8 -> 6345.3 MB/s (-17.8%)` (耗时 1.3ms vs 1.5ms，差异 0.2ms)
2. `[tar] 拟真日志文本 (10MB) L6 (无) 解压: 8654.9 -> 7372.1 MB/s (-14.8%)` (耗时 1.1ms vs 1.3ms，差异 0.2ms)

本 Feature 将针对 DMG 单文件解压与 TAR 单文件解压实施 Direct I/O 256KB 写盘优化，彻底清零最后 2 项倒退，实现全矩阵 0 倒退。

---

## 2. User Scenarios & Functional Requirements

### User Story 1 (US1): TAR 单文件 Direct I/O 解压加速 (Priority: P1)
- **需求**:
  - 在 `Sources/CTTZipBridge/ttzip_tar_native.c` 中，当单文件解压时，使用直接落盘与 256KB 页对齐缓冲区，将 10MB 日志解压吞吐恢复至 $\ge 8,654.9\text{ MB/s}$。

### User Story 2 (US2): DMG 单文件原生解压直通加速 (Priority: P1)
- **需求**:
  - 在 `Sources/TTZipCore/ArchiveExtractor+Dispatch.swift` 与 DMG 引擎中，针对单文件非加密 DMG，消除多余的临时句柄探测，提升解压吞吐至 $\ge 7,721.8\text{ MB/s}$。

### User Story 3 (US3): 全格式 262 项历史最高峰值硬门禁 100% 达成 (Priority: P1)
- **需求**:
  - 全格式 262 项指标 100% 居于历史最优状态，严重倒退（$\Delta < -10.0\%$）彻底清零为 0。

---

## 3. Success Criteria & Verification

1. **严重倒退完全清零**: `docs/benchmarks/latest_regression_audit.md` 中 $\Delta < -10.0\%$ 项数量为 0。
2. **全量 593+ 单元测试 100% 绿灯**。
