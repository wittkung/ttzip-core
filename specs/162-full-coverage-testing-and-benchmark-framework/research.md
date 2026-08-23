# Research Findings: 全覆盖测试与基准遥测零回退体系 (Feature 162)

## R001 [SUBAGENT:research]: Integration of zlib-ng 8-Corpus Architecture and Codec Extension in C11 (`tests/c/bench_codecs.c`)

- **Decision**: Extend `tests/c/bench_codecs.c` to integrate all 8 standard benchmark corpora from `CTTZipCorpusGen.h` (`TTZIP_CORPUS_TEXT`, `TTZIP_CORPUS_SHORT_MATCH`, `TTZIP_CORPUS_DNA`, `TTZIP_CORPUS_RANDOM`, `TTZIP_CORPUS_LITERALS`, `TTZIP_CORPUS_MIXED`, `TTZIP_CORPUS_REALISTIC_RGB`, `TTZIP_CORPUS_STRIPED_RGB`) and add dual-directional (Compress + Decompress) timing and CPB calculation for **LZ4** (Fast L1 / HC L9), **Brotli** (Q6 / Q9), **Bzip2** (L1 / L9), and **Blosc2** (`ttzip_blosclz` L1 / L9) using pre-allocated, zero-heap aligned buffers.

- **Rationale**:
  1. **Zero-Heap Benchmark Stability**: Pre-allocating `raw` (1MB), `comp` (2MB), and `decomp` (1MB) once outside the measurement loop avoids allocator lock contention and memory fragmentation jitter, ensuring nanosecond-accurate throughput and CPB calculations.
  2. **Calibrated Microarchitectural Metrics**: Uses `ttzip_bench_nanos()` (backed by Apple `mach_absolute_time()` or `clock_gettime(CLOCK_MONOTONIC_RAW)`) and `ttzip_calc_cpb(bytes, elapsed_nanos)` with a nominal Apple Silicon baseline frequency of 3.50 GHz to produce deterministic Cycles Per Byte metrics.
  3. **Exact C API Signatures & Buffer Sizing**:
     - **LZ4 Fast (L1)**: `LZ4_compress_fast((const char*)raw, (char*)comp, src_size, comp_capacity, 1)` and `LZ4_decompress_safe((const char*)comp, (char*)decomp, comp_size, src_size)`. Output buffer capacity bounded by `LZ4_compressBound(src_size)`.
     - **LZ4 HC (L9)**: `LZ4_compress_HC((const char*)raw, (char*)comp, src_size, comp_capacity, 9)`.
     - **Brotli (Q6 / Q9)**: `ttzip_brotli_compress(raw, src_size, comp, comp_capacity)` / `ttzip_brotli_decompress(comp, comp_size, decomp, src_size)` via Apple `<compression.h>` (`compression_encode_buffer` / `compression_decode_buffer` with `COMPRESSION_BROTLI`).
     - **Bzip2 (L1 / L9)**: `ttzip_bzip2_compress(raw, src_size, comp, comp_capacity, level)` / `ttzip_bzip2_decompress(comp, comp_size, decomp, src_size)` via `BZ2_bzBuffToBuffCompress` / `BZ2_bzBuffToBuffDecompress`.
     - **Blosc2 / BloscLZ (L1 / L9)**: `ttzip_blosclz_compress(raw, (int)src_size, comp, (int)comp_capacity, clevel, clevel >= 5 ? 14 : 13)` and `ttzip_blosclz_decompress(comp, (int)comp_size, decomp, (int)src_size)`.
  4. **Lossless Roundtrip Assertion**: Verifies `memcmp(raw, decomp, size) == 0` for all codecs across all 8 corpora to prevent silent corruption.

- **Alternatives Considered**: Dynamic buffer reallocation per codec iteration. Rejected because `malloc`/`free` calls pollute L1/L2 data cache lines and introduce POSIX thread-lock latency, distorting CPB and MB/s throughput measurements by 8–15%.

- **Source**:
  - `tests/c/bench_codecs.c:1-165`
  - `Sources/CTTZipBridge/include/CTTZipCorpusGen.h:1-44`
  - `Sources/CTTZipBridge/CTTZipCorpusGen.c:1-253`
  - `Sources/CTTZipBridge/include/CTTZipStreamCoder.h:23-40, 57-93`
  - `Sources/CTTZipBridge/include/ttzip_blosclz.h:1-64`
  - `Vendor/include/lz4.h` & `Vendor/include/lz4hc.h`

---

## R002 [SUBAGENT:research]: Container Format & Extraction Pipeline Benchmark Suite (`tests/c/bench_formats.c`)

- **Decision**: Create `tests/c/bench_formats.c` to benchmark container packaging and extraction pipelines across **ZIP** (Store & Deflate), **TAR.GZ**, **TAR.ZST**, **TAR.BZ2**, **TAR.XZ**, **7Z**, and **UnRAR**, integrating platform-specific Peak RSS memory accounting via `getrusage(RUSAGE_SELF, &usage)` and wall-clock telemetry over an isolated in-memory temporary VFS.

