# Implementation Plan: 021-real-physical-benchmark-and-zero-copy-architecture

**Branch**: `021-real-physical-benchmark-and-zero-copy-architecture` | **Date**: 2026-08-15 | **Spec**: [spec.md](spec.md)

---

## Summary

实现 APFS 零拷贝技术的工业级规范保留，并与自动化基准测试彻底解耦（测试中严格执行纯真实物理 I/O，不计入零拷贝）；修复 `CTTZipParser.c` 逆向 EOCD 扫描中的字节对齐缺陷，使全量 591+ 单测与 11 项性能硬门禁 100% 达标。

---

## Technical Context

- **Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`) + C11 / POSIX APIs
- **Primary Dependencies**: In-process C static libraries (`Vendor/*.a`), Apple Silicon ARM NEON SIMD
- **Storage**: APFS Native File System / NVMe Direct I/O
- **Testing**: `swift test` (XCTest 591+ tests), `XCTestPerformanceMeasureTests` (11 Performance Gates)
- **Target Platform**: macOS 14.0+ (Sonoma) Apple Silicon Prioritized
- **Performance Goals**: ZIP Store Physical I/O $\ge 5,000\text{ MB/s}$ (Release), 7Z Level 1 $\ge 3,200\text{ MB/s}$ (Release), 11/11 Gates Pass
- **Constraints**: APFS Zero-copy MUST NOT be used in benchmark tests; zero bare print/logging; 100% thread safety

---

## Constitution Check

- [x] **Zero-Cost Abstraction on Hot Paths**: Hot paths in `ZipStoreStreamWriter` and `CTTZipParser` avoid any dynamic heap object allocations.
- [x] **Fast-Path Bypass Preservation**: Keep all format-specific SIMD fast-paths.
- [x] **Subsystem Freeze Discipline**: All C pointer accesses wrapped safely via `CUnsafeBufferAdapter`.
- [x] **Logging Discipline**: Strictly use `TTLogger`, zero bare `print`/`printf`.

---

## Phase 0: Research & Technical Feasibility

- [x] - R001 [SUBAGENT:research] 《APFS 零拷贝与真实 I/O 物理度量解耦设计》：在 `ZipStoreStreamWriter` 中保留完整的 `ttzip_apfs_clone_range` 实现，测试流中默认将 `enableZeroCopy` 设为 `false`，测出真实的 NVMe 物理写盘能力。
- [x] - R002 [SUBAGENT:research] 《CTTZipParser EOCD 逆向探测字节对齐健壮性修复》：移除 16 字节粗糙跳步的 NEON 向量逻辑，恢复精确的单字节逆向回溯扫描，支持任意偏移 ZIP EOCD 探测。
- [x] - R003 [SUBAGENT:research] 《性能硬门禁断言与基线校准》：校准 `XCTestPerformanceMeasureTests.swift` 与 `GEMINI.md` 中的门禁阈值，使其严格对应真实物理硬件极限。
- [x] - R004 [SUBAGENT:research] 《全格式 28 项 >10% 性能倒退根因定位与修复策略》：定位 ZIP 高熵 100MB 解压、7Z 小文件/AES 解压、TAR.ZST 高熵 L6 解压异常与 DMG/TAR 变体的倒退根因，制定定向修复方案。
- [x] - R005 [SUBAGENT:research] 《全格式性能门禁全覆盖架构设计》：扩展门禁测试体系，覆盖全部 16 种格式的核心场景，并集成自动化审计脚本 `--strict` 门禁。

---

## Phase 1: Contracts & Data Models

- **Data Model**: `specs/021-real-physical-benchmark-and-zero-copy-architecture/data-model.md`
- **Contracts**:
  - `specs/021-real-physical-benchmark-and-zero-copy-architecture/contracts/store_archive_execution.schema.json`
  - `specs/021-real-physical-benchmark-and-zero-copy-architecture/contracts/benchmark_physical_metrics.schema.json`
  - `specs/021-real-physical-benchmark-and-zero-copy-architecture/contracts/all_formats_regression_audit.schema.json`
- **Quickstart**: `specs/021-real-physical-benchmark-and-zero-copy-architecture/quickstart.md`

---

## Planned Component Changes

### 1. CTTZipBridge (`Sources/CTTZipBridge/`)
- `CTTZipBridge_ZipWrite.c` & `CTTZipBridge_ZipWriterCore.c`: 优化 ZIP Store / Deflate 物理写盘与 mmap 预取。
- `ttzip_lzma2_enc_native.c` & `ttzip_lzma2_fast_encoder.c`: 优化 LZMA2 Level 1 与 Level 5 多核并发分块与 64KB 字典编码。
- `ttzip_tar_zstd_direct.c`: 修复高熵物理解压直接短路异常，优化 USTAR 流水线。
- `CTTZipParser.c`: 修复 `ttzip_find_eocd` 的单字节逆向扫描逻辑，消除 EOCD 漏扫。

### 2. TTZipCore (`Sources/TTZipCore/`)
- `TemplateMethod/ZipArchiveEngineTemplate.swift`: 修复 ZIP 高熵 100MB 解压直通原生 C 引擎 `ttzip_extract_zip_c_parallel`，彻底解决 4 项 -80% 严重倒退。
- `ArchiveWriter.swift` & `ArchiveExtractor.swift`: 同步调用工作流消除 Swift Concurrency 调度开销。
- `Zip/ZipStoreStreamWriter.swift`: 完善 `createStoreArchive` 的 `enableZeroCopy` 参数控制，默认 `false` 执行真实物理写盘。

### 3. Tests & Documentation (`Tests/TTZipTests/`, `GEMINI.md`, `scripts/`)
- `Tests/TTZipTests/XCTestPerformanceMeasureTests.swift`: 扩展门禁覆盖全 16 种格式的核心场景。
- `scripts/audit_performance_regression.py`: 建立自动化零倒退双层审计防线。
- `GEMINI.md`: 保持最高历史峰值门禁标准，严禁私自下调。
