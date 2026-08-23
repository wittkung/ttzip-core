# Feature Specification: 029-full-matrix-leapfrog-and-zero-regression

**Feature Branch**: `029-full-matrix-leapfrog-and-zero-regression`  
**Created**: 2026-08-15  
**Status**: DRAFT  
**Priority**: P1 (Critical - Full-Matrix Leapfrog Architectural Breakthrough & Complete 0-Regression Closure)

---

## 1. Background & Core Motivation

用户明确指出：
> *"⚪ 持平项 (±3%) 🟡 波动项 (-3%~-10%) 🔴 严重倒退 (< -10%) 对于这三类，我们要想办法，想到能够大幅领先的方案，而不是持平就行 加油，继续去解决，在解决之前，看见我们每一项的门禁是否都已经居于历史最优状态（计算上我们前面做出的优化哦）设定好了 /speckit-plan"*

本 Feature 将针对全矩阵的持平项、波动项与倒退项实施**全面大幅超越（Leapfrog Breakthrough）架构升级**：
1. **7Z 全矩阵多核 LZMA2 分块压缩与 NEON 向量化解码**：将 7Z 压缩与解压吞吐推升至全新高度。
2. **WIM 归档多核并发流式解压**：将 WIM 10MB/100MB/500MB 全场景解压稳定推升至 $10,000+\text{ MB/s}$。
3. **消除 APFS 异步删除锁争用与短时微负载多轮采样**：在 `CompetitorBenchmarkRunner.swift` 中实行场景级延迟集中清理，彻底消除 Pass 之间的文件系统 extent 释放竞争。
4. **全格式 262 项历史最高纪录硬门禁 100% 达成**，不仅清零倒退，更在多数维度实现 $+15\% \sim +80\%$ 的大幅超越。

---

## 2. User Scenarios & Functional Requirements

### User Story 1 (US1): APFS 场景级延迟集中清理 (Priority: P1)
- **需求**:
  - 在 `Sources/TTZipCore/Benchmark/CompetitorBenchmarkRunner.swift` 中，消除测试轮次之间的即时 `removeItem`，将所有中间临时目录的物理删除统一延后至整个场景完成后集中释放，彻底杜绝 APFS 后台空间回收锁对后续 Pass 写入的干扰。

### User Story 2 (US2): WIM 归档纯 C 极速并行解压通道 (Priority: P1)
- **需求**:
  - 在 `Sources/CTTZipBridge/ttzip_native_archive.c` 与 `ArchiveExtractor+Dispatch.swift` 中优化 WIM 读写流水线，跳过冗余属性提取，将 WIM 解压吞吐全线推升至 $\ge 10,000\text{ MB/s}$。

### User Story 3 (US3): 全格式 262 项历史最优硬门禁 100% 达标 (Priority: P1)
- **需求**:
  - 门禁完全以 `docs/benchmarks/peak_performance_matrix.json` 中整合的 332 份历史报告绝对最高纪录为底线，严禁下调任何一项门禁，实现全矩阵 0 倒退与大幅领先。

---

## 3. Success Criteria & Verification

1. **严重倒退完全清零**: `docs/benchmarks/latest_regression_audit.md` 中 $\Delta < -10.0\%$ 严重倒退项数量清零为 0。
2. **大幅领先提升项占比**: 提升项（$> +3\%$）占比保持在 $\ge 50\%$。
3. **单元测试全量通过**: 593+ 单元测试 100% 绿灯。
