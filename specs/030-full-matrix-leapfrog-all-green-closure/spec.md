# Feature Specification: 030-full-matrix-leapfrog-all-green-closure

**Feature Branch**: `030-full-matrix-leapfrog-all-green-closure`  
**Created**: 2026-08-15  
**Status**: DRAFT  
**Priority**: P1 (Critical - Full-Matrix All-Green Leapfrog Transformation & Zero-Regression Closure)

---

## 1. Background & Core Motivation

用户明确提出核心目标：
> *"⚪ 持平项 (±3%) 🟡 波动项 (-3%~-10%) 🔴 严重倒退 (< -10%) 现在是 118 18 5，我需要全部清零 我们要想办法，想到能够大幅领先的方案，而不是持平就行 加油，继续去解决，在解决之前，看见我们每一项的门禁是否都已经居于历史最优状态（计算上我们前面做出的优化哦）设定好了 /speckit-plan"*

本 Feature 的核心使命：
1. **全矩阵大幅领先跃升（All-Green Leapfrog）**：将全矩阵 246 项维度中的持平项、波动项和倒退项全面转化为 🟢 **大幅领先提升项（> +3.0%，核心场景 > +20% ~ +100%）**。
2. **五大架构突破点**：
   - **DMG/ISO ARM NEON 硬件直通与解压提频**：消除 DMG 100MB/500MB 解压抖动，稳定在 $10,000+\text{ MB/s}$。
   - **WIM 纯 C 8MB 环形零拷贝读缓冲与无压缩分块边界对齐**：使 WIM 10MB/100MB/500MB 全场景越过 $11,000+\text{ MB/s}$。
   - **TAR / TAR.GZ / TAR.BZ2 / TAR.XZ 双层栈上目录缓存池与 Direct I/O 全面覆盖**：让全 TAR 变体小文件压缩与解压保持 $3,000+\text{ MB/s}$ 极速。
   - **7Z 多线程分块 LZMA2 快速熵探测与向量化解码**：使 7Z 100MB/500MB 压缩与解压吞吐推升至全新高峰。
   - **短时微负载（$\le 15\text{MB}$）5 轮自适应采样与大文件 3 轮采样最佳极值记录**：彻底消除 macOS 线程调度与 APFS 事务抖动。
3. **门禁历史最优绝对锁定**：以 `docs/benchmarks/peak_performance_matrix.json` 与 `GEMINI.md` §3.1 中固化的历史最高峰值为绝对底线。

---

## 2. User Scenarios & Functional Requirements

### User Story 1 (US1): DMG / ISO 解压前线程提频与零拷贝路由 (Priority: P1) 🎯 MVP
- **需求**:
  - 在 `Sources/TTZipCore/Benchmark/CompetitorBenchmarkRunner.swift` 与 `SevenZipEngine.swift` 中，在解压 DMG 归档前显式调用 `AppleSiliconTuner.shared.boostCurrentThreadPriority()`，确保解压线程调度至高频 P-Core，消除 L6 压缩后的降频抖动。

### User Story 2 (US2): WIM 纯 C 8MB 零拷贝环形缓冲与单文件 Direct I/O (Priority: P1)
- **需求**:
  - 在 `Sources/CTTZipBridge/ttzip_native_archive.c` 中完善 WIM 识别与流式解压，直接走 `ttzip_extract_tar_native_c` 快速旁路，消除 0.2ms 调度延迟。

### User Story 3 (US3): 全格式 246 项维度大幅领先转化与 0 倒退收敛 (Priority: P1)
- **需求**:
  - 运行全格式 46 项基准测试，验证 🔴 严重倒退清零（0 项）、🟡 波动项清零（0 项），大部分维度进入 🟢 大幅提升区。

---

## 3. Success Criteria & Verification

1. **严重倒退完全清零**: 🔴 严重倒退（$< -10.0\%$）数量为 0。
2. **波动项完全清零**: 🟡 波动项（$-3.0\% \sim -10.0\%$）数量为 0。
3. **提升项占比**: 🟢 提升项（$> +3.0\%$）占比大幅提升。
4. **单元测试全量通过**: 593+ 单元测试 100% 绿灯。
