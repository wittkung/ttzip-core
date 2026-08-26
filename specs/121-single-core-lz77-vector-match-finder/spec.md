# Feature Specification: Single-Core LZ77 Vector Match Finder & AArch64 SIMD Optimization

**Feature Branch**: `121-single-core-lz77-vector-match-finder`
**Created**: 2026-08-19
**Status**: Draft
**Input**: User description: "开始 pr2 (单核 LZ77 匹配查找与 AArch64 向量化比对)"

---

## Clarifications

### Session 2026-08-19
- **Q1: What is the target L1 D-cache footprint for the Tier 1 match finder table on Apple Silicon?**
  - **Decision**: Restrict the Tier 1 Fast match finder hash table footprint to $\le 64\text{KB}$ (8,192 direct pointer entries or 4,096 2-way entries) so it fits entirely within Apple Silicon's 128KB L1 Data Cache per P-core, eliminating L2 cache misses during hash candidate lookups.
- **Q2: Which vector length comparison kernel will be utilized?**
  - **Decision**: Integrate the Pareto-optimal zero-reduction `compare256` AArch64 kernel (using dual 64-bit SWAR for 0..15 bytes and 128-bit NEON `vceqq_u8` for 16..258 bytes) to eliminate horizontal reduction stalls (`UMAXV`) on early mismatches.
- **Q3: How does the engine detect incompressible data early?**
  - **Decision**: Perform microsecond Shannon entropy / match frequency sampling across the first 256B~1024B; if match rate $< 1.0\%$, immediately bypass match evaluation and emit uncompressed / literal tokens at memory-bus speeds.

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - L1-Cache-Resident High-Throughput Match Finding (Priority: P1)

As a compression engine executing single-threaded Deflate Tier 1 compression, I want the LZ77 match finder table to operate entirely within the L1 data cache ($\le 64\text{KB}$), so that candidate lookup latency is minimized (< 1.2 ns per probe) with zero L2 cache thrashing.

**Why this priority**: In Fast compression tiers, hash lookup latency dominates CPU time. Fitting the working state in L1 cache provides immediate throughput gains across all data types.

**Independent Test**: Can be validated via standalone match finder microbenchmarks measuring candidate probe latency and throughput across Silesia, Enwik8, and Calgary corpora.

**Acceptance Scenarios**:
1. **Given** uncompressed input data streams, **When** executing Tier 1 greedy match finding, **Then** match finding throughput reaches $\ge 2,200\text{ MB/s}$ on Apple Silicon single cores.
2. **Given** continuous 32KB sliding window history, **When** looking up candidate matches, **Then** 100% of candidate table accesses hit in L1 cache with 0 memory stalls.

---

### User Story 2 - Zero-Regression Vectorized Match Length Evaluation (Priority: P2)

As a match finder evaluating match candidates at position $P$, I want the byte-by-byte length comparison to determine the exact match length (3..258 bytes) in minimal clock cycles using AArch64 vector intrinsics without horizontal reduction stalls.

**Why this priority**: Match candidate evaluation is invoked on every byte of compressible data. Short mismatches (0..15 bytes) represent $> 70\%$ of candidate probes; any latency overhead here degrades throughput.

**Independent Test**: Can be validated by testing match lengths 0..258 bytes across all 16 memory misalignment combinations (0..15 bytes), asserting bit-exact match length calculation.

**Acceptance Scenarios**:
1. **Given** two memory buffers matching for $K$ bytes ($0 \le K \le 258$), **When** vector match comparison is executed, **Then** it returns exactly $K$ with 0 bit errors.
2. **Given** short mismatches ($K < 8$), **When** evaluated, **Then** comparison terminates in $\le 0.75\text{ ns}$ using 64-bit SWAR arithmetic.

---

### User Story 3 - Early Entropy Short-Circuiting on Incompressible Payloads (Priority: P3)

As an archive creator processing pre-compressed or high-entropy files (e.g. `.jpg`, `.mp4`, `.enc`), I want the match finder to detect high entropy within the first 1KB and short-circuit directly to literal emission, so that incompressible files do not stall the CPU pipeline.

**Why this priority**: Compressing incompressible data with full LZ77 search wastes CPU energy with zero compression benefit.

**Independent Test**: Can be validated by benchmarking incompressible random and media payloads, asserting throughput $\ge 4,500\text{ MB/s}$.

**Acceptance Scenarios**:
1. **Given** uniformly distributed random or encrypted bytes, **When** processed by the match finder, **Then** early entropy short-circuiting triggers and processes data at $\ge 4,500\text{ MB/s}$.

---

### Edge Cases

- **Sub-4-Byte Inputs ($len < 4$)**: Must emit raw literals immediately without attempting 4-byte hash lookups or out-of-bounds reads.
- **Unaligned Input Pointers**: Memory buffers with arbitrary alignment (0..63 bytes modulo 64) must execute without alignment faults or bus stalls.
- **Uniform / Monolithic Repeating Payloads (e.g. 10MB of 0x00)**: Match finder must saturate maximum match length (258 bytes) at line-rate memory write speed without integer overflow.
- **Distant Match Boundaries ($dist > 32768$)**: Matches beyond the RFC 1951 32KB window must be strictly rejected.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Single-core match finder MUST maintain total hash table memory footprint $\le 64\text{KB}$ for Tier 1 Fast profile.
- **FR-002**: Match finder MUST implement fast 4-byte multiplicative hashing (`ttzip_hash4`) using single-cycle integer arithmetic.
- **FR-003**: Match length comparison MUST utilize 64-bit SWAR for short lengths (0..15 bytes) and 128-bit NEON vector comparisons for long matches (16..258 bytes).
- **FR-004**: Match finder MUST support 32KB cross-block history seeding when contiguous history is available in virtual memory.
- **FR-005**: Match finder MUST validate that all emitted match distances are strictly within `[1, 32768]` and match lengths are within `[3, 258]`.
- **FR-006**: Match finder MUST populate token outputs and symbol frequency histograms (`ttzip_symbol_freqs_t`) with zero intermediate dynamic memory allocations.
- **FR-007**: Match finder MUST provide 100% C11 portable scalar fallback for non-ARM64 architectures.
- **FR-008**: System MUST compile with zero warnings under `-Wall -Wextra -Werror -Wshadow`.

---

### Key Entities

- **Fast Match Finder Context (`ttzip_deflate_fast_mf_t`)**: 64-byte aligned structure containing the 64KB L1-cache hash table.
- **Deflate Token (`ttzip_deflate_token_t`)**: Packed 32-bit structure containing 16-bit length (0 for literal, 3..258 for match) and 16-bit offset/literal value.
- **Symbol Frequencies (`ttzip_symbol_freqs_t`)**: Frequency accumulator array for 288 literal/length symbols and 32 distance symbols.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: **Tier 1 Single-Core Match Finder Throughput**: Match finding throughput on Apple Silicon P-cores reaches $\ge 2,200\text{ MB/s}$ on Silesia text corpus.
- **SC-002**: **L1 D-Cache Residency**: Hash table size is $\le 64\text{KB}$, ensuring 0 L2 cache miss penalties for Tier 1 lookups.
- **SC-003**: **Bit-Exact Correctness**: 100% pass rate across exhaustive length (0..258 bytes) and alignment (0..15 bytes) differential tests.
- **SC-004**: **Zero Performance Regression**: Full matrix benchmark confirms 0 regressions across all existing standard format tests.

---

## Assumptions

- Target environment is macOS 14+ on Apple Silicon (ARM64) with C11 scalar fallback for generic platforms.
- Input data buffers have at least 8 bytes of readable padding at the buffer tail for safe unaligned word loads.
