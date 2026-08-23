# Feature Specification: libdeflate-Aligned Single-Core DEFLATE Engine with Apple Silicon Optimization

**Feature Branch**: `116-libdeflate-aligned-core-and-apple-silicon-supremacy`
**Created**: 2026-08-19
**Status**: Draft
**Input**: User description: "对的，我们好好的先把他们的那个代码直接就原原本本可以复制过来，对吧？类似的，然后我们再一步一步去优化 /speckit-specify"

## Clarifications

### Session 2026-08-19
- Q: What is the core architectural strategy? → A: Adopt the full canonical algorithmic architecture of `libdeflate` (16-bit relative index `hc_matchfinder`, 8-byte `deflate_sequence` representations, dynamic entropy-based block splitting, and fused 64-bit codeword bitstream emission) as the baseline foundation, and then incrementally apply Apple Silicon ARM64 hardware optimizations (NEON 4-way vector hashing, SWAR mismatch filtering, and multi-port load unrolling) to strictly dominate libdeflate on the Pareto frontier.
- Q: Which compression levels are in scope? → A: Single-threaded Levels 1 through 9 (with primary focus on Tier 3 / Level 3 and Tier 4 / Level 6 intermediate Pareto leadership).
- Q: What are the target metrics? → A:
  - Tier 3: Throughput $\ge 1.20\text{ GB/s}$, Compressed Size $\le 3.34\text{ MB}$ on 100MB `enwik8` (beating libdeflate L3 @ 1.07 GB/s | 3.34 MB).
  - Tier 4: Throughput $\ge 800\text{ MB/s}$, Compressed Size $\le 3.21\text{ MB}$ on 100MB `enwik8` (beating libdeflate L6 @ 722 MB/s | 3.21 MB).

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Canonical Baseline Alignment & Perfect Space Parity (Priority: P1)

As a systems developer using TTZip's native single-core DEFLATE engine, I want the core compression pipeline to achieve exact compression ratio and space savings parity with libdeflate across all levels (Level 1 to Level 9), ensuring our match finding, block splitting, and sequence encoding match the industry gold standard.

**Why this priority**: Without matching libdeflate's compression ratio baseline on intermediate levels (3.34 MB on L3, 3.21 MB on L6 for 100MB `enwik8`), throughput speedups cannot establish upper-right Pareto dominance.

**Independent Test**: Can be tested independently by compressing standard datasets (100MB `enwik8`, Silesia) and verifying that TTZip achieves $\le$ compressed bytes compared to libdeflate at identical levels.

**Acceptance Scenarios**:
1. **Given** 100MB `enwik8` uncompressed data, **When** compressed with Tier 3 (Level 3), **Then** compressed size is $\le 3.34\text{ MB}$ (matching or beating libdeflate Level 3).
2. **Given** 100MB `enwik8` uncompressed data, **When** compressed with Tier 4 (Level 6), **Then** compressed size is $\le 3.21\text{ MB}$ (matching or beating libdeflate Level 6).
3. **Given** arbitrary input data, **When** compressed with the aligned engine, **Then** all output bitstreams decompress with 100% byte-exact SHA-256 integrity.

---

### User Story 2 - Apple Silicon Vectorized & Multi-Port Acceleration (Priority: P2)

As an Apple Silicon macOS user, I want the aligned DEFLATE engine to leverage ARM64 NEON vector extensions, 64-bit GPR SWAR bit-manipulation, and 3-way L1D load-port concurrency to execute faster than libdeflate on Apple Silicon M-series hardware, occupying the dominant upper-right envelope on the Pareto frontier.

**Why this priority**: Hardware-specific vectorization and microarchitectural unrolling unlock the remaining performance headroom on Apple Silicon, pushing throughput beyond libdeflate's portable C code.

**Independent Test**: Can be tested independently by running single-core benchmarks on Apple Silicon hardware and asserting that TTZip throughput exceeds libdeflate throughput by $\ge 10\%$ across all levels.

**Acceptance Scenarios**:
1. **Given** 100MB standard text/binary corpora, **When** compressing with Tier 3 on Apple Silicon, **Then** single-core throughput reaches $\ge 1.20\text{ GB/s}$ (vs libdeflate $\sim 1.07\text{ GB/s}$).
2. **Given** 100MB standard text/binary corpora, **When** compressing with Tier 4 on Apple Silicon, **Then** single-core throughput reaches $\ge 800\text{ MB/s}$ (vs libdeflate $\sim 722\text{ MB/s}$).
3. **Given** the full single-core Pareto frontier curve, **When** plotted against all competitors, **Then** TTZip forms a strictly dominant upper-right envelope.

---

## Functional Requirements *(mandatory)*

- **FR-001**: The engine MUST adopt the 16-bit relative index match finder architecture (`mf_pos_t`) bounded within 256KB state memory to ensure L1/L2 cache residency.
- **FR-002**: The engine MUST represent parsed LZ77 tokens as compact 8-byte `deflate_sequence` records (`litrun_len`, `match_len`, `match_offset`, `next_hashes`).
- **FR-003**: The engine MUST implement dynamic entropy-guided block splitting to detect optimal block boundary phase shifts on large inputs.
- **FR-004**: The engine MUST support fused 64-bit codeword bitstream emission combining length and offset tokens in a single instruction sequence.
- **FR-005**: The engine MUST incorporate ARM64 NEON vector string matching and dual-anchor SWAR mismatch filtering on Apple Silicon targets.
- **FR-006**: All generated bitstreams MUST be 100% RFC 1951 compliant and verified against system `/usr/bin/unzip`.

---

## Success Criteria *(mandatory)*

- **SC-001**: Level 3 Pareto Dominance: TTZip Tier 3 achieves $\ge 1.20\text{ GB/s}$ throughput and $\le 3.34\text{ MB}$ compressed size on 100MB `enwik8` (strictly dominating libdeflate L3).
- **SC-002**: Level 6 Pareto Dominance: TTZip Tier 4 achieves $\ge 800\text{ MB/s}$ throughput and $\le 3.21\text{ MB}$ compressed size on 100MB `enwik8` (strictly dominating libdeflate L6).
- **SC-003**: Level 1/2 Preservation: TTZip Tier 1 and Tier 2 maintain $\ge 1.70\text{ GB/s}$ extreme throughput.
- **SC-004**: 100% Round-trip fidelity across all single-core oracle tests.

---

## Key Entities *(mandatory)*

- `CanonicalDeflateSequence`: 8-byte intermediate record encoding a literal run followed by a match pair.
- `Compact16BitMatchFinder`: 256KB sliding window match finder using 16-bit relative table offsets.
- `DynamicBlockSplitter`: Subsystem estimating Shannon entropy derivatives to split blocks at optimal boundary points.
