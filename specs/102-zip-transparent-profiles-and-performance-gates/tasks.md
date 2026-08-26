# Tasks: Feature 102 (ZIP Transparent Profiles & Performance Gates)

## Phase 1: Core Profile Architecture (US1)
- [x] T001 [US1] Create `ZipCompressionProfile.swift` in `Sources/TTZipCore/Zip/ZipCompressionProfile.swift` defining strong-typed configuration struct and 8 golden static presets.
- [x] T002 [US1] Refactor `effectiveZipRawLevel` in `Sources/TTZipCore/ArchiveCompressionTypes.swift` to delegate transparently to `ZipCompressionProfile.profile(for: level)`.
- [x] T003 [US1] Update `ZipExtremeBlockWriter.swift` in `Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift` to consume `ZipCompressionProfile` and map 1:1 to `TTZipZopfliOptions`.

## Phase 2: Comprehensive Performance Gates Hardening (US2 & US3)
- [x] T004 [US2] Update `ZipMultiCoreParetoFrontierPkTests.swift` in `Tests/TTZipTests/ZipMultiCoreParetoFrontierPkTests.swift` to drive benchmarks via `ZipCompressionProfile.allProfiles`.
- [x] T005 [US3] Update `XCTestPerformanceMeasureTests.swift` in `Tests/TTZipTests/XCTestPerformanceMeasureTests.swift` with modern performance floors aligned with 8 golden profiles.
- [x] T006 [US3] Execute full test suites (`swift test --filter XCTestPerformanceMeasureTests` and `swift test --filter ZipMultiCoreParetoFrontierPkTests`) to assert 100% pass and generate latest Pareto frontier chart.
