# Implementation Plan: ZIP Compression Architecture & Micro-Optimization Survey (112-zip-architecture-and-micro-optimization)

## Technical Context

TTZip's ZIP compression pipeline spans three distinct layers:
1. **Swift High-Level Orchestrator**: `ZipArchiver`, `ZipParallelWriter`, `ZipExtremeBlockWriter`, `ZipCompressionProfile`, `ZipStoreStreamWriter`, `ZipBlockParallelCompressor`.
2. **Buffer & I/O Plane**: APFS Range Clone CoW (`ttzip_apfs_clone_range`), Direct I/O (`ttzip_apfs_preallocate`), `ZipMemoryEngine`.
3. **In-Process C11/POSIX Core Engines**: `native_deflate` (`ttzip_deflate_engine`), `ttzip_zopfli_engine`, `libdeflate`, `ttzip_huffman_inplace`.

### Key Technical Goals & Opportunities:
- **Zero-Allocation Core Codec**: Replace dynamic `malloc`/`free` and 512KB/768KB `memset` in `native_deflate` with thread-local scratchpad state (`g_tls_deflate_lazy_mf`).
- **NEON 128-Bit Match Length Finding**: Upgrade scalar SWAR 8-byte comparison in `ttzip_deflate_fast.c` and `ttzip_deflate_lazy.c` to 128-bit `uint8x16_t` vectorization (`vceqq_u8`).
- **Fixed-Point DAG Cost Evaluation**: Inject Q8.8 fixed-point log2 lookup (`ttzip_fast_log2_fixed`) into Zopfli DAG shortest-path parser, eliminating `double` arithmetic and function pointer dispatch.
- **100,000+ Small File Batch Aggregation**: Replace 8,248-byte `ttzip_c_item_t` with 48-byte `ttzip_compact_item_t` and a continuous string arena; replace 500,000+ discrete `pwrite` calls with 4MB aligned multi-entry buffer flushing.
- **Continuous 32KB Sliding Dictionary**: Decouple contiguous memory assumptions in C match-finder, allowing segmented non-contiguous 32KB window passing across all parallel tiles.

---

## Constitution Check

- [x] **Zero-Cost Abstraction on Hot Paths**: Hot loops bypass dynamic objects, Visitor/Decorator trees, and intermediate `Data(count:)` zeroing.
- [x] **Fast-Path Preservation**: Dedicated fast-paths (Store CoW, Level 1 parallel Deflate, WinZip AES-256 SIMD) are strictly preserved.
- [x] **Throughput Floors**: Meets all performance floors (>= 5,000 MB/s parallel Level 1, >= 6,000 MB/s Store).
- [x] **Bounds-First & Invariant Security**: Strictly bounded heap memory (<= 64MB per worker), zero buffer overflows, safe pointer arithmetic across segmented buffers.
- [x] **Oracle-First Testing**: Bit-exact archive verification against standard `unzip -t`, `7zz t`, and `bsdtar -tvf`.

---

## Phase 0: Outline & Research

- R001 [SUBAGENT:research] 《Swift 层 ZIP 压缩调度与管道架构深度审计》：梳理调度链条、分块切分、内存模型与 32KB 跨块字典。
- R002 [SUBAGENT:research] 《C 桥接层与底层原生 Deflate/Zopfli 编解码器性能与内存拓扑审计》：审计 TLS 内存复用、ARM64 NEON 向量化、定点数 DAG 熵模型与历史窗口安全性。
- R003 [SUBAGENT:research] 《100,000+ 小文件高频压缩热路径与单遍流式写入优化》：审计目录遍历系统调用、紧凑元数据 Arena、4MB 聚合缓冲与 APFS 弹性预分配。

---

## Phase 1: Design & Contracts

- **Data Model**: `specs/112-zip-architecture-and-micro-optimization/data-model.md`
- **Contracts**:
  - `contracts/zip_compression_plan_contract.json` [SUBAGENT:research]
  - `contracts/zip_micro_optimization_telemetry.json` [SUBAGENT:research]
- **Validation Guide**: `specs/112-zip-architecture-and-micro-optimization/quickstart.md`

---

## Proposed Component Changes

### 1. `Sources/CTTZipBridge/native_deflate/`
- Introduce thread-local `ttzip_deflate_fast_mf_t` and `ttzip_deflate_lazy_mf_t` scratchpad state.
- Implement NEON 128-bit `ttzip_fast_match_len_neon128`.
- Eliminate per-block `memset` via epoch-based hash invalidation.

### 2. `Sources/CTTZipBridge/ttzip_zopfli_engine.c` & `zopfli/`
- Activate Q8.8 fixed-point entropy model (`ttzip_fast_log2_fixed`).
- Support non-contiguous 32KB history window pointers without intermediate `malloc`/`memcpy`.

### 3. `Sources/CTTZipBridge/CTTZipBridge_ZipWrite.c` & `CTTZipBridge_ZipWriterCore.c`
- Introduce 48-byte `ttzip_compact_item_t` and string memory arena.
- Implement 4MB aligned double-buffered stream sink for multi-entry batch aggregation.
- Implement two-stage resilient APFS preallocation with final `ftruncate` boundary alignment.

### 4. `Sources/TTZipCore/Zip/`
- Optimize `ZipBlockParallelCompressor.swift`: eliminate `Data(count:)` zeroing.
- Optimize `ZipParallelWriter.swift`: eliminate lock contention and redundant `Data` wrappers.
- Wire single-file multi-block Extreme routing into `ArchiveWriter+Dispatch.swift`.