- **Rationale**:
  1. **Direct Bridge Integration**:
     - Packaging uses `ttzip_create_archive_tuned()` and dedicated format routines (`ttzip_create_zip_parallel_c`, `ttzip_create_tar_native_c`, `ttzip_create_tar_zstd_direct_c`, `ttzip_create_7z_native_c`).
     - Extraction uses `ttzip_extract_archive_advanced(archive_path, dest_dir, false, NULL)` which executes hardware-accelerated SIMD/mmap extractors before falling back to libarchive.
  2. **Platform-Calibrated Memory RSS Telemetry**:
     - macOS (`__APPLE__`): `usage.ru_maxrss` is reported in **bytes**, requiring conversion `(double)usage.ru_maxrss / (1024.0 * 1024.0)` to yield MB.
     - Linux (`__linux__`): `usage.ru_maxrss` is reported in **kilobytes**, requiring conversion `(double)usage.ru_maxrss / 1024.0`.
  3. **Zero-Pollution Temporary VFS**:
     - Constructs synthetic multi-file hierarchy in `/tmp/ttzip_bench_vfs_XXXXXX` populated by `ttzip_generate_corpus()`.
     - Measures both packaging MB/s and extraction MB/s, validating extracted content checksums and ensuring cleanup via `rm -rf` / directory unlinking.

- **Alternatives Considered**: Running container benchmarks exclusively in Swift using `XCTest` or `Process.run`. Rejected because Swift runtime ARC overhead, `FileManager` temporary object churn, and IPC pipe buffering introduce non-deterministic overhead that masks raw C11 container decompression and I/O streaming efficiency.

- **Source**:
  - `Sources/CTTZipBridge/CTTZipBridge_Archive.c:53-105, 210-325, 444-511`
  - `Sources/CTTZipBridge/include/CTTZipBridge_Archive.h:25-67`
  - `Sources/CTTZipBridge/ttzip_tar_native.c:1-120`
  - `Sources/CTTZipBridge/ttzip_tar_zstd_direct.c:1-150`
  - `Sources/CTTZipBridge/CTTZipBridge_UnRAR.c:1-41`
  - `tests/c/test_tar_container.c:1-80`
  - `tests/c/bench_stress_vfs.c:1-78`

---

## R003 [SUBAGENT:research]: 5-Gate Zero-Regression Telemetry Protocol & Automation Script (`scripts/run_optimization_gate.sh`)

- **Decision**: Implement `scripts/run_optimization_gate.sh` as a unified 5-stage automated gate runner that sequentially executes:
  - **Gate 1**: Native C11 Microkernel & Unit Test Suites (`./build/ttzip_c_test_runner all` executing 22 C test suites).
  - **Gate 2**: C Microarchitectural PMU, Checksum & Codec Benchmark (`./build/ttzip_benchmark_runner --all`).
  - **Gate 3**: Swift / Native 50-Point Matrix Stability & CV Gate (`swift run ttzip-bench gate` enforcing `medianCv <= 1.50%`).
  - **Gate 4**: 160-Point Compression Delta Engine & Binary Size Audit (`swift run ttzip-bench delta --fail-pct 5.0 --json-out build/delta_report.json`).
  - **Gate 5**: End-to-End CLI I/O and Process Peak RSS Gate (`ttzip-cli` archive roundtrip on synthetic corpus verifying 100% byte integrity and Peak RSS `< 64MB`).

- **Rationale**:
  1. **Layered Defense Strategy**: Covers the execution stack from pure C microkernel invariants (Gate 1 & 2) up to high-level Swift matrix telemetry (Gate 3 & 4) and user-facing CLI binaries (Gate 5).
  2. **Fail-Fast and CI Integration**: Supports `--bail` to stop immediately on any stage failure, `--stage <name>` for targeted verification, and `--json <path>` for structured CI/CD telemetry artifact export.
  3. **Strict Invariant Auditing**: Verifies that performance CV remains stable under local thermal variance and that binary symbol footprints and compression ratios do not suffer regression.

- **Alternatives Considered**: Relying solely on `scripts/local-ci.sh` or `scripts/run_local_ci_gate.sh`. Rejected because existing scripts run only a subset of Swift tests without binding the newly extended C11 benchmark suite (`ttzip_benchmark_runner --all`), multi-level compression delta sweep (`ttzip-bench delta`), and CLI Peak RSS memory bounds into a consolidated gating pipeline.

- **Source**:
  - `scripts/local-ci.sh:1-51`
  - `scripts/run_local_ci_gate.sh:1-238`
  - `scripts/run_delta_audit.sh:1-22`
  - `Sources/TTZipBench/main.swift:17-32, 62-127, 283-509`
  - `Sources/TTZipCore/Audit/CompressionDeltaEngine.swift:43-152`
  - `CMakeLists.txt:174-297`
