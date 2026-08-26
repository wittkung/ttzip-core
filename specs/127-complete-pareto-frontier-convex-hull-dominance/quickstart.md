# Quickstart & Verification Guide: Complete Pareto Frontier Convex Hull Dominance

## 1. Single-Core Pareto Frontier 1v1 Duels
```bash
TTZIP_RUN_BENCHMARKS=1 swift test --filter ZipSingleCoreParetoFrontierPkTests
```
- **Expected Output**:
  - `Structured Logs & JSON`: TTZip L1 $\ge 6.0\text{ GB/s}$ ($\le 0.72\text{ MB}$).
  - `Binary & Machine Code`: TTZip L1 $\ge 7.5\text{ GB/s}$ ($\le 0.64\text{ MB}$).
  - `Mixed Modality Workspace`: TTZip L1 $\ge 550\text{ MB/s}$ ($\le 37.60\text{ MB}$), L5 $\ge 350\text{ MB/s}$ ($\le 37.24\text{ MB}$).
  - `Single-Core Full PK (enwik8)`: TTZip 12-tier curve forms outer convex envelope enclosing all competitors.
- **Failure Diagnostic**: Check if matchfinder hash table exceeds 64KB or if early lazy cutoff was bypassed.

## 2. Hard Performance Floor Verification
```bash
swift test --filter XCTestPerformanceMeasureTests
```
- **Expected Output**: All 13 performance gates pass 100% green.
- **Failure Diagnostic**: Ensure zero heap allocations in inner compression loop.

## 3. Full Repository Regression Test
```bash
swift test
```
- **Expected Output**: 1138+ tests pass with 0 unexpected failures.
