# Implementation Plan: 022-full-matrix-zero-regression-and-throughput-closure

**Branch**: `022-full-matrix-zero-regression-and-throughput-closure`  
**Feature Spec**: `specs/022-full-matrix-zero-regression-and-throughput-closure/spec.md`  

---

## Technical Context

本项目 TTZip 是基于 Swift 6.0 与 C11 原生编写的高性能 macOS Sonoma 归档压缩引擎。
在完成 10 项 Release 硬门禁达标与 APFS 零拷贝解耦后，当前目标是将全格式 262 项细分维度中剩余的 **28 项严重性能倒退（$< -10.0\%$）** 完全消除，并确保 **TAR.ZST Direct 50MB** 打包突破 $\ge 19,000\text{ MB/s}$ 门禁。

---

## Constitution Check

- [x] **零成本抽象 (Zero-Cost Abstraction)**：所有优化仅在 C11 桥接层与 Swift 底层数据平面执行，热路径零堆分配与零动态多态分发。
- [x] **Fast-Path 旁路保留原则**：ZIP、7Z、TAR.ZST、DMG 各格式专属硬件特化与 NEON SIMD 旁路完整保留。
- [x] **硬性能门禁底线**：严禁下调任何门禁阈值，全部 11 项 Release 门禁与 100% 零倒退必须通过真实算法调优达标。
- [x] **严格日志纪律**：统一使用 `TTLogger`，生产环境与测试环境零裸 `printf`/`print`。

---

## Phase 0: Outline & Research

- R001 [SUBAGENT:research] 《ZIP 大文件与高熵物理写盘 I/O 调优》：研究通过 `posix_fallocate` 与异步并发预分配消除 `CTTZipExtract.c` 中的磁盘写瓶颈。
- R002 [SUBAGENT:research] 《7Z 高熵解压 L2 Cache 对齐》：研究 7Z 解压引擎环形缓冲区从 64KB 调整为 256KB 对 Apple Silicon L2 命中率与吞吐的提升。
- R003 [SUBAGENT:research] 《DMG 镜像管道直通优化》：研究消除 DMG 写入过程中的中间临时文件拷贝。
- R004 [SUBAGENT:research] 《TAR.ZST 50MB Direct 极速调优》：研究 ZSTD 128KB 块与多核直接流式管道突破 19,000 MB/s 的配置。

---

## Phase 1: Design & Contracts

- `data-model.md`: 定义 `RegressionClosureMatrixRecord` 与 `ThroughputFloorVerificationRecord` 实体。
- `contracts/regression_closure_audit.schema.json` [SUBAGENT:research]: 建立 28 项倒退清零全量审计 Schema 契约。
- `quickstart.md`: 编写全格式零倒退与 11 项门禁端到端执行与验证指南。

---

## Phase 2 to Phase 7 Component Touch-points

1. `Sources/CTTZipBridge/CTTZipExtract.c`
2. `Sources/CTTZipBridge/ttzip_tar_zstd_direct.c`
3. `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c`
4. `Sources/TTZipCore/ArchiveWriter+Dispatch.swift`
5. `Tests/TTZipTests/XCTestPerformanceMeasureTests.swift`
6. `scripts/audit_performance_regression.py`
7. `scripts/run_all_tests.sh`
