# Feature Specification: 全覆盖测试与基准遥测零回退体系 (Full-Coverage Testing & Benchmark Framework)

**Feature ID**: `162-full-coverage-testing-and-benchmark-framework`  
**Created**: 2026-08-20  
**Status**: Ready for Clarification & Planning  

---

## 1. Executive Summary & Problem Statement

### 1.1 背景与现状
TTZip 在算法选型上已经确立了 100% 直调业界 SOTA 原生库（`libdeflate`, `zstd`, `fast-lzma2`, `lzfse`, `snappy`, `PMULL`）的大方针，并建立了基础的 C11 跑分器、50 点纯内存多维吞吐矩阵（`TTZipCoreCodecBenchmarks.swift`）以及 160 点压缩体积扫描引擎（`CompressionDeltaEngine.swift`）。

然而，对比工业级顶级基石库（如 `zlib-ng` 的微架构多语料差分跑分体系与 `libarchive` 的全格式正交矩阵与 Fuzzing 安全防御），TTZip 现有测试体系仍存在以下核心缺口：
1. **全算法覆盖缺口**：C11 原生跑分器中尚未统一串联 LZ4、Brotli、Bzip2、Blosc2 与 Range Coder；
2. **全格式容器与解压全链路缺口**：缺乏对真实磁盘文件系统归档（ZIP, 7Z, TAR.GZ, TAR.ZST, TAR.BZ2, TAR.XZ, DMG, UnRAR）在千万级小文件与超大文件流下的解包耗时与吞吐自动化度量；
3. **优化零回退自动门禁缺口**：缺乏一套能够在本地单条命令下串联“正确性断言 ➔ 微架构 CPB 遥测 ➔ 50 点全语料吞吐 ➔ 160 点体积防膨胀 ➔ 真实 I/O 内存峰值”的 5 重物理闸门。

### 1.2 目标与收益
全面吸收 `zlib-ng`（微架构/8类标准语料/微观切片）与 `libarchive`（全格式正交/真实兼容包/ZipSlip防御）的核心长处，为 TTZip 建立一套**工业级全覆盖测试、基准遥测与零回退门禁体系**。

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1 (US1): 开发者/CI 执行全算法双向（压缩+解压）微架构基准
- **Given**: 开发者对自研 C 胶水层（如缓冲池、无锁队列、跨域调度）进行了修改；
- **When**: 运行 C11 原生基准测试套件 `./build/ttzip_benchmark_runner --all`；
- **Then**: 必须输出覆盖全部 10 种算法（Deflate L1/6/9, Zstd L1/3, FL2 L3, LZ4, LZFSE, Snappy, Brotli, Bzip2）在 8 类标准语料下的双向压缩/解压吞吐（MB/s）与每字节周期（CPB），且耗时不超过 200 ms。

### User Scenario 2 (US2): 全格式容器与真实磁盘 I/O 压测
- **Given**: 一个包含 10,000 个碎片小文件或 1GB 大流的真实测试目录树；
- **When**: 触发端到端容器归档与解包压测；
- **Then**: 针对 `.zip`, `.7z`, `.tar.gz`, `.tar.zst`, `.tar.bz2`, `.tar.xz`, `.dmg`, `.rar` 执行自动化打包与解包，记录端到端挂钟耗时、吞吐与峰值驻留内存（Peak RSS <= 128MB），并断言解压后文件哈希 100% 比特一致。

### User Scenario 3 (US3): 5 重物理闸门零回退自动化判定
- **Given**: 代码提交前的本地 Git 拦截或 PR 自动化流水线；
- **When**: 执行 `scripts/run_optimization_gate.sh`；
- **Then**: 自动按序流转 5 重闸门：
  1. 正确性与安全确界（21 套 C 单测 100% 通过，ASan 零泄漏）；
  2. 微架构与反汇编防劣变（CRC32 >= 65 GB/s, 0 栈溢出）；
  3. 50 点全语料吞吐矩阵（CV% < 1.0%, 吞吐 >= 98% 基线）；
  4. 160 点逐级压缩体积防膨胀（0 个 REGRESSION）；
  5. 真实 I/O 内存峰值与解压无损验证。
  任一闸门红灯立即终止并输出精确失败诊断。

---

## 3. Functional Requirements

### 3.1 C11 核心基准测试扩展 (bench_codecs.c & bench_formats.c)
- **REQ-001**: `bench_codecs.c` 必须补齐 LZ4 (L1/9)、Brotli (Q6/9)、Bzip2 (L1/9) 的成对压缩与解压吞吐度量；
- **REQ-002**: 必须提供每种算法对应的 `Cycles/Byte (CPB)` 精确计算输出；
- **REQ-003**: 建立独立的容器格式基准套件 `bench_formats.c`，对 ZIP (Deflate/Store)、TAR.GZ、TAR.ZST、7Z (Solid)、UnRAR 进行端到端解压速率评估。

### 3.2 标准语料集生成器对齐 (BenchmarkCorpusGenerator)
- **REQ-004**: 统一 C 层与 Swift 层的 8 大标准语料（Text, ShortMatch, DNA, Random, Literals, Mixed, RealisticRGB, StripedRGB），确保跨语言基准语料 100% 确定性可重现；
- **REQ-005**: 语料生成器在热测试循环内必须维持 0 堆分配与 64 字节缓存行对齐。

### 3.3 自动化 5 重闸门调度流水线 (scripts/run_optimization_gate.sh)
- **REQ-006**: 提供一键式本地/CI 测试门禁脚本，整合 CMake CTest、C11 Benchmark Runner、Swift 50 点矩阵、160 点 Delta 引擎与 CLI 端到端压测；
- **REQ-007**: 支持 `--json-out` 输出结构化测试报表，支持 `--assert-zero-regression` 强制断言。

---

## 4. Success Criteria

| 维度 | 量化指标 | 判定依据 |
| :--- | :--- | :--- |
| **算法覆盖率** | **10/10 核心算法** (100%) | 覆盖 Deflate, Zstd, LZMA2, LZ4, LZFSE, Snappy, Brotli, Bzip2, Blosc2, RangeCoder |
| **容器格式覆盖率** | **8/8 主流容器格式** (100%) | 覆盖 ZIP, 7Z, TAR, TAR.GZ, TAR.ZST, TAR.BZ2, TAR.XZ, UnRAR |
| **解压链路覆盖** | **100% 双向覆盖** | 所有支持解压的格式均配备独立解压吞吐与元数据校验用例 |
| **全量跑分总耗时** | **<= 2.5 秒** | 50 点吞吐 + 160 点体积 + C 微内核跑分整机执行时间 |
| **回归误报率** | **变异系数 CV% < 1.0%** | 多次重复运行结果稳定可信，无系统噪声误报警 |
| **内存峰值约束** | **Peak RSS <= 128 MB** | 任何基准或全链路归档测试过程中驻留内存严格受控 |
