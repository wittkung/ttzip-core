# Phase 0 Grounded Research: Complete Optimization Wiring & Configuration Creep Audit

**Feature**: `specs/093-complete-optimization-wiring-and-configuration-creep-audit`  
**Date**: 2026-08-18  

---

## Research Item 1: Zero-Allocation Hot-Path Invariant Audit in `ttzip_tar_native.c` & Streaming Writers (R001)

### Decision
1. **Eliminate All Per-File Heap Allocations in `write_reg_file_data` & `add_file_or_dir_to_archive`**:
   - In `write_reg_file_data`, replace the `malloc(1MB)` fallback with a 64KB stack buffer loop (`char stack_buff[65536]`) and direct POSIX `read()`.
   - In `add_file_or_dir_to_archive` and `add_item_to_zstd_stream`, replace all `asprintf`/`strdup` path concatenations with fixed stack buffers (`char sub_full[4096]`, `char sub_rel[4096]`) and `snprintf`.
   - In LongLink header generation, replace `calloc(1, total_payload)` with a 4KB stack buffer `uint8_t name_buf[4096]`.
2. **Unify and Integrate 64-bit SWAR Zero Detection**:
   - Standardize `ttzip_swar_is_zero_512b(const uint8_t block[512])` and replace the scalar `for (int i = 0; i < 512; i++)` in `ttzip_tar_zstd_direct.c:673`.
3. **Upgrade Small Buffer Sizes to 64KB**:
   - Replace 4KB stack buffers in `ttzip_tar_zstd_direct.c` with 64KB stack buffers (`65536` bytes) to saturate APFS/NVMe cluster sizes while staying within secondary thread stack limits (512KB).

### Rationale
- Eliminates heap allocation lock contention and arena fragmentation across GCD worker threads.
- Stack buffers are immediately reclaimed on function return with zero syscall overhead.
- Branchless SWAR evaluates 512-byte headers in 8 operations (< 10 ns) instead of 512 branch evaluations.

### Alternatives Considered
- **Thread-Local Storage (TLS) / Heap Pool**: Rejected due to TLS cleanup complexity across GCD worker queues and increased memory footprint.
- **100% Mandatory `mmap`**: Rejected because `mmap` fails on special descriptors and virtual filesystems.

### Source
- `Sources/CTTZipBridge/ttzip_tar_native.c:28-64`, `ttzip_tar_zstd_direct.c:100-144, 673-677`

---

## Research Item 2: Configuration Creep Analysis & Default Transparent Parameterization (R002)

### Decision
Internalize and automate low-level compression heuristics across `ArchiveAdvancedOptions` and format-specific structs (`ZipFormatOptions`, `SevenZipFormatOptions`, `ZstdFormatOptions`, `TarFormatOptions`):
1. **CPU Cores Auto-Tuning**: When `cpuThreads == 0` or omitted, automatically resolves to `AppleSiliconTuner.shared.optimalCompressionThreads` and executes `boostCurrentThreadPriority()`.
2. **LDM & Memory Windowing**: Automatically bound `zstdWindowLog` to unified memory tiers (8/16GB: 64MB; 24/36GB: 1024MB; 48/64GB: 2048MB; 96/128GB: 4096MB) and clamp against source file size $\lceil \log_2(\text{fileSize}) \rceil$.
3. **Transparent Defaults (Zero Configuration Creep)**: Public configurations expose only user intent (level, password, encryption, solid mode, filtering). Inert options (`matchFinder`, `numFastBytes`, `jobSizeMB`) are eliminated from caller burden.

### Rationale
- Follows Rule §5.7 (*Zero Configuration Creep*): System-level libraries must not push heuristics to callers when the library can safely evaluate them via objective parameters.

### Alternatives Considered
- **Exposing Fine-Grained Knobs to C Bridging**: Rejected because static manual tuning causes OOM on 8GB machines or under-utilization of L2/L3 cache on M-Max/Ultra chips.
- **Untyped Dictionaries (`[String: Any]`)**: Rejected due to lack of compile-time type safety.

### Source
- `Sources/TTZipCore/ArchiveCompressionTypes.swift`, `Sources/TTZipCore/AppleSiliconTuner.swift`

---

## Research Item 3: 16-Format Full-Stack Engine Wiring & Dispatch Verification Matrix (R003)

### Decision
Confirm that all 16 supported formats (**ZIP, 7Z, TAR, TAR.ZST, TAR.GZ, TAR.BZ2, TAR.XZ, WIM, DMG, ISO, LZ4, LZIP, LRZIP, AAR, BROTLI, SNAPPY**) are 100% wired directly to dedicated fast-path native in-process C engines or Apple Silicon SIMD-accelerated frameworks. Zero external CLI subprocesses exist.
- Formulate `ExhaustiveOptimizationAuditTests.swift` enforcing an automated 5-stage verification pipeline across all 16 formats (Creation -> Binary Signature Magic -> Metadata Introspection -> Extraction -> Bitwise SHA-256 Differential Oracle).

### Rationale
- Direct in-process C static library binding (`Vendor/*.a`), GCD multithreaded chunk encoders (`CTTZipBridge_GzParallel.c`), and OS-level frameworks (`AppleArchive`, `Compression.framework`) ensure zero IPC serialization overhead and full sandbox compliance (`-DMAS_BUILD`).

### Alternatives Considered
- Spawning CLI subprocesses (`/usr/bin/tar`, `/usr/local/bin/7z`): Rejected due to 10-30ms startup latency and sandbox violations.

### Source
- `Sources/TTZipCore/ArchiveWriter+Dispatch.swift`, `Sources/TTZipCore/ArchiveExtractor+Dispatch.swift`, `Sources/CTTZipBridge/CTTZipBridge_Archive.c`
