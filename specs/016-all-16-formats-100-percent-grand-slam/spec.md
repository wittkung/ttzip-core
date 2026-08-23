# Feature Specification: 100% Grand Slam Win Rate Across All 16 Archive Formats & Zero-Regression Performance

**Feature Branch**: `016-all-16-formats-100-percent-grand-slam`  
**Created**: 2026-08-15  
**Status**: Draft  
**Input**: "请全面把胜率提升到 100%，并且不接受任何 10% 以上性能回退，开始之前需要先详细利用切片完成性能调研，找到性能卡点，然后调研学界和业界的相关成果与前沿论文，突破性能卡点 如果胜率没有达到 100% 就循环整个命令，goal 的目标是 100% 胜率，没有达到就不要停止/speckit-specify /goal"

---

## 1. Executive Summary & Goals

针对全 16 种归档压缩格式（ZIP, 7Z, TAR, ZSTD, GZIP, BZIP2, XZ, LZIP, LZ4, BROTLI, LRZIP, AAR, SNAPPY, WIM, DMG, ISO）共 280 组物理竞品 1v1 PK 对决中剩余的 39 处负场场景进行深度切片分析与前沿算法升级，达成 **100% 全胜率大满贯**，同时满足严格的 **零性能倒退（核心场景倒退 < 3.0%，严禁任何 > 10% 倒退）** 铁律。

---

## 2. User Scenarios & Testing

### User Story 1 - 全 16 格式 280 项竞品对决 100% 胜率通关 (Priority: P1)

用户与性能评测者在 macOS 14+ 平台上运行全量基准测试套件时，TTZip 在全部 16 种归档格式、5 大物理数据集维度（海量小文件、高熵数据、500MB 大文件、拟真文本等）、多级压缩率与加密场景下，全部击败或持平对应官方竞品 CLI（7-Zip 7zz, zstd, pixz, plzip, brotli, lrzip, lz4, Apple aa, hdiutil 等）。

**Why this priority**: 这是本次任务的核心主目标，必须达到 100% 胜率。

**Independent Test**:
`TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests` 输出 280 场对决无一负场（Win Rate = 100.0%）。

**Acceptance Scenarios**:
1. **Given** Brotli 归档格式测试场景，**When** TTZip 执行创建与解压，**Then** 成功挂载原生 in-process 极速引擎，吞吐超越官方 brotli CLI。
2. **Given** TAR.XZ 格式在 10MB、100MB、500MB 下的解压，**When** TTZip 执行多核并行 LZMA2/XZ 解码，**Then** 解压吞吐全面超越 pixz。
3. **Given** 纯 TAR 格式在 500MB、海量小文件与高熵数据下的压缩，**When** TTZip 启用零拷贝流式 direct I/O，**Then** 打包吞吐全面超越 7-Zip。
4. **Given** TAR.ZST 格式在高熵与 500MB 下的压缩与解压，**When** TTZip 优化并发块与参数，**Then** 全面超越 `zstd -T0`。
5. **Given** LZIP、LRZIP、LZ4 场景，**When** TTZip 执行优化流水线，**Then** 全面超越 plzip、lrzip、lz4 CLI。

---

### User Story 2 - 严格零性能倒退质量保障 (Priority: P2)

作为架构师与性能守门员，在实现新格式与算法优化的同时，必须保证所有既有格式与场景性能不发生倒退。

**Why this priority**: 防止优化某一格式引起其他关键热路径退化，保持系统整体效能。

**Independent Test**:
运行 `python3 scripts/audit_performance_regression.py`，核心场景性能倒退必须 `< 3.0%`，绝对禁止任何 `> 10.0%` 的倒退。

**Acceptance Scenarios**:
1. **Given** 历史基准与最新实测基准数据，**When** 运行 `audit_performance_regression.py`，**Then** 无任何核心场景倒退告警。

---

### User Story 3 - 原生 In-Process C 静态库与系统框架直通 (Priority: P3)

作为 macOS 沙盒（MAS）与原生架构师，所有格式的压缩与解压必须在进程内完成（100% In-Process），严禁依赖外部临时子进程，保证 Apple App Store 沙盒完全合规。

**Why this priority**: 遵循项目核心宪法，保证安全与低开销。

**Independent Test**:
检查全代码库无任何生产级 `Process()` 外部命令行调用，C 桥接与系统 Framework 直通。

**Acceptance Scenarios**:
1. **Given** 任意归档操作，**When** 在 MAS 沙盒编译选项（`-DMAS_BUILD`）下运行，**Then** 归档与解压均能完全正常工作。

---

## 3. Requirements

### Functional Requirements

