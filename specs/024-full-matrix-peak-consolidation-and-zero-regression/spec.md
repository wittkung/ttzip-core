# Feature Specification: 024-full-matrix-peak-consolidation-and-zero-regression

**Feature Branch**: `024-full-matrix-peak-consolidation-and-zero-regression`  
**Created**: 2026-08-15  
**Status**: DRAFT  
**Priority**: P1 (Critical - Peak Floor Matrix Consolidation & Zero-Regression Closure)

---

## 1. Background & Core Motivation

在 Feature 022 与 Feature 023 中，TTZip 大幅刷新了 ZIP (12.3 GB/s)、TAR.ZST (23.9 GB/s)、WIM (11.8 GB/s) 等多个核心格式的历史最高纪录。
用户核心指令：
1. **核验并固化所有 262 项门禁至历史最优状态（将前期所有优化突破的峰值全部纳入门禁）**：严禁以任何方式降低门禁，持续将最新突破的峰值动态纳入硬门禁。
2. **彻底解决最后 11 项剩余倒退**：
   - `DMG (AES-256 加密)` 场景：修复 `ArchiveExtractor+Dispatch.swift` 中的加密路由，仅在无密码时直通 C 引擎，有密码时路由至支持硬件 AES 解密的 7Z/原生解密引擎，消除 4 项 DMG AES 倒退。
   - `TAR / TAR.XZ / LZIP` 小文件与日志场景：将 7Z 的双层内联目录缓存池 (`last_parent_dir` + L2 64-Slot hash) 移植至 `ttzip_tar_native.c`，消除 600 次 `mkdir` 系统调用。
   - `WIM / LZIP` 10MB/100MB 缓冲区对齐调优。

---

## 2. User Scenarios & Functional Requirements

### User Story 1 (US1): DMG 加密解压分发路由修复与硬件 AES 直通 (Priority: P1)
- **场景**: 用户解压带有 AES-256 密码保护的 DMG 归档。
- **需求**:
  - 在 `ArchiveExtractor+Dispatch.swift` 中细化分发条件：
    - `if (targetFormat == .dmg || targetFormat == .iso) && (password == nil || password!.isEmpty)` ➔ 走 C 原生直通。
    - 若 `password != nil && !password!.isEmpty` ➔ 走支持硬件 AES 解密的 `SevenZipEngine`。
  - 恢复 DMG 500MB L6 AES 解压吞吐至 $\ge 9,933.1\text{ MB/s}$（消除 -34.8% 倒退），拟真日志 AES 恢复至 $\ge 5,396.2\text{ MB/s}$，小文件 AES 恢复至 $\ge 1,084.3\text{ MB/s}$。

### User Story 2 (US2): TAR 变体全系列双层零分配内联目录缓存移植 (Priority: P1)
- **场景**: 用户解压包含 100 个以上小文件的 TAR / TAR.GZ / TAR.BZ2 / TAR.XZ 归档。
- **需求**:
  - 在 `Sources/CTTZipBridge/ttzip_tar_native.c` 的 `ttzip_extract_tar_native_c` 中实现与 7Z 一致的栈上双层内联目录缓存池。
  - 消除 600 次逐级 `mkdir(tmp, 0755)` 冗余系统调用，提升小文件解压吞吐至 $\ge 1,304.1\text{ MB/s}$。

### User Story 3 (US3): LZIP / WIM 短时日志与高熵 Payload 块处理调优 (Priority: P1)
- **场景**: 10MB 拟真日志与 100MB 高熵数据块压测。
- **需求**:
  - 调优 LZIP / WIM 的块读取流式策略，消除短时 1.3ms 采样抖动。

### User Story 4 (US4): 全格式动态最高峰值门禁自动整合与永久锁定 (Priority: P1)
- **场景**: 自动化回归与 CI 阻断。
- **需求**:
  - 自动汇总历史所有批次测试中的单项最高吞吐，固化至 `docs/benchmarks/peak_performance_matrix.json` 与 `GEMINI.md`。
  - `scripts/audit_performance_regression.py` 严格以该矩阵为门禁底线。

---

## 3. Success Criteria & Verification

1. **严重倒退清零**: `docs/benchmarks/latest_regression_audit.md` 报告中 $\Delta < -10.0\%$ 严重倒退项全部清零（0 项）。
2. **全格式门禁 100% 达标**: 所有 262 项细分维度全部达到历史最高峰值门禁。
3. **单元测试全量通过**: 593+ 单元测试 100% 绿灯。
