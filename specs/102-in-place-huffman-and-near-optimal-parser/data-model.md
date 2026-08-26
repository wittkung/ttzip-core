# Phase 1 Data Model: In-Place Huffman Builder & Near-Optimal Parser

**Feature Branch / Spec Directory**: `specs/102-in-place-huffman-and-near-optimal-parser`  
**Created**: 2026-08-18  
**Status**: Completed  

---

## 1. Entities & Structural Models

### Entity 1: `InPlaceHuffmanResult`
Represents the output of the in-place canonical Huffman tree builder.

| Field Name | Type | Constraints | Description |
| :--- | :--- | :--- | :--- |
| `num_symbols` | `uint32` | $2 \le \text{num\_symbols} \le 1024$ | Total alphabet size (e.g. 288 for litlen, 32 for offset, 19 for precode) |
| `max_codeword_len` | `uint8` | $1 \le \text{max\_codeword\_len} \le 15$ | Configured maximum codeword length in bits |
| `used_symbol_count` | `uint32` | $0 \le \text{used\_symbol\_count} \le \text{num\_symbols}$ | Number of non-zero frequency symbols |
| `codeword_lengths` | `Array<uint8>` | Length = `num_symbols`, each $0 \le \text{len} \le 15$ | Computed bit length for each symbol |
| `reversed_codewords` | `Array<uint32>` | Length = `num_symbols` | RFC 1951 bit-reversed canonical codewords |
| `execution_micros` | `double` | $\ge 0.0$ | Time spent building the tree in microseconds |

### Entity 2: `NearOptimalCompressionResult`
Represents the output of the Level 10-12 Near-Optimal Dynamic Programming compression execution.

| Field Name | Type | Constraints | Description |
| :--- | :--- | :--- | :--- |
| `uncompressed_bytes` | `int64` | $\ge 0$ | Original input payload size |
| `compressed_bytes` | `int64` | $\ge 0$ | Output compressed size in bytes |
| `compression_level` | `int32` | $10 \le \text{level} \le 12$ | Compression level used (10=Fast DAG, 11=Balanced DAG, 12=Deep DAG) |
| `compression_ratio` | `double` | $> 0.0$ | Ratio: $\text{uncompressed\_bytes} / \text{compressed\_bytes}$ |
| `throughput_mbs` | `double` | $> 0.0$ | Monotonic physical throughput in MB/s |
| `passes_executed` | `int32` | $1 \le \text{passes} \le 4$ | Number of dynamic programming relaxation iterations |
| `is_rfc1951_compliant` | `boolean` | `true` | Assertion that stream conforms 100% to RFC 1951 |

---

## 2. Invariants & Bounds

1. **Zero Heap Allocation Invariant**: `ttzip_make_canonical_huffman_code_inplace` operates entirely inside caller-provided memory buffers.
2. **Kraft-McMillan Equality**: $\sum_{i \in \text{used}} 2^{-\text{len}[i]} = 1$ for complete codes.
3. **Bit-Reversal Symmetry**: For any canonical code $C$ of length $L$, $C_{\text{reversed}} = \text{reverse}(C, L)$ where $\text{reverse}(\text{reverse}(x, L), L) = x$.
4. **DAG Path Minimality**: $\text{Cost}(Path_{\text{selected}}) \le \text{Cost}(Path_{\text{greedy}})$.
