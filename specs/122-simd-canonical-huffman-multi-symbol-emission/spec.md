# Feature Specification: SIMD Canonical Huffman Coding & Multi-Symbol Emission

**Feature Branch**: `122-simd-canonical-huffman-multi-symbol-emission`
**Created**: 2026-08-19
**Status**: Draft
**Input**: User description: "推进 PR 3 (动态与静态规范 Huffman 树即时构建与多符号发射)"

---

## Clarifications

### Session 2026-08-19
- **Q1: How should match token bitstream emission be accelerated?**
  - **Decision**: Combine length codeword, length extra bits, offset codeword, and offset extra bits into a single packed `uint64_t` word ($< 48$ bits) and emit via a single branchless `ttzip_bs_write_bits64` call instead of 4 separate calls.
- **Q2: How should consecutive literal tokens be handled?**
  - **Decision**: Implement dual-literal pairing: when `token[i]` and `token[i+1]` are both literals, pack both codewords into a single 64-bit integer and write in a single cycle.
- **Q3: When should the engine use static vs. dynamic Huffman coding on small files?**
  - **Decision**: For files $< 4\text{KB}$, evaluate static Huffman bit cost against dynamic header overhead (~300 bits); if static Huffman is smaller or within 2%, emit static Huffman codes directly to eliminate dynamic tree construction latency on small files.

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Packed Multi-Symbol Token Serialization (Priority: P1)

As the Deflate compression pipeline converting LZ77 tokens into bitstreams, I want match tokens and dual-literal pairs to be packed into single 64-bit writes, so that bitstream serialization instruction count is reduced by 4x.

**Why this priority**: In single-core compression, bitstream encoding is executed for every single emitted symbol; packing multiple symbols into 64-bit registers saturates Apple Silicon 8-wide arithmetic pipelines.

**Independent Test**: Can be validated by executing standalone bitstream encoder microbenchmarks across 100K token arrays, asserting encoding throughput $\ge 2,500\text{ MB/s}$.

**Acceptance Scenarios**:
1. **Given** an array of LZ77 match tokens, **When** serialized to bitstream, **Then** all 4 fields of each match token are emitted in a single 64-bit buffer store.
2. **Given** consecutive literal tokens, **When** serialized, **Then** adjacent pairs are merged and written in a single operation.

---

### User Story 2 - Sub-4KB Small-File Static Huffman Fast-Path (Priority: P2)

As an archive engine compressing directories with thousands of small files ($< 4\text{KB}$), I want small files to use precomputed static Huffman codes without dynamic tree reconstruction, so that small-file single-core throughput exceeds $1,000\text{ MB/s}$.

**Why this priority**: Building dynamic Huffman trees on small files introduces latency overhead with negligible compression gain.

**Independent Test**: Validated on the 250MB compound mixed workspace (513 files), measuring Level 1 single-core throughput.

**Acceptance Scenarios**:
1. **Given** files $< 4\text{KB}$, **When** compressed, **Then** static RFC 1951 Huffman coding emits immediately without tree construction.
2. **Given** 513 mixed files in 250MB workspace, **When** compressed at single-core Level 1, **Then** throughput reaches $\ge 800\text{ MB/s}$ (vs 459 MB/s baseline).

---

### Edge Cases

- **Odd Number of Trailing Literals**: Final single literal must emit safely without out-of-bounds array reads.
- **Maximum Codeword Length (15 bits)**: RFC 1951 15-bit constraint must be strictly preserved; combined 4-field match token ($15+5+15+13=48$ bits) must never overflow 64-bit integer registers.
- **Empty / 0-Byte Chunks**: Empty streams must emit clean EOB markers without bitstream corruption.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide `ttzip_bs_write_bits64` accepting up to 56 bits in a single 64-bit accumulator.
- **FR-002**: Match token serialization MUST pack length codeword, length extra bits, offset codeword, and offset extra bits into a single `uint64_t`.
- **FR-003**: Literal serialization MUST pair adjacent literal tokens (`lit0, lit1`) into a single 64-bit write when both tokens have `length == 0`.
- **FR-004**: Small files ($< 4\text{KB}$) MUST evaluate static Huffman encoding to eliminate dynamic tree overhead.
- **FR-005**: All dynamic Huffman headers MUST conform 100% to RFC 1951 section 3.2.7.
- **FR-006**: Generated bitstreams MUST decompress with zero errors using Apple `unzip -t` and libdeflate.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: **Single-Core Bitstream Encoding Throughput**: Bitstream encoding speed on token arrays reaches $\ge 2,500\text{ MB/s}$.
- **SC-002**: **Mixed Workspace Small-File Throughput**: 250MB mixed workspace (513 files) single-core Level 1 throughput increases from 459 MB/s to $\ge 800\text{ MB/s}$.
- **SC-003**: **Zero Bitstream Regression**: 100% decompressibility across all standard test fixtures.
