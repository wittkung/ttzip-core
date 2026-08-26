# Phase 0 Research: Block-Splitting & Cost Evaluation

**Feature**: `123-block-splitting-and-cost-evaluation`
**Created**: 2026-08-19

---

## Research Items

### R001 [SUBAGENT:research] Vectorized Bit-Cost Evaluation Architecture
- **Decision**: Implement `ttzip_eval_huffman_bit_costs` using 64-bit SWAR/NEON dot-product arithmetic over `freqs` and length tables to compute exact bit costs in $< 1\mu s$.
- **Rationale**:
  - Exact bit cost evaluation requires computing $\sum (freq[i] \times len[i])$.
  - Precomputed static lengths are fixed constants (8/9/7/8 bits for litlen, 5 bits for offset).
  - Static cost can be computed in $\sim 0.3\mu s$ via:
    `static_litlen_bits = 8 * sum(f[0..143]) + 9 * sum(f[144..255]) + 7 * sum(f[256..279]) + 8 * sum(f[280..285])`.
    `static_offset_bits = 5 * sum(f_offset[0..29])`.
  - This avoids building full dynamic Huffman trees when static is clearly superior.
- **Alternatives Considered**:
  - *Heuristic length estimation*: Prone to misclassifications where dynamic headers exceed savings.
  - *Floating point entropy calculation*: Slow on single core, introduces transcendental instructions (`log2`).
- **Source**:
  - `Vendor/libdeflate-upstream/lib/deflate_compress.c:600-660`
  - `Sources/CTTZipBridge/native_deflate/ttzip_deflate_huffman.h:30-70`

---

### R002 [SUBAGENT:research] History-Preserving Adaptive Block Splitting
- **Decision**: Split continuous data streams into 64KB chunks with BFINAL=0, maintaining 32KB continuous dictionary history via pointer subtraction (`in + pos - cur_hist_len`).
- **Rationale**:
  - RFC 1951 Deflate blocks are logical bitstream boundaries; sliding dictionary matches seamlessly cross block boundaries without resetting distance references.
  - Splitting at 64KB boundaries bounds tree generation latency to $O(N)$ with $N \le 65536$, keeping all working state in L1 D-Cache.
- **Alternatives Considered**:
  - *Single 100MB giant block*: Requires large token tables exceeding L3 cache (up to 16MB), stalling memory buses.
- **Source**:
  - `Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c:160-175`
