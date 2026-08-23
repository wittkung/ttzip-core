# Quickstart: TTZip Delta Pipeline & Automated Binary/Compression Audit

**Feature Directory**: `specs/140-ttzip-delta-pipeline-and-binary-audit`  
**Status**: Ready  

---

## Scenario 1: Execute Local Delta Audit & Print Terminal Summary

### Command
```bash
swift run ttzip-bench delta
```

### Expected Output
```text
==========================================================================================================================
📊 TTZip Automated Delta Audit (Mach-O Binary & Multi-Level Compression)
==========================================================================================================================
Target: ttzip-bench (arm64 Darwin) | Head: 6ec5510 @ main | Base: 72f9808 @ main~1

[1] Binary Footprint
  Raw Size:      Base=8.44MB  Head=8.44MB  (Δ +0B, +0.00%)
  Stripped Size: Base=6.12MB  Head=6.12MB  (Δ +0B, +0.00%)
  Section __text: 7.62MB (Δ +0B)
  Exported Symbols: 184 defined, 0 added, 0 removed

[2] Compression Density Delta (160 Points)
  Deflate (L1..L12): 48/48 points IDENTICAL (Δ 0.00%)
  Zstandard (L1..L19): 76/76 points IDENTICAL (Δ 0.00%)
  Bzip2 (L1..L9): 36/36 points IDENTICAL (Δ 0.00%)

Summary: 160/160 Points Passed | 0 Regressions | Overall Verdict: ✅ PASS
==========================================================================================================================
```

### Failure Diagnostic
- If exit code is 70, check the output for any `🔴 REGRESSION` lines where compressed payload expanded by $> 0.10\%$.

---

## Scenario 2: Generate GitHub PR Review Markdown Report

### Command
```bash
swift run ttzip-bench delta --markdown-out delta_report.md
```

### Expected Output
- Generates `delta_report.md` with collapsible `<details open>` tables matching zlib-ng `/delta` style, ready for GitHub Action comment posting.

---

## Scenario 3: Run Standalone Automated Shell Script Gate

### Command
```bash
./scripts/run_delta_audit.sh
```

### Expected Output
```text
======================================================================
⚡️ TTZip Automated Delta Audit (Binary Size & Compression Ratio)
======================================================================
✅ Delta Audit Passed! 100% compliant and ready for PR review.
```
