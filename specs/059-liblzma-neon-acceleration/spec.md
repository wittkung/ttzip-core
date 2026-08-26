# Feature Specification: Liblzma (XZ Utils) ARM NEON Match Finder Acceleration & Upstream Baseline Integration

**Feature Branch**: `059-liblzma-neon-acceleration`

**Created**: 2026-08-17

**Status**: Draft

**Input**: User description: "2. tukaani-project/xz (liblzma 官方上游) 项目仓库：github.com/tukaani-project/xz 定位与生态：LZMA / LZMA2 的全球工业级参考实现，被 Linux 内核、dpkg、rpm、macOS/BSD 系统层、libarchive 及 TTZip 的 Vendor/liblzma.a 全局依赖。开源许可证：Public Domain / LGPL。技术对口点与代码位置：src/liblzma/lz/lz_encoder_mf.c（HC3、HC4、BT4 匹配查找器）。核心痛点：官方 liblzma 在 aarch64 平台上至今依然使用纯 C 标量循环逐字节比对匹配长度，缺乏 ARM NEON 硬件加速。对口算法与机制：将我们自研的 NEON HC4 向量化匹配长度计算作为 aarch64 特化模块引入，能够直接突破全球所有基于 liblzma 的 LZMA2 压缩工具在 ARM 架构下的性能天花板。双平台收益与帮助：macOS：作为 TTZip 静态链接库 Vendor/liblzma.a 的直接上游源码基线，打入 NEON 补丁后直接提升 7Z / XZ / TAR.XZ 在 macOS 上的基础解压缩吞吐。Windows：作为 Windows 端 7Z / XZ 标准解压管道的稳定性与格式合规性基石。详细看看相关内容我们是怎么实现的，这个库又是怎么实现的，比我们真的更快更好吗，我们可以怎么利用 /speckit-specify"

## Clarifications

### Session 2026-08-17
- Q: How should the acceleration be integrated across TTZip and upstream? → A: Two-fold integration: direct patch to `Vendor/xz-upstream/` rebuilding `Vendor/libTTZipVendor.a`, plus isolated atomic patch files for upstream community submission.
- Q: What is the fallback mechanism for non-ARM architectures (x86_64)? → A: Compile-time preprocessor gating (`#if defined(__ARM_NEON)` / `__aarch64__`) ensuring 100% transparent zero-regression fallback to upstream's 64-bit SWAR implementation.
- Q: How to prevent memory over-read in vector comparison? → A: Use strict length bounds checking `len + 16 <= limit`, followed by residual 8-byte SWAR check and scalar byte-level convergence for the final `< 8` bytes.


## User Scenarios & Testing *(mandatory)*

### User Story 1 - High-Throughput LZMA2 & XZ Compression on Apple Silicon (Priority: P1)

As a macOS user archiving large codebases, disk images, or multimedia tarballs into `.7z`, `.xz`, or `.tar.xz` formats, I want the compression engine to leverage hardware SIMD acceleration on ARM64 processors so that file packaging is completed significantly faster without increasing CPU power draw or compromising archive integrity.

**Why this priority**: Core user value of TTZip is maximum performance and native responsiveness. LZMA2 compression is computationally intensive, and match finding constitutes up to 70% of encoder execution time.

**Independent Test**: Compress standard test corpuses (e.g. Silesia, Enwik8, source tarballs) using `.xz` and `.7z` formats, verifying that throughput increases by >= 15% on standard presets while produced archives decompress bit-for-bit identically.

**Acceptance Scenarios**:

1. **Given** uncompressed input data (text, source code, binaries), **When** compressed with 7Z / XZ / TAR.XZ format using standard LZMA2 compression, **Then** compression finishes with verified throughput exceeding baseline, and decompression yields identical SHA-256 / CRC32 checksums.
2. **Given** standard command-line tools (`xz`, `7z`, `tar -xJf`) on external platforms (macOS, Linux, Windows), **When** opening the generated archives, **Then** all standard tools extract the contents without warnings or format incompatibilities.

---

### User Story 2 - Zero-Regression Integration with Vendor Library Foundation (Priority: P2)

As a TTZip maintainer and developer, I want the underlying upstream dependency `Vendor/liblzma.a` to incorporate hardened ARM NEON acceleration while preserving all upstream invariants, zero-cost fast-paths, and MAS sandbox compliance, so that all secondary modules relying on liblzma (such as libarchive TAR.XZ pipelines) automatically benefit from hardware acceleration.

**Why this priority**: Architectural consistency and modularity. Improving the base static library elevates the baseline performance across the entire application ecosystem without duplicating code.

**Independent Test**: Run the full regression test suite (`swift test`) and performance gate suite (`swift test --filter XCTestPerformanceMeasureTests`), ensuring all 525+ tests pass with zero performance regression across all 16 supported archive formats.

**Acceptance Scenarios**:

