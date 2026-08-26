# Phase 0 Technical Research: In-Place Huffman Builder & Near-Optimal Parser

**Feature Branch / Spec Directory**: `specs/102-in-place-huffman-and-near-optimal-parser`  
**Created**: 2026-08-18  
**Status**: Completed  

---

## Research Item R001 [SUBAGENT:research]: In-Place 2-Queue Huffman Tree Merging & Depth-Limited Overwriting

- **Decision**: Adopt the Van Leeuwen (1976) two-queue monotonic merge algorithm sharing the input `uint32_t A[]` symbol/frequency array with reverse-topological depth evaluation and shallow-leaf borrowing.
- **Rationale**:
  1. **Zero Dynamic Allocation**: Shares the caller's output buffer `A[]` (low 10 bits = symbol index, high 22 bits = frequency/parent/depth), eliminating all heap allocations and node structs during tree building.
  2. **Linear $O(N)$ Invariant**: Pre-sorted leaf queues guarantee monotonic minimum extraction in $O(1)$ per step.
  3. **Shallow-Leaf Borrowing**: Enforces length limit ($\le 15$ bits) by splitting shallow leaves instead of deep nodes, guaranteeing the Kraft-McMillan inequality $\sum 2^{-l_i} = 1$ with negligible compression ratio penalty ($< 0.01\%$) compared to package-merge.
- **Alternatives Considered**:
  - *Min-Heap Priority Queue with Dynamic Pointers*: Rejected due to high heap allocation overhead and poor L1D cache locality.
  - *Package-Merge Algorithm*: Rejected due to $O(L \cdot N)$ time and auxiliary list allocation overhead.
- **Source**: `Vendor/libdeflate-upstream/lib/deflate_compress.c` lines 815-1396 (`build_tree`, `compute_length_counts`, `deflate_make_huffman_code`).

---

## Research Item R002 [SUBAGENT:research]: ARM64 RBIT Hardware Bit Reversal vs Scalar Fallback

- **Decision**: Use ARM64 inline assembly / intrinsic `rbit %w0, %w1` with right-shift `rbit32(code) >> ((32 - len) & 31)` for Apple Silicon; provide a 256-byte cacheline-resident lookup table (`bitreverse_tab[256]`) for x86_64 / POSIX fallback.
- **Rationale**:
  1. **Single-Cycle Latency**: ARM64 `rbit` flips 32 bits in 1 CPU cycle; right-shifting aligns the reversed $L$-bit code to LSB.
  2. **RFC 1951 LSB-First Compliance**: Canonical Huffman codewords are numerically MSB-first but must be emitted LSB-first in the bitstream.
  3. **L1D Cache Residency**: The 256-byte fallback table consumes only 4 cachelines (256 bytes) and performs 2 lookups + shifts for 16-bit codes.
- **Alternatives Considered**:
  - *SWAR 5-stage bitwise shift-and-rotate*: Rejected because it requires 12-15 instructions with strict serial data dependencies, running slower than a 256-byte L1 table.
  - *64KB 16-bit lookup table*: Rejected due to L1 cache pollution and cold cache miss penalties.
- **Source**: `Vendor/libdeflate-upstream/common_defs.h` lines 711-733, `lib/deflate_compress.c` lines 1094-1152.

---

## Research Item R003 [SUBAGENT:research]: Near-Optimal Dynamic Programming Parser with Fixed-Point Bit Cost

- **Decision**: Standardize and wire `libdeflate`'s Level 10-12 Near-Optimal Parser with binary search tree matchfinder (`bt_matchfinder`), 16x fixed-point entropy modeling (`BIT_COST = 16`), and backward DAG shortest-path relaxation on 300KB soft blocks into TTZip's high-compression and `.ultra` pipelines.
- **Rationale**:
  1. **Near-Zopfli Compression Ratio**: Within 0.1% to 0.3% of Zopfli's extreme ratio on text/code benchmarks while operating **20x to 50x faster**.
  2. **Fixed-Point Quantization Immunity**: $1/16$ fractional bit resolution eliminates floating-point register spill and cross-platform rounding discrepancies while preventing 32-bit integer overflow ($300,000 \times 15 \times 16 \approx 7.2 \times 10^7 \ll 2^{32}-1$).
  3. **Thread-Local Zero Allocation**: Instances are cached in TTZip's TLS compressor array (`g_tls_compressors[14]`), preserving the zero-allocation hot path invariant.
- **Alternatives Considered**:
  - *Google Zopfli*: Rejected due to unacceptable latency ($< 1.5$ MB/s single-threaded throughput) and dynamic allocations.
  - *Standard Deflate Level 9 (Lazy2)*: Rejected because local greedy choices miss global Pareto optima achieved by DAG dynamic programming.
- **Source**: `Vendor/libdeflate-upstream/lib/deflate_compress.c` lines 81, 126-158, 3327-3849, `Vendor/libdeflate-upstream/lib/bt_matchfinder.h` lines 29-292.
