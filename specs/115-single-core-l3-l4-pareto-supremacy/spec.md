# Feature Specification: Single-Core L3/L4 Intermediate Pareto Dominance

**Feature Branch**: `115-single-core-l3-l4-pareto-supremacy`
**Created**: 2026-08-19
**Status**: Draft
**Input**: User description: "l3 l4 完全没有超过 而且怎么还变成一个点了，好好分析，需要全面跑到右上角去 /speckit-specify"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Differentiated Intermediate Compression Profiles (Priority: P1)

As a user or software integrator selecting intermediate compression presets (Normal / Level 3 and Maximum / Level 4/6), I want each compression tier to offer distinct, monotonic trade-offs between processing speed and compression ratio, so that no two tiers collapse into identical performance points on the Pareto frontier.

**Why this priority**: Users choosing intermediate compression expect clear gradations in compression strength and throughput. Collapsing Level 3 and Level 4 into identical performance metrics breaks user expectations and distorts the Pareto optimization curve.

**Independent Test**: Can be tested independently by compressing standard datasets (e.g. 100MB `enwik8` and Silesia) across all discrete tiers (Tier 1 through Tier 4) and verifying that both compressed size and compression duration exhibit strictly monotonic differentiation ($\Delta \text{size} > 1.5\%$, $\Delta \text{duration} > 15\%$).

**Acceptance Scenarios**:
1. **Given** uncompressed input data, **When** compressing with Tier 3 (Normal / Level 3), **Then** the compressed output size is measurably smaller than Tier 1/2 ($\ge 15\%$ smaller), with throughput significantly higher than Tier 4 ($\ge 30\%$ faster).
2. **Given** uncompressed input data, **When** compressing with Tier 4 (Maximum / Level 4/6), **Then** the compressed output size is measurably smaller than Tier 3 ($\ge 3\%$ smaller), reaching high space savings while maintaining strong throughput.
3. **Given** the full spectrum of single-threaded compression profiles (Tier 1 to Tier 7), **When** plotted on a throughput vs. space-savings chart, **Then** all points form a strictly separated, monotonic Pareto frontier with zero overlapping or duplicate coordinates.

---

### User Story 2 - Comprehensive Pareto Supremacy Over libdeflate (Priority: P2)

As a high-performance system architect, I want TTZip's single-core intermediate compression engine (Level 3 and Level 4/6) to strictly dominate libdeflate's corresponding levels (Level 3 and Level 6) on the Pareto frontier, achieving higher throughput at identical or superior space savings (occupying the upper-right quadrant of the Pareto efficiency curve).

**Why this priority**: To establish true architectural leadership in single-core compression, TTZip must outperform libdeflate not just on fast levels (Level 1/2), but across the entire middle and maximum compression spectrum.

**Independent Test**: Can be tested independently by benchmarking TTZip Tier 3 vs. libdeflate Level 3, and TTZip Tier 4 vs. libdeflate Level 6 on identical hardware using 100MB `enwik8`, verifying that TTZip achieves higher throughput and equal or greater space savings.

**Acceptance Scenarios**:
1. **Given** 100MB standard text/binary data, **When** comparing TTZip Tier 3 against libdeflate Level 3 in single-thread mode, **Then** TTZip Tier 3 achieves higher throughput ($\ge 1.20\text{ GB/s}$ vs libdeflate $\sim 1.07\text{ GB/s}$) while matching or improving space savings ($\ge 65.5\%$).
2. **Given** 100MB standard text/binary data, **When** comparing TTZip Tier 4 against libdeflate Level 6 in single-thread mode, **Then** TTZip Tier 4 achieves higher throughput ($\ge 850\text{ MB/s}$ vs libdeflate $\sim 749\text{ MB/s}$) while matching or improving space savings ($\ge 66.5\%$).
3. **Given** the resulting Pareto frontier analysis, **When** plotting the Pareto curve, **Then** TTZip data points form a strictly dominant envelope that bounds all competitor data points from above and to the right.

---

