# Feature Specification: Complete Optimization Wiring & Configuration Creep Audit (全量优化端到端装配与反配置膨胀深度审计)

**Feature Directory**: `specs/093-complete-optimization-wiring-and-configuration-creep-audit/`  
**Status**: DRAFT  
**Priority**: P1  
**Created**: 2026-08-18  

---

## Executive Summary

在经历多轮高密度底层算法与硬件协同加速研发后，TTZip 积累了业界领先的底层原语库（NEON BitShuffle/ByteDelta、Bit-Grooming、4-Way NEON Adler-32、ARM64 PMULL CRC-64/32、SWAR 字符串与 TAR 512B 快速校验、Shannon 熵自适应微探针、128KB L1D 微块容器与无锁插件跳表）。

本规范的目标是：**对全代码库进行地毯式审查，彻底排查并根除配置膨胀（Configuration Creep），消除热路径中残留的堆内存分配（如 `ttzip_tar_native.c` 中的 per-file `malloc`），确保全 16 种归档格式 100% 默认且透明地装配最优硬件与算法路径，实现零配置心智负担下的极致吞吐**。

---

## User Scenarios & Functional Requirements

### User Story 1 (P1): 消除热路径内存分配与 SWAR/特殊值默认装配 (Zero-Allocation Hot Path & SWAR)
- **As a** 系统级高性能归档引擎，
- **I want to** 在所有文件读取、流式写入和格式打包热循环中彻底消除动态堆内存分配（`malloc`/`free`），
- **So that** 避免 GC 压力、页表抖动与系统调用开销，实现内存总线饱和式线速归档。

#### Functional Requirements:
1. `FR1.1`: 重构 `Sources/CTTZipBridge/ttzip_tar_native.c` 中的 `write_reg_file_data`，将 fallback 分支的 `malloc(1MB)` 彻底移除，改为栈上固定 64KB 缓冲区与 `mmap` 零拷贝大文件路径。
2. `FR1.2`: 在 TAR/GZ/BZ2/XZ/ZST 原生打包循环中，默认挂载 `ttzip_swar_is_zero_512b` 与 `ttzip_detect_uniform_block`，实现稀疏块的高速跳过与高效流式写入。
3. `FR1.3`: 确保所有 C 句柄在初始化时结构体清零并在释放前执行安全擦除。

---

### User Story 2 (P1): 全面审查与根除配置膨胀 (Configuration Creep Elimination)
- **As a** 应用程序开发者与 API 调用方，
- **I want to** 调用 `ArchiveWriter` 与 `ArchiveEngineFactory` 时无需配置任何复杂的算法开关或硬件掩码，
- **So that** 引擎内部自动完成 CPU 拓扑识别、Shannon 熵探测、浮点步长分析与内存页对齐，提供开箱即用的最优体验。

#### Functional Requirements:
1. `FR2.1`: 审计 `ArchiveAdvancedOptions`、`SevenZipFormatOptions`、`ZipFormatOptions`、`ZstdFormatOptions`、`TarFormatOptions`，确保所有默认值 100% 对应 Apple Silicon 物理最优参数。
2. `FR2.2`: 任何底层可以通过客观指标（如文件大小 $\ge 16\text{KB}$、熵值 $H > 7.65$、4 字节浮点自相关性 $R(4) \ge 0.70$）自动判定的行为，严禁要求用户手动设置 flag。
3. `FR2.3`: 保持 API 接口极简清晰，消除未使用的废弃字段或冗余中间选项。

---

### User Story 3 (P1): 16 种归档格式全链路端到端装配审计 (16-Format Full Wiring Audit)
- **As a** 质量与架构合规门禁，
- **I want to** 逐一核验全 16 种格式（ZIP, 7Z, TAR, TAR.ZST, TAR.GZ, TAR.BZ2, TAR.XZ, WIM, DMG, ISO, LZ4, LZIP, LRZIP, AAR, BROTLI, SNAPPY）的调用栈，
- **So that** 证明没有任何一种格式遗留在未优化的通用慢路径中。

#### Functional Requirements:
1. `FR3.1`: 逐一建立 16 格式底层调度链路追踪矩阵，验证其全部对接至原生 C 静态库或 Apple Silicon 向量化原语。
2. `FR3.2`: 编写 `ExhaustiveOptimizationAuditTests.swift` 自动化测试，逐项断言 16 格式的底层调用入口与优化策略装配状态。

---

### User Story 4 (P2): 自动化基准与性能零倒退断言 (Zero-Regression Benchmark Invariant)
- **As a** CI/CD 自动化流水线，
- **I want to** 验证在完成热路径内存消除与反配置膨胀治理后，
- **So that** 13 项硬性能门禁指标与全量 1035+ 单元测试 100% 保持绿色通过。

#### Functional Requirements:
1. `FR4.1`: 执行全量回归测试 `swift test` 保持 0 failures。
2. `FR4.2`: 执行 `XCTestPerformanceMeasureTests` 验证 13 项吞吐底线无任何性能倒退。

---

## Success Criteria

1. **热路径堆分配**：`ttzip_tar_native.c` 与所有归档写入热循环中的 `malloc` 调用次数严格降为 **0 次**。
2. **配置透明度**：上层使用默认初始化的 `ArchiveAdvancedOptions()` 即可自动激发全套硬件加速与自适应探针。
3. **16 格式覆盖率**：16 种归档格式 100% 接入特化原生 C 引擎或向量化流水线。
4. **测试门禁回归**：全量单元测试（1035+ 项）100% 通过，13 项硬性能门禁全绿。
