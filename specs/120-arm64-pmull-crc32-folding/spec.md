# Feature Specification: ARM64 PMULL / CRC32 Multi-Way Folding & Cache Fusion Pipeline

**Feature Branch**: `120-arm64-pmull-crc32-folding`
**Created**: 2026-08-19
**Status**: Draft
**Input**: User description: "先推进第一个 pr (硬件 CRC-32 多路折叠与缓存融合管道)"

---

## Clarifications

### Session 2026-08-19
- **Q1: Which ARM64 instruction extensions will be utilized?**
  - **Decision**: Target ARM64 PMULL (polynomial multiplication in `arm_neon.h` / `pmull`/`pmull2`) combined with ARMv8-A CRC32 instructions (`__builtin_arm_crc32cb`, `__builtin_arm_crc32ch`, `__builtin_arm_crc32cw`, `__builtin_arm_crc32cd`), backed by scalar fallback for non-ARM64 platforms.
- **Q2: What is the optimal vector folding stride for Apple Silicon M-series cores?**
  - **Decision**: Implement a 64-byte / 128-byte multi-vector folded reduction stride (utilizing 8~12 128-bit NEON vector registers) to saturate the 8-wide out-of-order execution pipelines and dual crypto execution units on Apple Silicon M1-M4 P-cores.
- **Q3: How is cache fusion achieved for ZIP archive streaming?**
  - **Decision**: Expose `ttzip_crc32_update_fused()` and `ttzip_crc32_pmull_wide()` C interfaces so that Deflate block loaders, chunked writers, and memory-mapped inspectors update the CRC-32 checksum in L1 cache while data is hot, eliminating redundant memory reads.

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Single-Core Ultra-High-Throughput CRC-32 Calculation (Priority: P1)

As a compression engine consumer or archive processor, I want CRC-32 checksumming on Apple Silicon single cores to sustain extreme computational throughput ($\ge 35\text{ GB/s}$ in-cache, $\ge 15\text{ GB/s}$ in RAM) using multi-way PMULL polynomial folding, so that checksum computation never becomes a bottleneck at gigabyte-per-second I/O rates.

**Why this priority**: Checksum computation runs on every byte compressed or decompressed. At high throughput, scalar or simple instruction CRC32 consumes disproportionate CPU time.

**Independent Test**: Can be validated via standalone microbenchmarks across buffer sizes ranging from 1 byte to 100MB, comparing throughput against baseline `libdeflate_crc32` and standard scalar CRC-32.

**Acceptance Scenarios**:
1. **Given** cache-hot input data (1KB to 64KB), **When** CRC-32 is computed on a single Apple Silicon P-core, **Then** throughput reaches $\ge 35\text{ GB/s}$.
2. **Given** large memory buffers (10MB to 100MB), **When** CRC-32 is computed, **Then** memory-bound throughput reaches $\ge 15\text{ GB/s}$ (saturating L2/SLC read bandwidth).
3. **Given** arbitrary pointer alignment (0..63 bytes misalignment), **When** CRC-32 is computed, **Then** calculation runs safely without bus faults or performance cliffs.

---

## User Story 2 - Cache-Fused Checksumming with Zero Second-Pass Overhead (Priority: P2)

As an archive writer or reader processing sequential chunked data streams, I want to calculate the CRC-32 checksum concurrently while buffering or transmitting data, so that memory bandwidth is consumed exactly once.

**Why this priority**: Eliminating second memory passes across large archives preserves L2/L3 cache residency and saves CPU memory bus bandwidth.

**Independent Test**: Can be validated by benchmarking fused chunk processing (copy + CRC32 or Deflate read + CRC32) vs two-pass processing.

**Acceptance Scenarios**:
1. **Given** a chunked data buffer being prepared for Deflate compression, **When** invoking fused CRC-32 calculation, **Then** the combined operation achieves within 5% of pure copy/compression time without separate memory read traversal.

---

## User Story 3 - Deterministic Cross-Ecosystem Bit-Exact Verification (Priority: P3)

As a security and data integrity auditor, I want all CRC-32 results computed via PMULL multi-way folding to match the standard IEEE 802.3 CRC-32 polynomial ($0xEDB88320$) bit-for-bit across all possible input lengths and bit patterns.

