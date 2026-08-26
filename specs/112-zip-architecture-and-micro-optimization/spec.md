# Feature Specification: ZIP Compression Architecture & Micro-Optimization Survey (112-zip-architecture-and-micro-optimization)

## Executive Summary

TTZip's ZIP archiving engine delivers high throughput and high compression ratios on Apple Silicon through a hybrid architecture: Swift high-level task orchestration, direct APFS memory-mapped I/O, POSIX zero-copy storage, multi-block hardware Deflate scheduling, and in-process dynamic Zopfli/libdeflate C engines.

This specification establishes a systematic architectural and micro-optimization blueprint to survey, classify, and optimize all hot paths, memory allocations, CPU vectorization opportunities, and scheduling pipeline stages across the entire ZIP compression stack.

---

## Clarifications

### Session 2026-08-19
- **Q1 (Architecture Focus)**: Should the optimization target single-file multi-block throughput or small-file directory traversal?
  - *Decision*: Both. Single large files utilize L2-cache aligned multi-block concurrency with 32KB dictionary warmup; directory trees with many small files utilize thread-local Deflate state pools and single-pass sequential header streaming.
- **Q2 (Engine Fast-Paths)**: How should we balance generic design patterns with hot-path performance?
  - *Decision*: Strict preservation of inline fast-paths. Design patterns (Strategy, Facade, Template Method) govern configuration and setup, while raw pointers, zero-copy buffers, and SIMD kernels handle the inner hot loop.

---

## User Scenarios & User Stories

### User Story 1 (P1): Zero-Overhead High-Throughput Bulk Compression
As a user archiving large multi-gigabyte files or directories containing hundreds of thousands of small files, I want TTZip to compress at near-hardware SSD saturation rates (>= 5,000 MB/s for Level 1, >= 6,000 MB/s for Store) with zero unnecessary thread synchronization stalls and zero temporary memory page churn.

**Acceptance Scenarios**:
1. Archiving a 10GB single file at Level 1 (`.fast`) maintains >= 5,000 MB/s throughput on Apple Silicon with linear multi-core scaling.
2. Archiving 100,000 small files avoids per-file kernel allocation pauses by leveraging thread-local pre-allocated buffers and zero-copy header construction.
3. Memory consumption remains strictly bounded and deterministic regardless of total archive volume.

---

### User Story 2 (P2): Frontier-Dominating Maximum & Extreme Ratio Compression
As a power user or enterprise archiver seeking maximum space savings (Level 5 ~ 7), I want TTZip's Zopfli, optimal DAG parsing, and near-optimal Deflate engines to execute with hardware-vectorized cost calculation, L2-cache-aware chunking, and cross-block dictionary continuity to achieve maximum space reduction without unbounded compression times.

**Acceptance Scenarios**:
1. Level 6 (`.ultraZopfli`) and Level 7 (`.extremePeak`) achieve >= 97.02% space savings on text/structured datasets (e.g. `enwik8`), outperforming AdvanceCOMP (`advzip -4`) in both compression ratio and throughput.
2. Cross-block 32KB sliding dictionary warmup eliminates block boundary compression penalties across all concurrent threads.
3. ARM NEON vectorization accelerates Huffman bit-cost lookup and literal-length match scoring in the dynamic programming DAG parser.

---

### User Story 3 (P3): Seamless Architecture Layering & Fast-Path Preservation
As a systems engineer or developer maintaining TTZip, I want a clean, decoupled design pattern hierarchy (Facade, Strategy, Template Method, Decorator) that enforces zero dynamic dispatch overhead on I/O and compression hot paths while maintaining 100% Bit-Exact ZipCrypto and WinZip AES-256 compatibility.

**Acceptance Scenarios**:
1. Hot-path compression routines bypass dynamic object tree construction and dynamic dispatch via inlined fast-paths.
2. ZIP local header, data descriptor, central directory, and Zip64 end-of-central-directory structures are written using single-pass sequential layout with zero redundant `lseek` rewrites.
3. AES-256 encryption hot-loops utilize Apple ARM NEON hardware AES instructions (`aese` / `aesmc`) directly in-process.

---

## Functional Requirements

- **FR-001 (Architecture Demarcation)**: The ZIP compression subsystem MUST maintain a strict separation between:
  1. *High-Level Orchestrator* (`ZipArchiver`, `ZipParallelWriter`, `ZipExtremeBlockWriter`): File tree traversal, profile dispatch, thread pool coordination, and multi-volume management.
  2. *Buffer & I/O Plane* (`ZipStoreStreamWriter`, `ZipMemoryEngine`, `ZipDirectIOWriter`): Direct APFS preallocation, memory-mapped pages, zero-copy buffer slicing.
  3. *Core Codec Engine* (`CTTZipBridge`, `native_deflate`, `ttzip_zopfli_engine`, `libdeflate`): C11/POSIX zero-allocation compression kernels.
