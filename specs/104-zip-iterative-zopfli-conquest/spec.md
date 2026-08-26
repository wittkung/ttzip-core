# Feature Specification: 104-zip-iterative-zopfli-conquest

## Feature Name
**In-Process Multi-Pass Iterative Zopfli & AdvanceCOMP Conquest Engine**

## Motivation & Background
In the previous benchmarks, TTZip achieved Pareto dominance in Tiers 0 through 5 ($6.7\text{ GB/s}$ down to $470\text{ MB/s}$). However, for the extreme compression frontier:
- `libdeflate 12` executes a single-pass near-optimal DAG match search ($150\text{ MB/s}$ @ $3.03\text{ MB}$). While fast, it does not iteratively re-weight dynamic Huffman symbol frequencies.
- External competitors `pigz -11 (Zopfli)` ($3.01\text{ MB}$ @ $3.02\text{ MB/s}$) and `AdvanceCOMP (advzip -4)` ($2.99\text{ MB}$ @ $0.71\text{ MB/s}$) execute 10-15 iterations of dynamic Huffman tree re-weighting and block splitting, reaching smaller physical binaries.
- To achieve 100% genuine physical Pareto dominance (strictly smaller file size AND faster throughput — upper-right quadrant) without any fake/cached data, TTZip requires a native in-process C multi-pass iterative Zopfli engine accelerated by Apple Silicon 18-core multi-block parallelism and ARM NEON cost calculation.

## User Scenarios & Personas

### Scenario 1: Preserving High-Speed Graph Mid-Tier
Developers and users who want high compression with instant speed can use **Tier 5 / 6 (Graph Near-Optimal)**, achieving **$150\text{ MB/s}$** throughput and **$3.03\text{ MB}$** output.

### Scenario 2: Maximum Deflate Conquest (Upper-Right of advzip -4 and pigz -11)
Users seeking the absolute smallest ZIP file in existence can invoke **Tier 7 (Extreme Peak / Zopfli 15-Pass)**. The engine uses 18-core multi-block parallel iterative re-weighting to produce **$\le 2.95\text{ MB}$** (97.04% space savings) at **$\ge 1.8\text{ MB/s}$**, strictly outperforming `advzip -4` ($2.99\text{ MB}$ @ $0.71\text{ MB/s}$) on both axes simultaneously.

## Functional Requirements

1. **Hierarchy Preservation (8 Tiers)**:
   - **Tier 0 (Store)**: Method 0 Direct I/O ($> 6.5\text{ GB/s}$)
   - **Tier 1 (Fast)**: Greedy Deflate L2 ($> 5.5\text{ GB/s}$)
   - **Tier 2 (Fast+)**: Lazy Deflate L4 ($> 5.5\text{ GB/s}$)
   - **Tier 3 (Normal)**: Standard Deflate L6 ($> 4.5\text{ GB/s}$)
   - **Tier 4 (Maximum)**: Deep Lazy Deflate L8 ($> 2.5\text{ GB/s}$)
   - **Tier 5 (Graph Fast)**: Fast DAG Deflate L10 ($> 450\text{ MB/s}$)
   - **Tier 6 (Ultra Zopfli)**: Near-Optimal L12 / Zopfli 5-Pass ($> 5.0\text{ MB/s}$ @ $\le 2.99\text{ MB}$, conquering pigz-11)
   - **Tier 7 (Extreme Peak)**: Multi-Pass 15-Iteration Zopfli + Dynamic Block Splitting ($> 1.5\text{ MB/s}$ @ $\le 2.95\text{ MB}$, conquering advzip-4)

2. **In-Process C Implementation (`Sources/CTTZipBridge/ttzip_zopfli_engine.c`)**:
   - Must implement iterative Dynamic Huffman symbol frequency calculation and graph shortest-path cost updates.
   - Must support 32KB cross-block sliding history matching to prevent boundary compression degradation.
   - Must calculate symbol entropy costs using 16-bit fixed-point arithmetic or ARM NEON SIMD tables.
   - Must implement asymptotic cost convergence early-exit (terminating iterations when cost delta $< 0.005\%$).

3. **Multi-Core Parallel Scheduling**:
   - Partitions 100MB files into 2MB L2-cache-friendly tiles.
   - Dispatches iterations concurrently across all 18 CPU cores via `DispatchQueue.concurrentPerform`.
   - Maintains zero heap allocation in hot loop through thread-local state recycling.

## Clarifications

- **Q1**: 如何平衡中间档位的高速与极限档位的最大压缩率？
  - **A1**: Tier 5 保留 Deflate L10（$470\text{ MB/s}$），Tier 6 设定为 Zopfli 5-Pass（$5.5\text{ MB/s}$ @ $2.99\text{ MB}$，压制 pigz-11），Tier 7 设定为 Zopfli 15-Pass + 动态块切分（$1.85\text{ MB/s}$ @ $2.95\text{ MB}$，压制 advzip-4），在 $150\text{ MB/s}$ 处由 `libdeflate 12` 提供近最优快速旁路支持。

## Success Criteria

1. **Physical Integrity & Compliance**:
   - Standard `/usr/bin/unzip -t` and `/usr/bin/unzip -p` 100% pass on all outputs with 0 CRC errors.
2. **Strict Pareto Dominance**:
   - Tier 6 ($2.99\text{ MB}$ @ $> 5.0\text{ MB/s}$) is strictly to the upper-right of `pigz -11` ($3.01\text{ MB}$ @ $3.02\text{ MB/s}$).
   - Tier 7 ($\le 2.95\text{ MB}$ @ $> 1.5\text{ MB/s}$) is strictly to the upper-right of `advzip -4` ($2.99\text{ MB}$ @ $0.71\text{ MB/s}$).
3. **Zero Fabrication**:
   - All numbers must be physically measured with monotonic clock and 100% reproducible.
