# Quickstart Validation Guide: Performance Benchmarking & README Reconstruction

**Feature**: `073-performance-benchmarking-and-readme-reconstruction`
**Date**: 2026-08-18

This guide defines verifiable end-to-end scenarios to validate the performance whitepaper, CLI references, README reconstruction, and licensing consistency.

---

## Scenario 1: Physical Performance Benchmark Suite Execution

### Command
```bash
swift test --filter XCTestPerformanceMeasureTests
```

### Expected Output
```text
Test Suite 'XCTestPerformanceMeasureTests' passed.
Executed 13 tests, with 0 failures (0 unexpected)
[Throughput floors verified: ZIP L1 >= 1500 MB/s, ZIP Extract >= 7500 MB/s, TAR.ZST >= 15000 MB/s]
```

### Failure Diagnostic
- If test fails due to throughput dip below floor, check CPU throttling, thermal state, or debug mode compilation (`-c release` vs debug).
- Verify no background processes are consuming CPU/APFS bandwidth.

---

## Scenario 2: Documentation & Markdown Link Integrity Verification

### Command
```bash
./scripts/validate_docs_links.sh
```
*(Or executing markdown link validation across `README.md`, `docs/PERFORMANCE.md`, `ACKNOWLEDGEMENTS.md`, `CONTRIBUTING.md`, `ARCHITECTURE.md`)*

### Expected Output
```text
[PASS] README.md: All 18 internal and external links resolve (HTTP 200 / file exists).
[PASS] docs/PERFORMANCE.md: All benchmark cross-references valid.
[PASS] LICENSE & ACKNOWLEDGEMENTS.md: Attribution and SPDX identifiers 100% consistent.
```

### Failure Diagnostic
- If relative link fails, verify file casing and exact path under `docs/` or repo root.
- Ensure `docs/PERFORMANCE.md` is present and committed.

---

## Scenario 3: CLI Commands Syntax & Copy-Paste Verification

### Command
```bash
swift run ttzip-cli --help && swift run ttzip-cli man --check
```

### Expected Output
```text
OVERVIEW: TTZip - Ultra-High-Performance Archiving & Compression Engine for macOS.
COMMANDS:
  archive, extract, list, test, bench, inspect, health, man, completion
[PASS] All 9 subcommands successfully registered and documented.
```

### Failure Diagnostic
- If a subcommand is missing from help output, check `CLICommandRouter.swift` command table.
- Verify `Package.swift` targets `TTZipCLI`.

---

## Scenario 4: Licensing Consistency Assertion

### Command
```bash
git grep -E "License-BSD-3-Clause|SPDX-License-Identifier: BSD-3-Clause" Sources/ README.md Formula/
```

### Expected Output
```text
[PASS] Zero conflicting BSD-3-Clause declarations found in README.md or Formula/ttzip-cli.rb.
```

### Failure Diagnostic
- If matches occur in `README.md` or `Formula/`, update badge and formula metadata to reflect `Source-Available / Anti-Copycat License`.
