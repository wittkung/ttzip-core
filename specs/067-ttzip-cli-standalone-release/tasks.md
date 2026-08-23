# Implementation Tasks: ttzip-cli Standalone Release

**Feature**: `specs/067-ttzip-cli-standalone-release`

---

## Phase 1: CLI Ergonomics & Exit Code Hardening (User Story 1 - P1)

- [x] T001 [P] [US1] Update `Sources/TTZipCLI/TTZipCLIApp.swift` and `Sources/TTZipCLI/CLIArgumentParser.swift` for standard `--version` and `--help` formatting
- [x] T002 [P] [US1] Harden `Sources/TTZipCLI/CLICommandRouter.swift` with POSIX standard exit status codes (0-5)

---

## Phase 2: Standalone Release Packaging & Homebrew Formula (User Story 2 - P1)

- [x] T003 [P] [US2] Create automated universal binary packaging script in `scripts/package_cli.sh`
- [x] T004 [P] [US2] Create Homebrew formula template in `Formula/ttzip.rb`

---

## Phase 3: End-to-End Integration Testing & Verification (User Story 3 - P1)

- [x] T005 [US3] Create comprehensive CLI E2E test suite in `Tests/TTZipTests/CLICommandE2ETests.swift`
- [x] T006 [US3] Run full test suite and verify 100% roundtrip pass rate across all commands

---

## Phase 4: Release Packaging & Git Commit (User Story 4 - P2)

- [x] T007 [US4] Execute `scripts/package_cli.sh` to generate release tarball and compute SHA256 checksum
- [x] T008 [US4] Stage and commit updates to `main`
