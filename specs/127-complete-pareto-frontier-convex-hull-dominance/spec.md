# Feature Specification: Complete Pareto Frontier Convex Hull Dominance

**Feature ID**: `127-complete-pareto-frontier-convex-hull-dominance`  
**Status**: Draft  
**Target Platform**: macOS 14.0+ (Apple Silicon M-Series)  

---

## 1. Executive Summary & Vision

TTZip's native Deflate engine must establish an unbroken, convex Pareto frontier that strictly encloses and surpasses `libdeflate-1.22`, `Apple libcompression`, `7-Zip`, and `minizip-ng` across all four standardized benchmark corpora (`Structured JSON 100MB`, `Binary Mach-O 100MB`, `Mixed Modality Workspace 100MB`, and `enwik8 100MB`).

Every tier of TTZip must either deliver higher throughput at the same or better compression ratio, or achieve a smaller compressed size at equivalent throughput, leaving zero competitor data points outside or protruding through TTZip's convex hull.

---

## 2. User Scenarios & Acceptance Tests

### Scenario 1: Extreme Real-Time Ingestion (Structured JSON & Logs 100MB)
- **User Action**: Compresses 100MB structured telemetry/JSON stream with Level 1 or Level 2.
- **Expectation**: Throughput exceeds $6.0\text{ GB/s}$ (surpassing `libdeflate L1`'s 5.62 GB/s) while compressing to $\le 0.72\text{ MB}$ (22% smaller than `libdeflate L1`'s 0.92 MB).
- **Mid-Tier Expectation**: Level 4/5 throughput exceeds $2.5\text{ GB/s}$ at $\le 0.36\text{ MB}$ (surpassing `libdeflate L2`'s 2.12 GB/s, 0.37 MB).

### Scenario 2: Binary & Executable Code Archiving (Binary Mach-O 100MB)
- **User Action**: Packages compiled binaries with Level 1.
- **Expectation**: Throughput exceeds $7.5\text{ GB/s}$ (surpassing `libdeflate L1`'s 7.19 GB/s) while compressing to $\le 0.64\text{ MB}$ (24% smaller than `libdeflate L1`'s 0.84 MB).
- **Mid-Tier Expectation**: Throughput exceeds $2.5\text{ GB/s}$ at $\le 0.24\text{ MB}$ (surpassing `libdeflate L2`'s 2.12 GB/s).

### Scenario 3: Real-World Workspace Multi-Modality Archiving (Mixed Workspace 100MB)
- **User Action**: Archives mixed source files, assets, and configs.
- **Expectation**:
  - Level 1: $\ge 550\text{ MB/s}$, size $\le 37.60\text{ MB}$ (dominating `libdeflate L1` at 434 MB/s, 37.66 MB).
  - Level 3/4: $\ge 450\text{ MB/s}$, size $\le 37.45\text{ MB}$ (dominating `libdeflate L2` at 424 MB/s, 37.50 MB).
  - Level 5/6: $\ge 350\text{ MB/s}$, size $\le 37.22\text{ MB}$ (dominating `libdeflate L4..L9` at 338 MB/s, 37.24 MB).
  - Level 10..12: $\le 34.95\text{ MB}$ (breaking through the 37.17 MB libdeflate barrier by 2.22 MB).

### Scenario 4: Single-Core Full PK Convex Hull Enclosure (enwik8 100MB)
- **User Action**: Executes single-core benchmark comparing TTZip against all major competitors.
- **Expectation**: TTZip's 12-tier curve forms the strict outer convex envelope from Store (22.9 GB/s) through Fast (3.5+ GB/s), Balanced (1.0+ GB/s, 3.34 MB), Deep (800+ MB/s, 3.20 MB), down to Extreme (2.85 MB).

---

## 5. Clarifications

- **Q1**: What is the target throughput for Level 1 on Binary and JSON?
  - **A1**: $\ge 6.2\text{ GB/s}$ for Structured JSON (beating libdeflate L1's 5.62 GB/s) and $\ge 7.5\text{ GB/s}$ for Binary (beating libdeflate L1's 7.19 GB/s).
- **Q2**: How should mid-tier speeds ($L_2 \sim L_5$) compare against libdeflate $L_2$ ($2.12\text{ GB/s}$)?
  - **A2**: TTZip's fast lazy matchers ($L_2/L_3$) must achieve $\ge 2.5\text{ GB/s}$ on structured data while keeping smaller compressed sizes.
- **Q3**: How to resolve competitor points protruding outside TTZip's curve on enwik8?
  - **A3**: Re-align `testZipSingleCoreParetoFrontier` to sweep the calibrated 12-tier native spectrum ($L_0 \sim L_{12}$) rather than old legacy tiers.

---

## 3. Functional Requirements

1. **FR-001 (Direct JSON 3B Table Vector Bypass)**: Level 1 matchfinder must achieve $\ge 6.2\text{ GB/s}$ on repetitive JSON tokens by batching 3-byte token lookups into 64-bit vector registers.
2. **FR-002 (Binary Instruction Step-4 Vectorizer)**: Binary matchfinder must achieve $\ge 7.5\text{ GB/s}$ on ARM64 4-byte instruction words.
3. **FR-003 (Fast-Lazy 4-Way Compact Lookup Optimization)**: Mid-tier levels ($L_2 \sim L_5$) must achieve $\ge 2.5\text{ GB/s}$ on structured data and $\ge 350\text{ MB/s}$ on mixed data.
4. **FR-004 (Full PK Test Alignment)**: `testZipSingleCoreParetoFrontier` in `ZipSingleCoreParetoFrontierPkTests.swift` must evaluate the native 12-tier engine spectrum.
5. **FR-005 (Zero-Regression Hard Floor)**: All 1138 tests and 13 performance gate tests must pass with 0 failures.

---

## 4. Success Criteria

- **SC-001**: TTZip Level 1 throughput $> 6.0\text{ GB/s}$ on Structured JSON 100MB with compressed size $< 0.75\text{ MB}$.
- **SC-002**: TTZip Level 1 throughput $> 7.5\text{ GB/s}$ on Binary 100MB with compressed size $< 0.65\text{ MB}$.
- **SC-003**: TTZip mid-tier throughput $> 350\text{ MB/s}$ at size $\le 37.24\text{ MB}$ on Mixed Workspace 100MB.
- **SC-004**: Multi-modal visual inspection of all 4 generated PNG charts verifies complete convex hull enclosure with zero competitor protrusions.
- **SC-005**: 100% test pass rate on `swift test` (1138+ tests).
