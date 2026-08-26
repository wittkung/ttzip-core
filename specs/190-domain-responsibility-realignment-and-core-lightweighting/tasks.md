# Tasks: 190-domain-responsibility-realignment-and-core-lightweighting

## Phase 1: Benchmark Domain Realignment (US1)
- [x] T001 [P] [US1] Realign necessary benchmark runner models to `Sources/TTZipBench/`.
- [x] T002 [P] [US1] Delete `Sources/TTZipCore/Benchmark/` (48 files).

## Phase 2: Swift TUI & Concurrency Patterns Purge (US2)
- [x] T003 [P] [US2] Delete `Sources/TTZipCore/CLI/TUI/` (6 files).
- [x] T004 [P] [US2] Delete `Sources/TTZipCore/ConcurrencyPatterns/` (20 files).
- [x] T005 [P] [US2] Delete `Sources/TTZipCore/Security/MalformedStreamFuzzEngine.swift` (1 file).

## Phase 3: CI Alignment & Final Verification (US3)
- [x] T006 [US3] Verify `swift build` and `swift test` 100% PASS with zero warnings.
- [x] T007 [US3] Run `cargo test --workspace` on all Rust crates.
- [x] T008 [US3] Run `./scripts/run_local_ci_gate.sh` full CI validation.
