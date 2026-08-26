# Quickstart Validation Guide: Feature 074

**Feature**: `074-finder-integration-gui-i18n-fast-lzma2-tuning`  
**Date**: 2026-08-18

---

## Scenario 1: Finder QuickLook Preview Generator Verification

### Validation Command
```bash
swift test --filter QuickLookPreviewTests
```

### Expected Output
```text
Test Suite 'QuickLookPreviewTests' passed.
	 Executed 4 tests, with 0 failures (0 unexpected) in 0.045 seconds
```

### Failure Diagnostic
- If inspection fails, check `ArchiveReader.inspect` error handling and ensure the UTI is recognized in `ArchiveFormatStandardRegistry`.

---

## Scenario 2: GUI Bilingual Localization & Reactive State Switching

### Validation Command
```bash
swift test --filter GUILocalizationTests
```

### Expected Output
```text
Test Suite 'GUILocalizationTests' passed.
	 Executed 5 tests, with 0 failures (0 unexpected) in 0.030 seconds
```

### Failure Diagnostic
- If string keys are missing, check `LocaleCatalogZhHans.strings` and `LocaleCatalogEn.strings` in `Sources/TTZipCore/Localization/Catalogs/`.

---

## Scenario 3: Fast LZMA2 Micro-Tuning & 13 Hard Performance Floors

### Validation Command
```bash
swift test --filter XCTestPerformanceMeasureTests
```

### Expected Output
```text
Test Suite 'XCTestPerformanceMeasureTests' passed.
	 Executed 13 tests, with 0 failures (0 unexpected)
```

### Failure Diagnostic
- If `testSevenZipCompression_Level1_ThroughputFloor` or `testSevenZipCompression_XCTestMeasureMetrics` drops below floor, check memory alignment in `ttzip_lzma2_fast_encoder.c` and prefetching offsets in `ttzip_lzma_hc4_neon.c`.