**Why this priority**: Any checksum divergence corrupts archive verification across standard tools (`/usr/bin/unzip`, `/usr/bin/gzip`, `7z`).

**Independent Test**: Can be validated by testing against standard CRC-32 test vectors, fuzzing random lengths 0..65536 bytes, and asserting 100% exact equality against `zlib` and `libdeflate` oracles.

**Acceptance Scenarios**:
1. **Given** known ASCII test vectors (e.g. `""`, `"123456789"`, Silesia sample files), **When** computing CRC-32, **Then** results match standard reference values (e.g. `0x00000000`, `0xCBF43926`).
2. **Given** 10,000 random-length fuzz buffers (0..131072 bytes) with random alignments (0..63 bytes), **When** verifying against reference oracle, **Then** error rate is 0.000%.

---

### Edge Cases

- **Zero-Length Buffer (`len == 0`)**: Must immediately return initial `crc` value without dereferencing `buf` (even if `buf` is NULL).
- **Sub-Vector Buffers ($len < 16$ bytes)**: Must execute through ARMv8-A CRC32 scalar instructions (`__builtin_arm_crc32cb/h/w/d`) or slice-by-8 fallback without vector reduction overhead.
- **Unaligned Start Addresses (`buf % 16 != 0`)**: Must safely consume unaligned head bytes before entering the 64-byte / 128-byte folded vector loop.
- **Large Buffers (> 4GB)**: Length parameter must safely handle full 64-bit `size_t` without 32-bit truncation or overflow.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST implement ARM64 PMULL-based multi-way parallel folded polynomial CRC-32 computation supporting both CRC-32 (IEEE 802.3 / gzip / zip polynomial $0xEDB88320$).
- **FR-002**: System MUST utilize 128-bit NEON vector polynomial multiplication (`pmull` / `pmull2`) with at least 4 parallel accumulator streams (64..128 bytes per stride).
- **FR-003**: System MUST execute unaligned head and tail residue processing using ARMv8-A hardware CRC32 instructions (`crc32cb`, `crc32ch`, `crc32cw`, `crc32cd`).
- **FR-004**: System MUST provide strict scalar portable C fallback for non-ARM64 architectures with bit-exact parity.
- **FR-005**: System MUST provide public C interfaces `ttzip_crc32_fast(uint32_t crc, const uint8_t *buf, size_t len)` and `ttzip_crc32_pmull_wide(uint32_t crc, const uint8_t *buf, size_t len)` in `CTTZipCRC32Neon.h`.
- **FR-006**: System MUST pass 100% of differential test vectors against reference `libdeflate_crc32` and `zlib` oracles across all sizes (0..256KB) and alignments.
- **FR-007**: System MUST maintain zero heap allocations (`malloc`/`free`) on the CRC32 calculation path.
- **FR-008**: System MUST compile with zero warnings under `-Wall -Wextra -Werror -Wshadow`.

---

### Key Entities

- **Folding Constants Vector (`mults[k]`)**: Precomputed Galois field polynomial multiplication constants $(x^{N} \bmod G(x))$ representing distance foldings for 64B, 128B, and 192B vector strides.
- **CRC-32 Accumulator**: 32-bit running checksum state matching IEEE 802.3 standard.
- **NEON Vector State (`uint8x16_t v0..v11`)**: 128-bit SIMD registers holding intermediate folded polynomial states.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: **In-Cache CRC-32 Throughput**: Single-core CRC-32 throughput on Apple Silicon M-series P-core for 32KB~64KB buffers achieves $\ge 35.0\text{ GB/s}$.
- **SC-002**: **Main-Memory CRC-32 Throughput**: Single-core CRC-32 throughput for 50MB buffers achieves $\ge 15.0\text{ GB/s}$.
- **SC-003**: **Zero Performance Regression**: 0 test configurations show regression against existing `libdeflate_crc32` baseline.
- **SC-004**: **Bit-Exact Correctness**: 100% pass rate across exhaustive length (0..1024 bytes) and alignment (0..15 bytes) test matrix (16,384 cases).

---

## Assumptions

- Target hardware is macOS 14+ on Apple Silicon (ARMv8-A / ARMv9-A) with PMULL and CRC32 hardware instructions available.
- Portable C fallback implementation satisfies IEEE 802.3 CRC-32 standard on all other architectures.
