# Feature Specification: Block-Splitting & Cost Evaluation

**Feature Branch**: `123-block-splitting-and-cost-evaluation`
**Created**: 2026-08-19
**Status**: Draft
**Input**: User description: "推进 PR 4 (块分割启发式优化与极速动态/静态代价评估)"

---

## Clarifications

### Session 2026-08-19
- **Q1: How should static vs. dynamic Huffman selection be made?**
  - **Decision**: Perform exact bit-cost accounting: calculate `static_bit_cost` vs `dynamic_bit_cost + header_bits`. If static is smaller or within 1%, select static Huffman to save header bytes and avoid decompressor tree construction.
- **Q2: When should large continuous streams be split into multiple RFC 1951 blocks?**
  - **Decision**: Introduce adaptive block splitting at 64KB~128KB chunk boundaries or upon detecting symbol frequency divergence ($\chi^2$ divergence threshold $\ge 35\%$), preserving the 32KB sliding window history.
- **Q3: How to maintain zero heap allocations during block splitting?**
  - **Decision**: Reuse thread-local block state structures with zero runtime `malloc`/`free`.

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Microsecond Static vs. Dynamic Huffman Cost Evaluator (Priority: P1)

As the Deflate compression engine, I want to accurately evaluate the total bit cost of static vs. dynamic Huffman encoding before emitting a block, so that the engine automatically picks the smallest possible bitstream representation.

**Why this priority**: Choosing dynamic Huffman when static is smaller increases file size by 30~50 bytes per block, harming compression ratios on small/medium files.

**Independent Test**: Evaluated via unit tests comparing bit cost predictions against actual emitted bitstream lengths.

**Acceptance Scenarios**:
1. **Given** a token buffer with low symbol variance, **When** evaluated, **Then** the engine chooses the representation with lower bit count.
2. **Given** high-entropy or small chunks, **When** static bit cost $\le$ dynamic bit cost, **Then** static Huffman is chosen with zero dynamic header overhead.

---

### User Story 2 - Adaptive Block Splitting for Multi-Modal Continuous Streams (Priority: P2)

As an archiving engine compressing large files containing mixed data formats (e.g. tarballs containing executable code followed by XML or JSON), I want the engine to split blocks dynamically at entropy change points while retaining the 32KB sliding dictionary, so that compression ratio improves by 2%~5% without throughput loss.

**Why this priority**: Single global Huffman trees over mixed data dilute codeword optimality.

**Independent Test**: Measured on compound mixed corpora asserting compression ratio improvement.

**Acceptance Scenarios**:
1. **Given** a 256KB mixed text + binary payload, **When** compressed, **Then** the engine splits into optimal sub-blocks preserving sliding history.
2. **Given** split blocks, **When** decompressed, **Then** output is 100% bit-exact with the original stream.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST compute `ttzip_eval_static_huffman_bit_cost` in $< 1\mu s$ without memory allocation.
- **FR-002**: System MUST compute `ttzip_eval_dynamic_huffman_bit_cost` including RLE header overhead.
- **FR-003**: If `static_bits <= dynamic_bits`, system MUST emit RFC 1951 BTYPE 01 (Static Huffman) block.
- **FR-004**: System MUST support adaptive block splitting up to 64KB/128KB chunks while preserving 32KB dictionary history across block boundaries.
- **FR-005**: All split blocks MUST decompress seamlessly with standard `libdeflate` and macOS `unzip`.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: **Cost Evaluation Latency**: Bit cost evaluation completes in $< 2.0\mu s$ per 64KB chunk.
- **SC-002**: **Compression Ratio Optimization**: Average compressed size on mixed files decreases by $\ge 1.5\%$.
- **SC-003**: **Zero-Regression Throughput**: Single-core Deflate throughput maintains $\ge 4,500\text{ MB/s}$.
