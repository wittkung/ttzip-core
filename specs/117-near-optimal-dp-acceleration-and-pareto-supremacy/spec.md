# Feature Specification: Near-Optimal DP Acceleration and Full-Spectrum Pareto Supremacy

**Feature Branch**: `117-near-optimal-dp-acceleration-and-pareto-supremacy`
**Created**: 2026-08-19
**Status**: Draft
**Input**: User description: "想一想进一步优化策略 /speckit-specify"

## Clarifications

### Session 2026-08-19
- Q: What are the primary optimization targets for this stage? → A:
  1. **Tier 4 Near-Optimal DP Acceleration (Level 12)**: Accelerate the forward-pass dynamic programming shortest-path parser from 12 MB/s to 30~50 MB/s via Pareto-optimal match edge pruning and SIMD bit-cost evaluation, bridging the 210x speed cliff between Level 6 (888 MB/s) and Zopfli (1.5 MB/s).
  2. **Tier 2 (Normal) & Tier 3 (Maximum) Matchfinder Vectorization**: Implement dual-candidate 64-bit SWAR hash chain prefiltering in `hc_matchfinder` to push Tier 2 to $\ge 1.25\text{ GB/s}$ and Tier 3 to $\ge 950\text{ MB/s}$.
  3. **Continuous Pareto Curve Guarantee**: Ensure all 8 tiers (Tier 0 to Tier 7) form a strictly monotonic, continuous Pareto envelope dominating all competitors (libdeflate, 7-Zip, Apple ditto/zip, minizip-ng, Apple libcompression) across all compression regimes.

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Near-Optimal DP (Level 12) Forward-Pass Acceleration (Priority: P1)

As a power user seeking maximum single-pass lossless compression, I want TTZip's Tier 4 (Near-Optimal Level 12) to compress 100MB data in under 2.5 seconds (throughput $\ge 40\text{ MB/s}$) while maintaining the extreme 3.02 MB file size, so that I get near-Zopfli density at 30x the speed.

**Why this priority**: Level 12 achieves a state-of-the-art 3.02 MB size on 100MB `enwik8` (beating 7-Zip Ultra's 3.12 MB), but at 12.1 MB/s (7.86 seconds) it represents the main throughput bottleneck on the Pareto curve.

**Independent Test**: Compress 100MB standard corpora with Tier 4 (Level 12) and measure throughput and output size.

**Acceptance Scenarios**:
1. **Given** 100MB `enwik8` uncompressed data, **When** compressed with Tier 4 (Level 12), **Then** throughput is $\ge 35\text{ MB/s}$ and compressed size is $\le 3.05\text{ MB}$.
2. **Given** highly repetitive and non-repetitive mixed inputs, **When** the DAG forward-pass runs, **Then** Pareto edge pruning produces valid RFC 1951 bitstreams with 100% round-trip SHA-256 integrity.

---

### User Story 2 - Intermediate Lazy Matchfinder Vector Acceleration (Priority: P2)

As a user performing high-volume desktop compression, I want Tier 2 (Normal) and Tier 3 (Maximum) to reach $\ge 1.25\text{ GB/s}$ and $\ge 950\text{ MB/s}$ respectively on Apple Silicon, so that normal and maximum archiving finish almost instantaneously.

**Why this priority**: Tier 2 and Tier 3 are the most frequently used presets for daily archiving; pushing their throughput higher solidifies TTZip's lead over libdeflate L3 and L6.

**Independent Test**: Run single-core benchmark `ZipSingleCoreParetoFrontierPkTests` and verify throughput floors.

**Acceptance Scenarios**:
1. **Given** 100MB corpora, **When** compressed with Tier 2 (Normal), **Then** single-core throughput reaches $\ge 1.20\text{ GB/s}$ with size $\le 3.34\text{ MB}$.
2. **Given** 100MB corpora, **When** compressed with Tier 3 (Maximum), **Then** single-core throughput reaches $\ge 950\text{ MB/s}$ with size $\le 3.21\text{ MB}$.

---

### User Story 3 - Full 8-Tier Pareto Envelope Dominance (Priority: P3)

As a developer evaluating compression benchmarks, I want TTZip's 8 distinct tiers (0 through 7) to form a completely unbroken, strictly dominant outer frontier that envelops every competitor at every bitrate.

**Why this priority**: Eliminating gaps or sub-optimal plateaus ensures that TTZip is the Pareto-optimal choice regardless of whether the user prioritizes extreme throughput (15 GB/s), balanced speed (1.2 GB/s), deep ratio (888 MB/s), near-optimal DP (40 MB/s), or extreme Zopfli (2.85 MB).

**Independent Test**: Generate Pareto plot and assert that all 8 TTZip points lie on the computed Pareto convex hull.

**Acceptance Scenarios**:
1. **Given** the 27-algorithm benchmark PK matrix, **When** the Pareto frontier is calculated, **Then** all 8 TTZip tiers are classified as frontier points.
2. **Given** libdeflate, 7-Zip, Apple ditto/zip, minizip-ng, and Apple libcompression, **When** plotted together, **Then** no competitor point lies above or to the right of the TTZip frontier line.

---

## Functional Requirements *(mandatory)*

- **FR-001**: The near-optimal DP engine MUST implement Pareto edge pruning in the forward-pass DAG search, eliminating dominated $(length, offset)$ edges whose incremental bit cost exceeds shorter matches.
- **FR-002**: The near-optimal DP engine MUST use SIMD lookup for Huffman bit cost estimations on literal and distance symbol pairs.
- **FR-003**: The lazy matchfinder in `hc_matchfinder` MUST implement dual-order hash probing to skip non-matching chains before dereferencing memory.
- **FR-004**: All 8 compression profiles in `ZipCompressionProfile` MUST map deterministically to their respective engine levels.
- **FR-005**: All compressed outputs MUST pass byte-exact SHA-256 decompression verification against libdeflate and Apple libcompression.

---

## Success Criteria *(mandatory)*

- **SC-001**: Tier 4 Near-Optimal Speedup: Tier 4 throughput on 100MB `enwik8` reaches $\ge 35\text{ MB/s}$ (a $>2.8\times$ speedup over baseline 12.1 MB/s) while maintaining $\le 3.05\text{ MB}$ file size.
- **SC-002**: Tier 3 Maximum Throughput: Tier 3 throughput reaches $\ge 950\text{ MB/s}$ with $\le 3.21\text{ MB}$ file size.
- **SC-003**: Tier 2 Normal Throughput: Tier 2 throughput reaches $\ge 1.20\text{ GB/s}$ with $\le 3.34\text{ MB}$ file size.
- **SC-004**: Pareto Dominance: 100% of TTZip tiers form the outermost Pareto frontier across the entire spectrum.
- **SC-005**: 100% test suite pass rate with 0 regressions.

---

## Key Entities *(mandatory)*

- `NearOptimalDPGraph`: Forward-pass shortest-path DAG structure tracking lowest cumulative bit-cost paths across sliding blocks.
- `ParetoEdgePruner`: Heuristic subsystem discarding dominated match candidates during dynamic programming state expansion.
- `DualOrderHashProbe`: SIMD-accelerated matchfinder filter checking order-4 and order-5 hash signatures in parallel.
