# Implementation Plan: All 16 Formats Competitor Benchmark Matrix (010)

## Proposed Changes

### Phase 1: Test Suite & CLI Expansion
- Update `Tests/TTZipTests/AllFormatsPkSuiteTests.swift` to include all 16 compression formats: `[.sevenZip, .zip, .tarZst, .tarGz, .tarBz2, .tarXz, .tar, .lzip, .lz4, .brotli, .lrzip, .aar, .snappy, .wim, .dmg, .iso]`.
- Enhance timeout and logging for multi-format matrix execution.

### Phase 2: Engine & Competitor Invocation Hardening
- Audit `CompetitorBenchmarkRunner+ExtendedExecutors.swift` to ensure robust error handling and proper binary detection for each format.
- Ensure all temporary archives and extracted folders are deleted immediately after each format measurement.

### Phase 3: Reporting & Regression Integration
- Verify `CompetitorReportWriter.swift` generates comprehensive Markdown summaries with winner badges, speedups, and throughput for all 16 formats.
- Update `audit_performance_regression.py` to compare and report regression metrics across all 16 formats.
