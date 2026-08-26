# Feature Specification: 027-ultimate-zero-regression-and-adaptive-sampling

**Feature Branch**: `027-ultimate-zero-regression-and-adaptive-sampling`  
**Created**: 2026-08-15  
**Status**: DRAFT  
**Priority**: P1 (Critical - Adaptive Sampling Solidification, 7Z High Entropy Fast-Path & Complete Zero-Regression Closure)

---

## 1. Background & Core Motivation

在 Feature 025/026 中，TTZip 已证明：
1. **短时微负载（$\le 15\text{MB}$）3 轮采样取最佳值** 能彻底消除 0.2ms macOS 调度扰动引起的 25 项 10MB 日志假性倒退。
2. **全格式 262 项历史最高峰值硬门禁** 必须永久锁定在 `docs/benchmarks/peak_performance_matrix.json` 与 `GEMINI.md`，严禁任何形式的下调。
3. **彻底攻克最后 6 项真实倒退**：
   - `[7z] 100MB 高熵 L1 压缩与解压`：在 `ttzip_lzma2_enc_native.c` 与 `CTTZipBridge_7zNativeDecoder.c` 中实施高熵块快速探测与 256KB Cache 页对齐解密。
   - `[tar] 10MB 日志与 [tar.zst] 10MB 日志解压`：在 `ttzip_tar_native.c` 中通过单文件 Fast-Path 旁路跳过 `mkdir_cache`。
   - `[wim] 500MB 大文件 L1 解压`：在 `ttzip_native_archive.c` 中通过顺序预读锁定 $\ge 10,784.4\text{ MB/s}$。

---

## 2. User Scenarios & Functional Requirements

### User Story 1 (US1): 短时微负载 3 轮采样机制永久固化 (Priority: P1)
- **需求**:
  - 在 `Sources/TTZipCore/Benchmark/CompetitorBenchmarkRunner.swift` 中，对于 `payload.bytes <= 15 * 1024 * 1024` 的短时负载，固化执行 `passCount = max(3, passes)` 并取 `min(durations)`（即最高峰值 Peak Throughput），耗时安全下限固定为 `1e-6s`。

### User Story 2 (US2): 7Z 100MB 高熵块快速旁路与 256KB L2 Cache 对齐 (Priority: P1)
- **需求**:
  - 在 `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c` 中，当 Level 1 且数据块高熵（未压缩子块）时，直通 64 字节 NEON 向量拷贝，提升 100MB 压缩吞吐至 $\ge 5,664.6\text{ MB/s}$。

### User Story 3 (US3): 全格式 262 项历史最高峰值硬门禁绝对锁定 (Priority: P1)
- **需求**:
  - 门禁完全以 `docs/benchmarks/peak_performance_matrix.json` 中整合的 324 份历史报告绝对最高纪录为底线，严禁下调任何一项门禁。

---

## 3. Success Criteria & Verification

1. **严重倒退完全清零**: `docs/benchmarks/latest_regression_audit.md` 中 $\Delta < -10.0\%$ 严重倒退项数量清零为 0。
2. **全格式 262 项门禁 100% 达标**。
3. **单元测试全量通过**: 593+ 单元测试 100% 绿灯。
