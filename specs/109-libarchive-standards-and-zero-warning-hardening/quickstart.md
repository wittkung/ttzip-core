# Quickstart & Verification Guide: Native Test Log Architecture

## Scenario 1: Native Clean In-Process Test Execution

### Command
```bash
swift test
```

### Expected Output
- Clean single-line progress for each test suite (e.g. `✓ NativeDeflateEngineTests (5 tests, 69ms)`).
- End-of-run Totals table:
  ```text
  ================================================================================
  Totals:
    Test Suites:            86 passed, 0 failed, 86 total
    Tests:                1086 passed, 23 skipped, 1109 total
    Duration:             36.38s
    Status:               ALL TESTS PASSED (100% GREEN)
  ================================================================================
  ```
- Wall time $\le 40.0\text{s}$. Exit code `0`.

### Failure Diagnostic
- If any test fails, inspect the red failure diagnostic section immediately beneath the failing suite name, showing `file:line` and the dumped 2,000-entry memory log buffer.

---

## Scenario 2: Automated Codebase Standards & Zero-Warning Gate

### Command
```bash
./scripts/lint_codebase_standards.sh
```

### Expected Output
```text
================================================================
  TTZip Automated Code Standards & Zero-Warning Lint Latch
================================================================
▶ [Step 1/3] Verifying SPDX License & Copyright Headers on TTZip sources...
✅ SPDX Headers 100% verified on all TTZip-authored files.
▶ [Step 2/3] Verifying 100% Libarchive English in Native C & Core Bridge...
✅ C Bridge & Native Deflate 100% English Doxygen compliant.
▶ [Step 3/3] Compiling Targets & Tests with -warnings-as-errors...
================================================================
🎉 ALL CODEBASE STANDARDS & ZERO-WARNING GATES PASSED (100% OK)
================================================================
```
- Exit code `0`.

### Failure Diagnostic
- If step 1 fails: Run `find Sources Tests scripts -name "*.swift" -o -name "*.c" -o -name "*.h"` and add missing SPDX headers.
- If step 2 fails: Check C bridge files for non-ASCII characters.
- If step 3 fails: Fix all compiler warnings flagged by Swift 6.0.
