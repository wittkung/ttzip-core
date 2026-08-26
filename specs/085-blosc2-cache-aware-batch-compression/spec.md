# Feature Specification: Blosc2 Cache-Aware Batch Compression Pipeline

**Feature Branch**: `085-blosc2-cache-aware-batch-compression`
**Created**: 2026-08-18
**Status**: Ready for Planning
**Input**: User description: "[Blosc/c-blosc2](https://github.com/Blosc/c-blosc2) BSD 3-Clause Mac: C99 / NEON Win: C99 / AVX2. 借鉴其 L1/L2 Cache-Aware 分块模型，优化批量小文件并发打包管道。详细看看相关内容我们是怎么实现的，这个库又是怎么实现的，比我们真的更快更好吗，我们可以怎么利用 /speckit-specify"

---

## Clarifications

### Session 2026-08-18
- Q: How should the small-file threshold and batch chunk unit size be bounded? → A: Files strictly under 64KB are clustered into 128KB–256KB batch work units to match Apple Silicon L1 Data Cache (128KB) and private L2 cache lines; files >=64KB bypass to direct streaming or large-file block parallel paths.
- Q: Which container formats must be accelerated with 100% standard compliance? → A: ZIP, 7Z, and TAR-family (TAR.ZST, TAR.GZ) containers must produce bitstreams extractable by standard system utilities (`/usr/bin/unzip`, `/usr/bin/tar`, `7z`).

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Accelerated Small-File Batch Archiving (Priority: P1)

As a user packaging software projects, source trees, or web assets containing hundreds or thousands of small files (e.g. 500+ files under 64KB), I want TTZip to compress and archive them significantly faster without stalling on thread scheduling overhead or CPU cache misses, so that batch packaging feels instantaneous.

**Why this priority**: Batch small-file compression is the most common real-world archiving bottleneck where traditional per-file threading suffers from CPU cache thrashing, syscall contention, and thread dispatch overhead.

**Independent Test**: Can be tested by archiving a directory containing 500 small files (total size ~2MB–10MB) to `.zip`, `.tar.zst`, and `.7z`, verifying both throughput acceleration and 100% bitstream/format compliance with standard extraction tools (`/usr/bin/unzip`, `/usr/bin/tar`).

**Acceptance Scenarios**:
1. **Given** a directory containing 500 small files (average size 4KB), **When** creating a ZIP archive with standard compression, **Then** packaging completes at a sustained throughput exceeding the performance baseline without errors, and the output archive is fully extractable.
2. **Given** a mixed directory with both small files (<64KB) and large files (>1MB), **When** archiving to ZIP or TAR, **Then** all files are correctly compressed and ordered with verified CRC32 checksums.

---

### User Story 2 - Zero Memory Bloat and Cache-Locality Guarantees (Priority: P2)

As a power user running concurrent compression tasks on Apple Silicon or multi-core x86 machines, I want batch archiving to maintain a strictly bounded memory footprint and preserve L1/L2 CPU cache residency, avoiding memory spikes or thermal throttling.

**Why this priority**: High thread concurrency on small tasks easily pollutes L3 cache and triggers memory fragmentation if buffers are allocated dynamically per file.

**Independent Test**: Can be tested by running batch archiving under memory and CPU profiling, verifying that working buffer allocations are bounded, cache-line aligned, and reused across batch processing units.

**Acceptance Scenarios**:
1. **Given** a batch archiving task of 10,000 files, **When** monitoring peak memory usage, **Then** memory allocation remains stable within pre-calculated bounds and does not grow linearly with file count.

---

### User Story 3 - Full Standard Archive Ecosystem Interoperability (Priority: P3)

As a user distributing generated archives to external systems (macOS Finder, Linux servers, Windows Explorer, WinRAR, 7-Zip), I want all generated archives to strictly adhere to standard container specifications (ZIP PKWARE, TAR POSIX ustar/pax, 7-Zip 0.4), so that third-party tools can extract them with zero errors.

**Why this priority**: Internal chunking and batch scheduling optimizations must never alter or corrupt the external standard archive format.

**Independent Test**: Can be verified by running bidirectional differential oracle tests against native `/usr/bin/unzip`, `tar`, and `7z` utilities.

