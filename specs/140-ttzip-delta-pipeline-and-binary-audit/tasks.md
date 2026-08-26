# Tasks: TTZip Delta Pipeline & Automated Binary/Compression Audit

**Feature Directory**: `specs/140-ttzip-delta-pipeline-and-binary-audit`  
**Status**: Ready  

---

## Phase 1: Binary Section & Symbol Inspector (Priority: P1)

- [x] T001 [P] [US1] Implement `BinaryInspector` to parse Mach-O/ELF section bytes (`__TEXT.__text`, `__DATA.__data`, `__DATA.__bss`, `strip -x`) in `Sources/TTZipCore/Audit/BinaryInspector.swift`
- [x] T002 [P] [US2] Implement exported symbol extraction (`nm -gU`) and differential set operations in `Sources/TTZipCore/Audit/BinaryInspector.swift`

---

## Phase 2: Multi-Level Compression Ratio & Byte-Level Matrix (Priority: P1)

- [x] T003 [P] [US3] Implement `CompressionDeltaEngine` covering Deflate (L1..L12), Zstd (L1..L19), and Bzip2 (L1..L9) across 4 corpora (160 points) in `Sources/TTZipCore/Audit/CompressionDeltaEngine.swift`
- [x] T004 [US3] Verify multi-level compression engine execution in $< 0.5\text{ s}$ RAM-to-RAM

---

## Phase 3: GitHub Markdown PR Report & CLI Subcommand Wiring (Priority: P1)

- [x] T005 [P] [US4] Implement `DeltaReportFormatter` to generate zlib-ng style GFM report with collapsible `<details>` blocks in `Sources/TTZipCore/Audit/DeltaReportFormatter.swift`
- [x] T006 [US1] Wire `ttzip-bench delta` subcommand into `Sources/TTZipBench/main.swift` supporting `--markdown-out`, `--json-out`, and terminal tables
- [x] T007 [US4] Create executable wrapper script in `scripts/run_delta_audit.sh`

---

## Phase 4: Quality & Test Verification (Priority: P2)

- [x] T008 [US1] Create unit tests in `Tests/TTZipTests/DeltaAuditEngineTests.swift`
- [x] T009 Execute `./scripts/run_local_ci_gate.sh` and verify all 6 stages pass cleanly
- [x] T010 Commit, push, and broadcast completed status card
