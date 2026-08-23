# Feature Specification: ZIP Tier Rebalancing and Intermediate Pareto Frontier Bridge

**Feature Branch**: `114-zip-tier-rebalance-and-intermediate-pareto`  
**Created**: 2026-08-19  
**Status**: Draft  
**Input**: User description: "那感觉没必要保留 l2 了，整体后移 l3 变成新 l2 l4 变成新 l3 然后我希望能在现在的 l4 和 l5 中间再找一个帕累托前沿点"

---

## Executive Summary

This feature eliminates the algorithmic redundancy between legacy Tier 1 and Tier 2, rebalances the 8 golden standard compression profiles (Tiers 0 to 7), and introduces a high-value intermediate compression tier bridging the historical 210x throughput gap between Tier 3 (Maximum: 3.23 MB @ 4.28 GB/s) and Tier 5 (Graph Fast: 2.87 MB @ 20.4 MB/s).

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Elimination of Redundant Tier 2 & Profile Re-indexing (Priority: P1)

As a macOS user or developer, I want each TTZip compression tier to represent a distinct, non-overlapping point on the physical Pareto frontier so that I can choose compression levels with predictable, distinct speed vs. size trade-offs.

**Why this priority**: Eliminates user confusion from redundant presets that yield identical compressed payloads.

**Independent Test**: Execute single-core and multi-core Pareto benchmark tests and assert that each tier generates unique output sizes and monotonically increasing compression ratios.

**Acceptance Scenarios**:
1. **Given** 100MB Wikipedia text corpus, **When** compressed with Tier 1 (Fast) vs. Tier 2 (Normal), **Then** Tier 2 achieves strictly smaller compressed size (~3.38 MB vs. ~3.97 MB) with over 5.0 GB/s multi-core throughput.
2. **Given** all 8 compression profiles, **When** inspected via CLI or App UI, **Then** Tiers 0..7 are continuous, non-overlapping, and monotonically structured.

---

### User Story 2 - Intermediate Pareto Bridge Tier (Priority: P1)

As a power user seeking high compression without extreme multi-second delays, I want an intermediate compression tier (new Tier 4) that delivers ~3.02 MB to 3.12 MB payload size within 200ms to 500ms for 100MB datasets.

**Why this priority**: Bridges the 210x speed drop between Tier 3 (21ms) and Tier 5 (4.6s), providing a sweet-spot preset for everyday high-ratio archiving.

**Independent Test**: Compress 100MB corpus with new Tier 4 and verify payload size is between 3.00 MB and 3.15 MB with multi-core throughput >= 150 MB/s.

**Acceptance Scenarios**:
1. **Given** 100MB payload, **When** compressed with new Tier 4, **Then** output size is <= 3.15 MB and elapsed time is <= 0.65s (multi-core throughput >= 150 MB/s).
2. **Given** Pareto frontier plot generation, **When** multi-core benchmark is rendered, **Then** new Tier 4 occupies the optimal Pareto frontier point between Tier 3 and Tier 5.

---

### User Story 3 - Single-Core and Multi-Core Benchmark Alignment (Priority: P2)

As a performance engineer, I want the Pareto benchmark test harness (`ZipSingleCoreParetoFrontierPkTests` and `ZipMultiCoreParetoFrontierPkTests`) to reflect the new 8-tier hierarchy with real-time `TestTerminalRenderer` streaming output.

**Why this priority**: Ensures continuous automated CI regression verification with clear terminal visualization.

**Independent Test**: Run `swift test --filter ZipSingleCoreParetoFrontierPkTests` and verify 8 distinct tiers with 0 failures.

**Acceptance Scenarios**:
1. **Given** benchmark execution, **When** running PK suite, **Then** terminal outputs aligned CTest/zlib-ng style rows with distinct throughput and sizes for all 8 tiers.

---

## Edge Cases

- **Zero-length & Micro Files (< 32 KB)**: Tier 4 must safely compress small buffers without incurring thread coordination overhead.
- **Incompressible Data (High Entropy)**: Tier 4 must properly detect entropy saturation and emit dynamic or stored blocks without data expansion.
- **Corpus Scaling (1MB to 500MB)**: The Pareto curve must maintain strict convexity across all file sizes.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST configure the 8 golden standard compression profiles as follows:
  - **Tier 0**: Store (0) — Zero-compression memory direct write (>= 6000 MB/s)
  - **Tier 1**: Fast (1) — Lightweight fast greedy match finder (~3.97 MB, >= 5000 MB/s)
  - **Tier 2**: Normal (2) — Lazy evaluation match finder (~3.38 MB, >= 4500 MB/s)
  - **Tier 3**: Maximum (3) — Deep sliding window Deflate matcher (~3.23 MB, >= 3500 MB/s)
  - **Tier 4**: High Compression (4) — Optimal block partitioned Deflate parser (~3.02..3.12 MB, >= 150 MB/s)
  - **Tier 5**: Graph Fast (5) — 2-pass iterative graph-theoretic shortest-path parser (~2.87 MB, >= 20 MB/s)
  - **Tier 6**: Ultra Zopfli (6) — 5-pass iterative entropy-convergent parser (~2.86 MB, >= 5 MB/s)
  - **Tier 7**: Extreme Peak (7) — 15-pass block-split entropy peak parser (~2.82 MB, >= 1.5 MB/s)
- **FR-002**: System MUST update `ZipCompressionProfile.swift` to reflect the rebalanced presets and target throughput floors.
- **FR-003**: System MUST update `ArchiveWriter+Dispatch.swift` and `ZipExtremeBlockWriter.swift` to route new Tier 4 to the optimal high-compression Deflate coder.
- **FR-004**: System MUST update `ZipSingleCoreParetoFrontierPkTests.swift` and `ZipMultiCoreParetoFrontierPkTests.swift` to evaluate the new 8-tier hierarchy.
- **FR-005**: All benchmark executions MUST stream progress in real time via `TestTerminalRenderer` and `TestLogger`.

---

## Success Criteria *(mandatory)*

1. **Monotonicity**: Compressed payload sizes on 100MB enwik8 must be strictly monotonic: $S_0 > S_1 > S_2 > S_3 > S_4 > S_5 > S_6 > S_7$.
2. **Gap Closure**: New Tier 4 achieves between 3.02 MB and 3.12 MB payload size at >= 150 MB/s (18-core), closing the historical 210x throughput cliff.
3. **Pareto Dominance**: In multi-core 18-thread execution, TTZip Tiers 0..7 strictly dominate all competitor baselines (pigz, minizip-ng, Apple zip/ditto).
4. **Zero Regression**: 100% of existing unit tests and performance gates pass (`exit code 0`).
