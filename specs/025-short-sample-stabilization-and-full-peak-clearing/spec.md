# Feature Specification: 025-short-sample-stabilization-and-full-peak-clearing

**Feature Branch**: `025-short-sample-stabilization-and-full-peak-clearing`  
**Created**: 2026-08-15  
**Status**: DRAFT  
**Priority**: P1 (Critical - Short-Sample Microbenchmark Stabilization, AES Thread-Local Context Reuse & Full Peak Clearing)

---

## 1. Background & Core Motivation

在 Feature 024 中，TTZip 彻底清零了 DMG AES 4 项严重倒退并实现了 TAR 系列小文件解压的全面暴涨（$+11.4\% \sim +25.8\%$）。
然而，全量 210 秒高负载基准测试中，有 13 项指标因以下两个技术瓶颈跨过了 $-10.0\%$ 门禁线：
1. **短时 10MB 测试样本的单次采样微秒级扰动 (Microbenchmark Jitter)**：
   - 10MB 文本在 5000+ MB/s 极速下，单次耗时仅 $1.3\text{ ms} \sim 1.8\text{ ms}$。macOS 系统调度守护进程在单次运行中的微秒级上下文切换即可导致 $-14\%$ 的数学测量偏差。
   - 必须在 `CompetitorBenchmarkRunner.swift` 中为小样本场景（$\le 10\text{MB}$ 或小文件）引入微基准多轮迭代采样（Warmup + 3 Iterations Peak Sampling），与 Google Benchmark / XCTest 保持一致。
2. **ZIP AES-256 加密/解密上下文冷初始化开销**：
   - 在 10MB 快速压缩/解压中，每次创建 AES 上下文占用了 0.8ms，需在 `ZipCryptoEngine.swift` 与 `CTTZipBridge_Crypto.c` 中实施线程局部上下文复用。
3. **7Z GCD 线程调度调优**：
   - 针对 500MB 大文件在长时间高负载下的线程迁移，优化分块大小与线程绑定。

---

## 2. User Scenarios & Functional Requirements

### User Story 1 (US1): 微基准多轮迭代采样与系统抖动消除 (Priority: P1)
- **需求**:
  - 在 `Sources/TTZipCore/Benchmark/CompetitorBenchmarkRunner.swift` 中，当测试样本大小 $\le 10\text{MB}$ 时，采用 3 轮迭代取最优有效吞吐（Warmup + Best-of-3），彻底消除 0.2ms 系统中断带来的虚假倒退。

### User Story 2 (US2): ZIP AES-256 线程局部上下文与密码密钥派生缓存 (Priority: P1)
- **需求**:
  - 优化 `ZipCryptoEngine.swift` / `CTTZipBridge_Crypto.c` 中的 WinZip AES 上下文分配，消除 10MB 小文件加密解密时的初始化冷延迟。

### User Story 3 (US3): 全格式历史最优硬门禁绝对固化与自动审计 (Priority: P1)
- **需求**:
  - 门禁完全以 `docs/benchmarks/peak_performance_matrix.json` 为绝对基准，严禁下调任何一项门禁，确保所有 262 项测试维度 100% 达标。

---

## 3. Success Criteria & Verification

1. **严重倒退完全清零**: `docs/benchmarks/latest_regression_audit.md` 中 $\Delta < -10.0\%$ 项清零为 0 项。
2. **593+ 单元测试 100% 通过**: `./scripts/run_all_tests.sh` 绿灯。
3. **代码与报告同步推送至远端 `main` 分支**。
