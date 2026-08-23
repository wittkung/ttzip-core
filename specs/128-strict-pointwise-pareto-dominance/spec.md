# Feature Specification: Strict Pointwise Pareto Dominance over libdeflate

**Feature ID**: `128-strict-pointwise-pareto-dominance`  
**Status**: In Progress  
**Target Platform**: macOS 14.0+ (Apple Silicon M-Series ARM64)  

---

## 1. Executive Summary & Vision

The overarching definition of Pareto Dominance mandated for TTZip is **Strict Pointwise Pareto Superiority**:
For **every single evaluation point** $(S_{\text{lib}}, R_{\text{lib}})$ produced by `libdeflate` (from Level 1 through Level 12) across all 4 standard benchmark corpora (`Structured JSON 100MB`, `Binary Mach-O 100MB`, `Mixed Modality Workspace 100MB`, and `enwik8 100MB`), TTZip **MUST have at least one test point $(S_{\text{ttzip}}, R_{\text{ttzip}})$ located strictly in its upper-right quadrant**:

$$S_{\text{ttzip}} \ge S_{\text{lib}} \quad \text{AND} \quad R_{\text{ttzip}} \ge R_{\text{lib}} \quad (\text{i.e. } \text{CompressedSize}_{\text{ttzip}} \le \text{CompressedSize}_{\text{lib}})$$

This guarantees an absolute convex hull enclosure where no competitor point can protrude beyond TTZip's performance boundary.

---

## 2. User Scenarios & Acceptance Criteria

### Scenario 1: Structured Logs & JSON 100MB Pointwise Dominance
- **libdeflate L1** (5.90 GB/s, 0.92 MB): TTZip L1 must reach $\ge 6.00\text{ GB/s}$ and $\le 0.80\text{ MB}$.
- **libdeflate L2** (2.15 GB/s, 0.37 MB): TTZip L4/L5 must reach $\ge 2.20\text{ GB/s}$ and $\le 0.36\text{ MB}$.
- **libdeflate L3..L9** (1.42 GB/s, 0.35 MB): TTZip L6..L9 must reach $\ge 1.45\text{ GB/s}$ and $\le 0.35\text{ MB}$.
- **libdeflate L10..L12** (102~140 MB/s, 0.35 MB): TTZip L8..L10 must reach $\ge 150\text{ MB/s}$ and $\le 0.35\text{ MB}$, while TTZip L11/L12 pushes peak ratio to $0.18\text{ MB}$.

### Scenario 2: Binary Mach-O 100MB Pointwise Dominance
- **libdeflate L1** (7.35 GB/s, 0.84 MB): TTZip L1 must reach $\ge 7.50\text{ GB/s}$ and $\le 0.65\text{ MB}$.
- **libdeflate L2** (2.13 GB/s, 0.25 MB): TTZip L4/L5 must reach $\ge 2.20\text{ GB/s}$ and $\le 0.25\text{ MB}$.
- **libdeflate L3..L9** (1.40 GB/s, 0.24 MB): TTZip L6..L9 must reach $\ge 1.45\text{ GB/s}$ and $\le 0.24\text{ MB}$.
- **libdeflate L10..L12** (95~135 MB/s, 0.23 MB): TTZip L10..L12 must reach $\le 0.16\text{ MB}$.

### Scenario 3: Mixed Modality Real-World Workspace 100MB Pointwise Dominance
- **libdeflate L1** (432.6 MB/s, 37.66 MB): TTZip L1/L2 must reach $\ge 500\text{ MB/s}$ and $\le 37.60\text{ MB}$.
- **libdeflate L2** (424.1 MB/s, 37.50 MB): TTZip L3/L4 must reach $\ge 450\text{ MB/s}$ and $\le 37.45\text{ MB}$.
- **libdeflate L3..L9** (338.0 MB/s, 37.24 MB): TTZip L5..L7 must reach $\ge 350\text{ MB/s}$ and $\le 37.20\text{ MB}$.
- **libdeflate L10..L12** (35~90 MB/s, 37.17 MB): TTZip L10..L12 must reach $\le 34.95\text{ MB}$.

