# Quickstart Guide: Codebase Architecture & Quality Governance Verification

**Feature**: `220-comprehensive-codebase-and-quality-audit`  
**Date**: 2026-08-24  

---

## 1. Quick Verification Commands

### Step 1: Single-File LOC Defense Gate (Hard Limit: <= 800 LOC)
```bash
python3 scripts/lint_loc_gate.py
```
*Expected Outcome*: `✅ [PASS] All source files are clean and under the 800 LOC threshold.`

### Step 2: Systemic Invariants Lint Gate
```bash
python3 scripts/lint_codebase_invariants.py
```
*Expected Outcome*: `[LINT] Scanned workspace. Total violations: 0`

### Step 3: Automated SPDX Header & Codebase Standards Latch
```bash
bash scripts/lint_codebase_standards.sh
```
*Expected Outcome*:
```text
✅ SPDX Headers 100% verified on all TTZip-authored files.
✅ C Bridge & Native Deflate 100% English Doxygen compliant.
🎉 ALL CODEBASE STANDARDS & ZERO-WARNING GATES PASSED (100% OK)
```

### Step 4: Multilingual SDK Automated Verification
```bash
bash scripts/run_all_sdk_tests.sh
```
*Expected Outcome*: All present toolchains pass their respective test suites; missing optional runtimes log informative skip messages.

### Step 5: Full Local CI Gate Execution
```bash
./scripts/run_local_ci_gate.sh --json reports/local_ci_report.json
```
*Expected Outcome*: 4/4 stages PASS with 0 failures.
