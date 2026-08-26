# Feature Specification: Libdeflate Architecture Integration & Performance Exploitation

**Feature Branch**: `062-libdeflate-architecture-integration`

**Created**: 2026-08-17

**Status**: Draft

**Input**: User description: "4. ebiggers/libdeflate 项目仓库：github.com/ebiggers/libdeflate 定位与生态：全网性能天花板级别的 DEFLATE/GZIP/ZLIB 独立压缩库，由 Linux 内核加密层维护者 Eric Biggers 主导，纯手写 x86 AVX2/AVX-512 与 ARM NEON Intrinsics。开源许可证：MIT License（完全商用合规）。技术对口点与代码位置：对应目录/文件：lib/arm/matchfinder_impl.h、lib/deflate_compress.c、lib/arm/crc32_impl.h。对口算法与机制：其全内存块（Whole-buffer）零堆分配设计与硬件 CRC32/PMULL 汇编实现，是我们双平台 ZIP / GZIP 核心解压引擎的黄金对标与代码级底座。双平台收益与帮助：macOS：在 Apple Silicon 统一内存架构下实现极致的单核 2.0+ GB/s 解压吞吐。Windows：在 MSVC / Windows x86_64 环境下提供远超标准 zlib 的性能表现，作为双平台 ZIP 高速解压的第一主力。详细看看相关内容我们是怎么实现的，这个库又是怎么实现的，比我们真的更快更好吗，我们可以怎么利用 /speckit-specify"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Extreme In-Memory DEFLATE / ZIP / GZIP Decompression (Priority: P1)

As a user extracting ZIP, GZIP, and TAR.GZ archives containing large numbers of files or large single files, I want instantaneous decompression speed taking full advantage of Apple Silicon NEON and x86 AVX2 SIMD instructions so that multi-gigabyte archives unpack in milliseconds without UI freezes or excessive RAM allocations.

**Why this priority**: DEFLATE is the most ubiquitous compression algorithm in ZIP, GZ, APK, JAR, CBZ, and 7Z streams. Maximizing single-core and multi-threaded decompression throughput is the primary driver of TTZip's performance superiority over standard system utilities.

**Independent Test**: Can be independently verified by unpacking standard ZIP and GZ test corpuses (Silesia, Enwik8, HyperCompressBench) and asserting decompression throughput exceeds the constitutional baseline (>= 7500 MB/s multi-core on Apple Silicon, >= 2000 MB/s single-core).

**Acceptance Scenarios**:
1. **Given** a valid ZIP archive containing standard DEFLATE-compressed items, **When** parallel extraction is executed, **Then** all entries are decompressed using thread-local whole-buffer decompressor instances with 100% byte-for-byte fidelity and zero per-file heap allocation.
2. **Given** an encrypted or standard ZIP stream, **When** passing compressed payloads to the decompressor, **Then** CRC-32 checksums are computed via hardware SIMD/PMULL instructions with zero CPU pipeline stalls.

---

### User Story 2 - Chunked Streaming DEFLATE Compression with Constant Memory (Priority: P2)

As a user creating ZIP or GZIP archives from multi-gigabyte single files, I want multi-threaded chunked DEFLATE compression that maintains standard ZIP RFC 1951 compatibility while strictly bounding process resident memory within <= 64MB.

**Why this priority**: Whole-buffer compression cannot load a 50GB file into RAM at once. Chunked compression bridges the gap between libdeflate's raw block speed and stream-first memory boundedness.

**Independent Test**: Can be tested by streaming a 1GB+ synthetic payload through `ChunkedDeflateStreamWriter` and verifying that the resulting ZIP/GZ is valid according to `/usr/bin/unzip` and `/usr/bin/gzip`, while resident memory stays under 64MB.

**Acceptance Scenarios**:
1. **Given** an uncompressed input stream of arbitrary size, **When** streamed through 1MB chunked compression slots, **Then** all chunks are compressed concurrently across available CPU cores with BFINAL bit tagging and byte-alignment padding.
2. **Given** completed compressed chunks out of order, **When** flushing to disk, **Then** chunks are written sequentially with incremental running CRC-32 correctly calculated.

---

### User Story 3 - Cross-Platform (macOS Apple Silicon & Windows x86_64) Hardware Parity (Priority: P3)