### Scenario 4: enwik8 100MB Pointwise Dominance
- **libdeflate L1** (1.49 GB/s, 4.01 MB): TTZip L4/L5 reaches $\ge 2.00\text{ GB/s}$ and $\le 3.91\text{ MB}$ (Upper-Right Dominant).
- **libdeflate L3** (1.07 GB/s, 3.34 MB): TTZip L5/L6 reaches $\ge 1.10\text{ GB/s}$ and $\le 3.30\text{ MB}$ (Upper-Right Dominant).
- **libdeflate L6** (721.8 MB/s, 3.21 MB): TTZip L6/L7 reaches $\ge 750\text{ MB/s}$ and $\le 3.20\text{ MB}$ (Upper-Right Dominant).
- **libdeflate L9** (263.2 MB/s, 3.18 MB): TTZip L8/L9 reaches $\ge 300\text{ MB/s}$ and $\le 3.18\text{ MB}$ (Upper-Right Dominant).
- **libdeflate L12** (12.1 MB/s, 3.02 MB): TTZip L11/L12 reaches $\le 3.00\text{ MB}$ (Ultra-Ratio Dominant).

---

## 3. Clarifications

- **## Clarifications**:
  - **Q1**: What does "在它的右上方" (Upper-Right Quadrant) mean mathematically?
    - **A1**: In a Cartesian coordinate system where the X-axis is Throughput (MB/s or GB/s, larger is better, to the right) and the Y-axis is Space Savings / Compression Ratio (percentage or reciprocal size, larger is better, upwards), a point $(S_1, R_1)$ is to the upper-right of $(S_0, R_0)$ if and only if $S_1 \ge S_0$ and $R_1 \ge R_0$. This is the exact definition of Pareto Dominance.
  - **Q2**: Which architectural changes are needed to push TTZip Level 1 from 5.45 GB/s to $\ge 6.2\text{ GB/s}$ on JSON and $\ge 7.5\text{ GB/s}$ on Binary?
    - **A2**: In `ttzip_deflate_fast.c`, implement a 4-Byte Stride Unrolled Skip Loop when no match is found, combined with direct hash bucket prefetching and 64-bit SWAR match length computation.
  - **Q3**: How to ensure Level 6..9 achieve $\le 3.20\text{ MB}$ on enwik8 at $\ge 750\text{ MB/s}$?
    - **A3**: In `ttzip_deflate_lazy.c`, refine the 4-way hash bucket update and hash chain depth (depth 8 for Level 6, depth 16 for Level 7, depth 32 for Level 8) with ARM64 SWAR match extension.

---

## 4. Functional Requirements

1. **FR-001 (Level 1 4-Byte Stride & Prefetch Fast-Path)**: `ttzip_deflate_hybrid_fast_find_matches` must achieve $\ge 6.2\text{ GB/s}$ on JSON and $\ge 7.5\text{ GB/s}$ on Binary Mach-O.
2. **FR-002 (Level 2..5 4-Way Compact HT-4 Tuning)**: Achieve $\ge 2.2\text{ GB/s}$ on JSON/Binary and $\ge 450\text{ MB/s}$ on Mixed Workspace.
3. **FR-003 (Level 6..9 Deep Lazy Refinement)**: Achieve $\ge 750\text{ MB/s}$ at $\le 3.20\text{ MB}$ on enwik8, and $\ge 1.45\text{ GB/s}$ on JSON/Binary.
4. **FR-004 (Pointwise Assertion In Benchmarks)**: Add automated assertions verifying that for every libdeflate level tested, at least one TTZip point has $S_{\text{ttzip}} \ge S_{\text{lib}} \land R_{\text{ttzip}} \ge R_{\text{lib}}$.
5. **FR-005 (100% Bit-Exactness & Decompression)**: All archives must decompress 100% cleanly with `/usr/bin/unzip -t` and pass all 1138 unit tests.

---

## 5. Success Criteria

- [ ] **SC-001**: 100% of libdeflate evaluation points have a strictly dominating TTZip point in the upper-right quadrant on all 4 benchmark corpora.
- [ ] **SC-002**: JSON Level 1 throughput $\ge 6.0\text{ GB/s}$ with compressed size $\le 0.80\text{ MB}$.
- [ ] **SC-003**: Binary Level 1 throughput $\ge 7.5\text{ GB/s}$ with compressed size $\le 0.65\text{ MB}$.
- [ ] **SC-004**: Mixed Workspace Level 1..4 achieves $\ge 500\text{ MB/s}$ at $\le 37.60\text{ MB}$.
- [ ] **SC-005**: All 1138 regression tests pass with 0 failures.
