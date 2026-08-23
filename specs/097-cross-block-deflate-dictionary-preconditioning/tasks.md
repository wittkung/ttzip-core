# Tasks: 097-cross-block-deflate-dictionary-preconditioning

## Phase 1: User Story 1 - Multi-Block Dictionary Test Harness & Verification (Priority: P1)

**Story Goal**: Implement exhaustive unit and consensus tests verifying cross-block Deflate dictionary preconditioning and ratio gains.
**Independent Test**: `swift test --filter CrossBlockDeflateDictionaryTests`.

- [x] T001 [P] [US1] Create `Tests/TTZipTests/CrossBlockDeflateDictionaryTests.swift` with multi-block test corpus, dictionary vs non-dictionary ratio assertions, and `/usr/bin/unzip` system oracle roundtrip.
- [x] T002 [US1] Run `swift test --filter CrossBlockDeflateDictionaryTests` and assert 100% bit-exact decompressed SHA-256 match.

---

## Phase 2: User Story 2 - Performance Gates & Full Regression (Priority: P2)

**Story Goal**: Verify zero performance regression across all 13 constitutional performance gates and 525+ tests.
**Independent Test**: `swift test --filter XCTestPerformanceMeasureTests` and full `swift test`.

- [x] T003 [US2] Execute `swift test --filter XCTestPerformanceMeasureTests` and verify all 13 throughput floors pass.
- [x] T004 [US2] Execute full 6-stage CI gate via `./scripts/run_local_ci_gate.sh`.