### User Story 3 - Deterministic Bit-Stream Fidelity & Multi-Format Round-Trip (Priority: P3)

As a security and data integrity officer, I want all intermediate-level compressed streams to maintain 100% byte-exact round-trip fidelity and seamless compatibility with standard ecosystem decompression tools (`/usr/bin/unzip`, `/usr/bin/gzip`, `zlib`), with zero data corruption across arbitrary file sizes and boundary conditions.

**Why this priority**: Deep search and matchfinder optimizations must never compromise data integrity or standard compliance.

**Independent Test**: Can be tested independently by running automated round-trip compression and decompression across 1,000+ randomized test corpora, validating SHA-256 integrity against original inputs.

**Acceptance Scenarios**:
1. **Given** any payload compressed with Tier 3 or Tier 4, **When** decompressed via system `/usr/bin/unzip` or standard DEFLATE decompressors, **Then** the decompressed stream passes integrity checks and matches the original SHA-256 digest 100%.
2. **Given** edge-case payloads (0-byte, micro 1-byte, highly repetitive, high entropy), **When** processed through intermediate compression levels, **Then** the engine executes safely without buffer overflow, memory leak, or invalid bitstream generation.

---

## Functional Requirements *(mandatory)*

- **FR-001**: The single-core compression system MUST provide distinctly configured match searching and parsing parameters for Tier 3 (Fast Lazy) and Tier 4 (Deep Lazy / Near-Optimal), ensuring non-overlapping operational characteristics.
- **FR-002**: Tier 3 single-core compression MUST achieve $\ge 1.20\text{ GB/s}$ throughput on 100MB standard corpora while delivering $\ge 65.0\%$ space savings in Release mode.
- **FR-003**: Tier 4 single-core compression MUST achieve $\ge 850\text{ MB/s}$ throughput on 100MB standard corpora while delivering $\ge 66.5\%$ space savings in Release mode.
- **FR-004**: The lazy evaluation match finder MUST execute with bounded memory footprints resident in CPU L1/L2 caches, eliminating intermediate full-buffer memory re-allocations.
- **FR-005**: Intermediate compression levels MUST support single-pass or cache-aligned chunked token processing to avoid multi-pass memory bandwidth bottlenecks on large inputs ($\ge 50\text{MB}$).
- **FR-006**: All generated bitstreams MUST be strictly compliant with RFC 1951 (DEFLATE standard) and verified against standard system tools.

---

## Success Criteria *(mandatory)*

- **SC-001**: Single-core Pareto frontier separation: Tier 1, Tier 2, Tier 3, and Tier 4 produce 4 distinct, non-overlapping coordinates on the throughput vs. space-savings Pareto curve.
- **SC-002**: Tier 3 Pareto dominance: TTZip Tier 3 exceeds libdeflate Level 3 in single-core throughput by at least $10.0\%$ ($\ge 1.20\text{ GB/s}$ vs $\sim 1.07\text{ GB/s}$) at equivalent or superior compression ratio on 100MB `enwik8`.
- **SC-003**: Tier 4 Pareto dominance: TTZip Tier 4 exceeds libdeflate Level 6 in single-core throughput by at least $10.0\%$ ($\ge 850\text{ MB/s}$ vs $\sim 749\text{ MB/s}$) at equivalent or superior compression ratio on 100MB `enwik8`.
- **SC-004**: Comprehensive Pareto envelope: The plotted Pareto frontier curve for TTZip single-core engine lies entirely above and to the right of all libdeflate, 7-Zip, and Apple Native benchmark points.
- **SC-005**: 100% Round-trip fidelity: Zero data corruption or checksum mismatches across all single-core oracle tests.

---

## Key Entities *(mandatory)*

- `CompressionProfile`: Configuration object specifying search depth, chain length, nice match length, and Huffman encoding mode for each discrete tier.
- `ParetoPoint`: Benchmark measurement entity containing throughput (MB/s), space savings percentage, compressed size (bytes), and uncompressed size (bytes).
- `ParetoFrontierResult`: Analytical dataset representing the non-dominated optimal boundary across all competing compression engines.