1. **Given** the full matrix of archive formats (ZIP, 7Z, TAR.XZ, etc.), **When** running full automated regression suites, **Then** all 525+ tests pass and 0 regressions are detected.
2. **Given** memory safety analysis under AddressSanitizer and UndefinedBehaviorSanitizer, **When** executing edge-case compression on small, empty, unaligned, or random byte buffers, **Then** zero memory leaks, buffer over-reads, or undefined behaviors occur.

---

### User Story 3 - Upstream Contribution & Open Source Reference Alignment (Priority: P3)

As an open-source compression engineer, I want the accelerated match finding algorithms formatted as clean, atomic, portable patches ready for upstream submission to `tukaani-project/xz`, adhering to upstream coding conventions (0BSD / Public Domain, C99/C11 cross-platform support, zero regressions on non-ARM architectures).

**Why this priority**: Contributing back to the global upstream ecosystem solidifies TTZip's leadership in high-performance native compression, aligns long-term maintenance overhead, and benefits the entire developer community.

**Independent Test**: Compile and run the complete upstream test suite (`ctest` / `make check`) in `Vendor/xz-upstream` on both ARM64 and x86_64 targets with 100% test pass rate.

**Acceptance Scenarios**:

1. **Given** upstream `tukaani-project/xz` source tree, **When** the acceleration patch is applied, **Then** all native unit tests pass and non-ARM64 platforms build identically with zero regressions.
2. **Given** an ARM64 test runner, **When** running upstream `xz` benchmarks on standard inputs, **Then** match finding and encoding show clear speedups.

---

### Edge Cases

- **Small Buffer / Tail Residuals**: What happens when data length is less than 8 bytes or remaining match length is not an exact multiple of 16 bytes? System must seamlessly handle residuals with safe scalar convergence without out-of-bounds reads.
- **Extreme Repetitive / Dense Zero Patterns**: How does the system handle multi-megabyte sparse / zero sequences? System must leverage early bypass / run-length properties to achieve maximum throughput without stalling inside vector loops.
- **Unaligned Memory Pointers**: What happens when input pointers are unaligned? System must use architecture-safe unaligned vector loads (`vld1q_u8` / unaligned 64-bit access) that are safe on ARM64 and modern processors.
- **Cross-Platform Endianness**: How does the system ensure deterministic output across Big-Endian and Little-Endian architectures? System must use appropriate bit-shift and byte-ordering intrinsics (`__builtin_ctzll` vs `__builtin_clzll`).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST implement hybrid 64-bit register and 128-bit vector match length calculation for LZMA/LZMA2 match finders on ARM64 architecture.
- **FR-002**: The match length calculation MUST guarantee exact parity with reference byte comparison: returning the exact number of identical bytes up to the specified limit.
- **FR-003**: The match finder MUST support Hash Chain (HC3, HC4) and Binary Tree (BT2, BT3, BT4) configurations with 100% format compatibility.
- **FR-004**: The system MUST preserve 100% archive bitstream validity, ensuring any compressed output can be uncompressed by standard tools without discrepancies.
- **FR-005**: The system MUST support hardware-accelerated CRC32 / hashing instructions where available on ARMv8-A platforms.
- **FR-006**: The static vendor library `Vendor/liblzma.a` MUST be buildable with the acceleration enabled, maintaining static link compatibility with TTZip and Mac App Store sandbox rules (`-DMAS_BUILD`).
- **FR-007**: All memory reads MUST strictly adhere to buffer boundaries, preventing buffer over-reads even when operating on trailing chunks.

### Key Entities

- **Match Finder State**: Represents the hash tables, cyclic position buffers, and search parameters used during sliding window dictionary search.
- **Match Length Comparator**: A stateless, high-throughput comparison primitive that evaluates the identical length between current input and candidate match positions.
- **LZMA2 Compressed Stream**: The standard RFC-compliant output stream consisting of raw, uncompressed, or range-coded LZMA chunks with dictionary history.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Match length evaluation throughput on ARM64 reaches >= 4.5 GB/s (representing >= 50% speedup over scalar comparison).
- **SC-002**: Standard LZMA2 / XZ single-threaded compression throughput on ARM64 improves by >= 15% across representative text and binary workloads.
- **SC-003**: 100% of TTZip automated regression tests (525+ test cases) and performance gate checks pass with zero performance regression.
- **SC-004**: Decompressed output bit-for-bit matches original inputs across all test sets (verified by SHA-256 and CRC32).
- **SC-005**: Zero memory leaks or undefined behavior detected across ASan and UBSan runs.

## Assumptions

- **Architecture**: Target deployment is prioritized for Apple Silicon (ARM64) macOS 14.0+, while maintaining full cross-compilation support for Intel (x86_64) and Windows (ARM64/x64).
- **Toolchain**: Built using standard Apple Clang / Xcode toolchain with support for ARM NEON intrinsics (`<arm_neon.h>`) and ARM ACLE CRC32 (`<arm_acle.h>`).
- **License Compliance**: All contributions and upstream patches respect the Public Domain / 0BSD licensing of `tukaani-project/xz`.