**Acceptance Scenarios**:
1. **Given** an archive generated with batch cache-aware scheduling, **When** extracted by macOS Archive Utility and system `/usr/bin/unzip`, **Then** all extracted files match the original directory tree, file sizes, and POSIX permissions bit-for-bit.

---

### Edge Cases

- **Zero-Byte Files & Empty Directories**: Empty files and folder markers must be correctly recorded in headers with method 0 (Stored) and 0 CRC32 without triggering batch chunk alignment errors.
- **Deeply Nested Paths & Long Filenames**: Filenames exceeding standard header limits (e.g. 260 chars) must correctly emit Info-ZIP Unicode Path Extra Fields or POSIX pax extended headers.
- **Single Massive File Mixed with Thousands of Tiny Files**: Pipeline must smoothly partition small files into cache-coalesced batches while routing large files to streaming or multi-core block workers without pipeline stalls.
- **Permission Denied / Unreadable Files**: File read errors on individual items during batch pre-reading must cleanly abort or report specific item diagnostics without corrupting adjacent entries in the batch payload arena.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST cluster small files (uncompressed size < 64KB) into cache-aware batch work units (target size 128KB–256KB) to maximize L1/L2 CPU cache residency during compression.
- **FR-002**: The system MUST preserve thread-local compressor state across consecutive files within each batch unit, eliminating per-file compressor creation and destruction overhead.
- **FR-003**: The system MUST allocate cache-line aligned (64-byte / 128-byte aligned) memory arenas for batch payload staging, eliminating heap allocation fragmentation.
- **FR-004**: The system MUST calculate hardware-accelerated CRC32 checksums for each file in the batch using ARM NEON / SSE4.2 SIMD instructions.
- **FR-005**: The system MUST generate standard-compliant ZIP, 7Z, and TAR archives that pass verification against native operating system tools (`/usr/bin/unzip`, `/usr/bin/tar`, `7z`).
- **FR-006**: The system MUST route files exceeding the small-file threshold (>= 64KB) through direct streaming or large-file block-parallel paths, maintaining zero degradation for large-file workloads.
- **FR-007**: The system MUST support standard compression levels (Store, Fastest, Normal, Maximum, Ultra) within the cache-aware batch pipeline.
- **FR-008**: The system MUST safely handle POSIX directory trees, symlinks, file timestamps, and Mac-specific metadata filters (`.DS_Store`, `__MACOSX`) during batch collection.

### Key Entities

- **BatchWorkUnit**: A coalesced bundle of small files aggregating to an L1/L2 cache-optimal payload size (128KB–256KB), assigned to a single worker core for sequential, cache-hot compression.
- **PayloadArena**: A contiguous, cache-line aligned memory buffer pre-allocated for staging uncompressed and compressed byte streams for a batch of files.
- **ArchiveItemMetadata**: Structured metadata describing an individual file entry (relative path, original size, compressed size, CRC32, compression method, header offset).

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Batch small-file archiving throughput for a 500-file corpus (4KB per file) MUST achieve >= 50 MB/s in Debug mode and >= 70 MB/s in Release mode.
- **SC-002**: Processing overhead per small file (context switches and allocation delays) is reduced by at least 30% compared to independent per-file thread dispatch.
- **SC-003**: 100% of generated ZIP, 7Z, and TAR archives pass bidirectional differential extraction tests against native system tools with zero checksum mismatches.
- **SC-004**: Peak memory allocation during batch archiving remains strictly bounded within the pre-calculated payload arena capacity without uncontrolled heap growth.
- **SC-005**: All existing performance invariants across large files, 7Z, TAR.ZST, and LZ4 remain 100% green with zero regression.

---

## Assumptions

- Target operating environment is macOS 14.0+ on Apple Silicon (M1/M2/M3/M4) with x86_64 compatibility.
- Apple Silicon performance cores feature 128KB L1 Data Cache and shared L2 Cache (4MB–16MB cluster), making 128KB–256KB batch slices optimal for hot cache residency.
- Small files are predominantly under 64KB; files larger than 64KB benefit more from direct parallel streams or block chunking.
- Standard archive formats (ZIP, TAR, 7Z) must be strictly maintained; proprietary container headers (like `.b2frame`) are not used for standard archive output.
