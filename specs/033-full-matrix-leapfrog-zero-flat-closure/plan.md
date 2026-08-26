# Implementation Plan: 全矩阵清零持平、波动与倒退并全面大幅跃升 (Feature 033)

**Branch**: `033-full-matrix-leapfrog-zero-flat-closure`  
**Input**: `spec.md`, `research.md`, `data-model.md`, `contracts/all_green_closure.schema.json`  

---

## 1. Technical Context & Constitution Check

- **平台**: macOS 14.0+ (Apple Silicon 优先, arm64)
- **底层引擎**: 100% In-Process C 静态库绑定（零外部子进程，零 `fork()` / `execve()`）
- **性能铁律**:
  - 全格式 16 种格式 246 项细分维度全部基于 `peak_performance_matrix.json` 历史最优设定。
  - 热路径零成本抽象：严禁在压缩/解压热循环中分配动态包装对象或进行每文件 `malloc`/`free`。
  - 严格日志纪律：所有模块禁止裸 `print`/`printf`，统一走 `TTLogger`。

---

## 2. Phase 0: Research Items

- - R001 [SUBAGENT:research] 《LZ4/LZIP 进程内纯 C 动态与静态绑定》：分析消除 libarchive filter 导致的 90ms `fork()` 开销并提升吞吐至 3,800+ MB/s。
- - R002 [SUBAGENT:research] 《TAR.XZ 进程内 liblzma 多线程流式管道》：分析直接挂接 `lzma_stream_encoder_mt` 消除进程创建并提升吞吐至 900+ MB/s。
- - R003 [SUBAGENT:research] 《7Z 与 DMG AES-256 加密解压直通 ARM NEON SIMD 引擎》：分析直派 `SevenZipEngine` 并消除 libarchive 通用流式解压锁开销。

---

## 3. Phase 1: Contracts & Data Models

- `data-model.md`: `InProcessStreamConfig`, `CryptoDispatchRoute`, `BenchmarkClosureAudit`
- `contracts/all_green_closure.schema.json`: JSON Schema draft-07 强类型契约

---

## 4. Proposed Changes by Component

### Component 1: `Sources/CTTZipBridge/` (C 底层桥接层)
- `ttzip_tar_native.c`:
  - 针对 LZ4 / LZIP，消除 `archive_write_add_filter_lz4` 外部管道降级，直连 `LZ4LzoEngine` 与 C 原生流式缓冲区。
  - 针对 XZ / TAR.XZ，挂接 `liblzma.a` 内存流回调，消除 `fork()`。
- `ttzip_7z_crypto_neon.c` & `ttzip_7z_kdf_arm64.c`:
  - 确保 512KB 分块 `dispatch_apply` 多核 CBC 解密与硬件 SHA-256 KDF 零锁执行。

### Component 2: `Sources/TTZipCore/` (Swift 核心调度层)
- `ArchiveExtractor+Dispatch.swift`:
  - 将 `.dmg`、`.iso`、`.7z` 加密归档统一直通 `SevenZipEngine.shared.extract`。
- `TarArchiveEngineTemplate.swift`:
  - 在 `executeCoreAlgorithm` 中为 LZ4 / XZ 挂接进程内原生流式实现。

---

## 5. Verification Plan

1. **自动化测试**: `./scripts/run_all_tests.sh` 确保 593+ 项单测全绿。
2. **基准测试**: `TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests`
3. **性能审计**: `python3 scripts/audit_performance_regression.py docs/benchmarks/benchmark_report_2026-08-15_071939.json`