- **FR-002 (Small-File Hot Path Optimization)**: For archives containing >= 10,000 small files (< 64KB each), the engine MUST eliminate per-entry `malloc`/`free` calls by utilizing thread-local arena memory pools for Deflate state and header serialization.
- **FR-003 (Sequential Single-Pass Zero-Seek Header Streaming)**: When writing uncompressed (Store) or streaming entries, the pipeline MUST generate standard Data Descriptors (`0x08074B50`) and sequential Local File Headers without requiring backward file pointer repositioning (`lseek`), enabling pipe and socket streaming compatibility.
- **FR-004 (L2-Cache Aware Block Slicing)**: For multi-block parallel Deflate, chunk sizes MUST be aligned to the processor L2 cache footprint (2MB ~ 4MB chunks) to maximize CPU cache locality during hash-table match finding.
- **FR-005 (Continuous Sliding Dictionary Warmup)**: In multi-threaded chunked compression, block $k$ ($k > 0$) MUST ingest the final 32KB of uncompressed data from block $k-1$ into its LZ77 sliding window, eliminating block boundary compression degradation.
- **FR-006 (ARM NEON Hardware Acceleration Integration)**: Match finding (Hash4/Hash3 string comparison), Adler32/CRC32 checksumming, and Huffman symbol counting MUST execute with dedicated ARM NEON 128-bit vector intrinsics.
- **FR-007 (ZipCrypto & WinZip AES-256 Zero-Copy In-Place Encryption)**: Encryption layers MUST operate in-place over pre-allocated output buffers without intermediate `Data(bytes:)` memory copies.
- **FR-008 (Zip64 Transparent Scalability)**: The engine MUST automatically and transparently switch to 64-bit offsets and entry counters whenever archive size >= 4GB or total file count >= 65,535 entries, maintaining full PKWARE AppNote spec compliance.

---

## Non-Functional Requirements & Success Criteria

- **SC-001 (Throughput Floor)**: ZIP Level 1 parallel compression throughput MUST exceed **5,000 MB/s** on Apple Silicon M-series (18 cores) for 100MB+ workloads, and Store Direct I/O MUST exceed **6,000 MB/s**.
- **SC-002 (Single-Core Algorithmic Dominance)**: Single-threaded ZIP Level 1 compression MUST exceed **1,400 MB/s**, and Level 6 MUST exceed **800 MB/s**, beating system `ditto` by >= 2.0x.
- **SC-003 (Maximum Space Savings)**: Level 7 (`.extremePeak`) space savings on `enwik8` MUST achieve >= **97.02%**, matching or exceeding AdvanceCOMP `advzip -4`.
- **SC-004 (Zero Memory Leak & Bounded Footprint)**: Peak heap memory allocation during multi-gigabyte compression MUST remain strictly bounded below **64 MB per active worker thread**.
- **SC-005 (Standards & Bit-Exact Verification)**: 100% of generated ZIP archives across all tiers (0 through 7) MUST pass verification by standard `unzip -t`, `7zz t`, and `bsdtar -tvf`.

---

## Key Technical Entities

1. `ZipCompressionProfile`: Strongly-typed parameter configuration struct defining Deflate level, Zopfli iterations, match-finder depth, and block splitting topology.
2. `ZipParallelWriter` / `ZipExtremeBlockWriter`: High-throughput Swift concurrent block orchestrator managing worker pools and sequence re-assembly.
3. `ZipCentralDirectory`: Sequential central directory builder and Zip64 locator generator.
4. `NativeDeflateEngine`: Zero-dependency, thread-local C Deflate codec (`ttzip_deflate_engine`).
5. `ZopfliEngine`: Iterative dynamic Huffman DAG shortest-path optimizer with NEON cost acceleration (`ttzip_zopfli_engine`).

---

## Assumptions & Dependencies

1. **Hardware Target**: Apple Silicon ARM64 / ARM64e (with macOS 14.0+ Sonoma baseline), compatible with Intel x86_64.
2. **Standard Compliance**: PKWARE .ZIP File Format Specification Version 6.3.9 and RFC 1951 (DEFLATE Compressed Data Format Specification).
3. **In-Process Immunity**: Zero external CLI process spawning; 100% in-process static C/Swift runtime bindings.