As a cross-platform developer and user on macOS or Windows, I want identical high-performance DEFLATE and CRC32 acceleration on both Apple Silicon (ARM64 PMULL / NEON) and x86_64 (AVX2 / PCLMULQDQ) architectures without requiring external CLI dependencies.

**Why this priority**: TTZip delivers standalone in-process performance across platforms without depending on OS-specific shell utilities.

**Independent Test**: Can be tested by building and running the test suite on both macOS ARM64 and Windows x86_64 architectures and verifying all roundtrip and throughput tests pass.

**Acceptance Scenarios**:
1. **Given** execution on an Apple Silicon system, **When** computing CRC-32 or decompressing DEFLATE, **Then** hardware NEON and PMULL assembly paths are engaged directly.
2. **Given** execution on an x86_64 system, **When** computing CRC-32 or compressing DEFLATE, **Then** AVX2/PCLMULQDQ fast paths are engaged seamlessly.

---

### Edge Cases

- **Zero-byte input files**: Compressor and decompressor must safely return 0/empty buffer without null pointer dereferences.
- **Corrupted DEFLATE bitstreams**: Decompressor must return explicit error codes (`LIBDEFLATE_BAD_DATA`) rather than crashing or overflowing memory buffers.
- **Incompressible random data**: Chunked and whole-buffer compressors must handle negative compression expansion gracefully using fallback STORE blocks or bounds checking.
- **Single huge file exceeding 4GB**: ZIP64 headers and chunked stream writers must seamlessly handle 64-bit offsets and sizes without 32-bit truncation.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide in-memory zero-heap-allocation DEFLATE decompression via Thread-Local decompressor reuse.
- **FR-002**: System MUST provide Thread-Local DEFLATE compressor instances for levels 1 through 12.
- **FR-003**: System MUST execute CRC-32 checksum calculations using CPU hardware instructions (ARMv8 PMULL/CRC32 on Apple Silicon, PCLMULQDQ on x86_64).
- **FR-004**: System MUST support multi-threaded chunked DEFLATE streaming for large files with RFC 1951 bit-alignment padding and bounded memory usage (<= 64MB).
- **FR-005**: System MUST validate all uncompressed outputs against expected uncompressed sizes and CRC-32 checksums before committing extracted files to disk.
- **FR-006**: System MUST maintain 100% interoperability with standard system decompressors (`/usr/bin/unzip`, `/usr/bin/gzip`, `7z`, `WinRAR`).
- **FR-007**: System MUST provide Swift-native `LibdeflateAccelerator` and `LibdeflateCAdapter` with Flyweight memory page reuse.

### Key Entities

- **DeflateCompressorPool**: Thread-local storage pool managing `libdeflate_compressor` instances across compression levels 1-12 without per-task allocation overhead.
- **DeflateDecompressorPool**: Thread-local storage pool managing `libdeflate_decompressor` instances for instantaneous block decompression.
- **ChunkedDeflateStreamState**: Bounded multi-threaded ring buffer managing in-flight 1MB compression chunks, sequence reordering, and sequential disk output.
- **HardwareChecksumEngine**: Hardware-accelerated CRC-32/Adler-32 pipeline utilizing ARM NEON/PMULL and x86 SIMD extensions.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Whole-buffer DEFLATE decompression throughput exceeds 2,000 MB/s on a single Apple Silicon performance core.
- **SC-002**: Parallel multi-core ZIP decompression throughput exceeds 7,500 MB/s (Debug) / 10,000 MB/s (Release) on 10MB+ corpuses.
- **SC-003**: Hardware CRC-32 checksum throughput exceeds 15,000 MB/s on Apple Silicon.
- **SC-004**: Chunked DEFLATE streaming memory footprint remains strictly <= 64MB regardless of input file size (even for 100GB+ streams).
- **SC-005**: 100% of generated ZIP and GZ files pass verification by standard system tools (`unzip -t` and `gzip -t`).

## Assumptions

- Target environments are macOS 14.0+ (ARM64 / x86_64) and Windows 10/11 (x86_64 / ARM64).
- `libdeflate` C library (v1.22+) is compiled as an in-process static library and linked directly via `CTTZipBridge`.
- Swift calling layers interact via `CUnsafeBufferAdapter` and `MemoryPageFlyweightPool` to avoid memory copying and heap allocations.