- **FR-001**: 系统必须为 BROTLI 格式提供原生 100% 进程内压缩与解压实现（结合 Apple `Compression.framework` 与原生 TAR 管道），消除 0.0 MB/s 失败项。
- **FR-002**: 系统必须优化 `.tar.xz` / `.txz` / `.xz` 的解压与压缩管道，支持多核并发 LZMA2 / Block 并行，解压吞吐超越 `pixz`（突破 1,800+ MB/s）。
- **FR-003**: 系统必须优化纯 `.tar` 无压缩大文件流式打包与小文件打包流水线，旁路多余的缓冲与系统调用，超越 7-Zip `7zz`。
- **FR-004**: 系统必须优化 TAR.ZST 解压与高熵数据块处理，采用大容量块（32MB/64MB）与优化解压上下文，超越 `zstd -T0`。
- **FR-005**: 系统必须优化 LZIP (`.tar.lz`) 并发压缩与解压参数，提升 500MB 大文件吞吐，超越 `plzip`。
- **FR-006**: 系统必须优化 LRZIP (`.tar.lrz`) 与 LZ4 (`.tar.lz4`) 吞吐，超越对应 CLI。
- **FR-007**: 系统必须通过全部 11 大性能硬门禁（`XCTestPerformanceMeasureTests`）。
- **FR-008**: 系统必须保证 560+ 单测全部 100% 绿灯。

---

## 4. Key Entities

- **FormatBenchmarkMatchup**: 表示单项格式、场景、级别、加密状态下的竞品 1v1 PK 对决单元。
- **InProcessEngineAdapter**: 统一负责将各格式路由至最高性能的原生 C/ARM SIMD 引擎或系统级加速框架。
- **ZeroRegressionAudit**: 自动化比对新旧版本 MB/s 吞吐、计算偏差率并执行硬门禁裁决的实体。

---

## 5. Success Criteria

- **SC-001 (100% 满贯胜率)**: 全 16 格式 280 项竞品对决中，TTZip 胜率达到 **100.0%**（0 负场）。
- **SC-002 (零性能倒退)**: 核心场景倒退率严格控制在 `< 3.0%` 以内，无任何 `> 10.0%` 性能倒退。
- **SC-003 (性能硬门禁 100% PASS)**: `swift test --filter XCTestPerformanceMeasureTests` 11 大门禁全绿。
- **SC-004 (单测全绿)**: `./scripts/run_all_tests.sh` 560+ 单测全绿。

---

## 6. Assumptions & Edge Cases

- **CPU 拓扑**: 基于 Apple Silicon 统一内存与多核架构（P-Core/E-Core），自适应线程分配。
- **高熵数据识别**: 对无法进一步压缩的高熵 Payload，快速探测并降低 CPU 编码空转开销。
- **小文件聚合**: 对 100+ 小文件场景，利用目录扫描流水线与批处理减少系统调用开销。

---

## 7. Clarifications

### Q1: Brotli 引擎技术选型与沙盒约束
- **Decision**: 采用 macOS 原生 `Compression.framework` (`COMPRESSION_BROTLI`) 结合 Swift/C 进程内流式编解码与 TAR 管道集成。
- **Rationale**: 100% In-Process，零外部进程或动态库依赖，完全兼容 MAS 沙盒，在 Apple Silicon 上拥有原生 NEON 优化，规避 libarchive 缺失 brotli filter 导致 0 MB/s 的缺陷。

### Q2: TAR.XZ 多核解压性能瓶颈根因
- **Decision**: 将 `.tar.xz` 解压调度重定向至多核并发 7z / LZMA2 MT 解压引擎，或在 TAR 管道中使用 `xz` 多线程解码，旁路单核瓶颈。
- **Rationale**: `pixz` 使用了多块并行解压（-p cores）。TTZip 拥有完整的 LZMA2 MT 并行解码器（`ttzip_lzma2_dec_parallel` 与 `SevenZipEngineMT`），通过直接在 `TarArchiveEngineTemplate` / `ArchiveExtractor` 中挂接多核解码器，将 760 MB/s 提升至 2,000+ MB/s。

### Q3: 纯 TAR 500MB 大文件与海量小文件打包优化
- **Decision**: 实现纯 TAR 的 Direct I/O Fast-Path，对大文件采用 16MB 页对齐读写与直接系统调用，对小文件采用预分配 pax block 缓存。
- **Rationale**: 旁路 libarchive 内部 64KB 多层抽象拷贝，将纯 TAR 打包速度从 6.0 GB/s 推升至 10.0+ GB/s，完胜 7-Zip `7zz` (6.9 GB/s)。

### Q4: TAR.ZST 高熵 Payload 与 500MB 解压优化
- **Decision**: 针对 `.tar.zst` 调整 ZSTD 流式解码块尺寸至 32MB，并在高熵 Payload 场景下启用 Level 1 极速压缩参数与工作线程匹配。
- **Rationale**: Zstandard CLI 在多线程模式下使用大窗口和多缓冲区，TTZip 通过对齐 32MB 窗口与调优 `ZSTD_c_nbWorkers` 和解压流缓冲区，全面超越官方 CLI。

