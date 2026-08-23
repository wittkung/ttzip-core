# Implementation Plan: 067-ttzip-cli-standalone-release

## Technical Context
`ttzip-cli` is the command-line interface and benchmarking suite for TTZip. It is powered by `TTZipCore` and `CTTZipBridge`. This plan establishes the standalone packaging workflow, E2E test coverage, and Homebrew formula definition for the official release of `ttzip-cli`.

---

## Constitution Check
- [x] **Zero-Cost Abstractions**: CLI commands directly route to in-process C static library pipelines with zero CLI subprocess spawning.
- [x] **Zero Memory Leakage**: All buffer pointers and memory pools adhere to RAII and CUnsafeBufferAdapter rules.
- [x] **Universal Architecture**: Targets `arm64` and `x86_64` macOS 14+ environments.

---

## Phase 0: Research Items
- R001: macOS Universal Binary Compilation & Release Packaging (`research.md`)
- R002: Homebrew Tap & Formula Distribution Standard (`research.md`)
- R003: POSIX Exit Codes & Machine-Readable Output Separation (`research.md`)

---

## Phase 1: Design & Documentation Artifacts
- **Data Model**: `specs/067-ttzip-cli-standalone-release/data-model.md`
- **Contracts**: `specs/067-ttzip-cli-standalone-release/contracts/cli-release-manifest.schema.json`
- **Quickstart Guide**: `specs/067-ttzip-cli-standalone-release/quickstart.md`

---

## Component Change List
1. **CLI Core & Routing**:
   - `Sources/TTZipCLI/CLIArgumentParser.swift`: Support `--json`, `--threads`, `--output`, and standard POSIX flags.
   - `Sources/TTZipCLI/CLICommandRouter.swift`: Enforce exit code contracts (0-5) and stream separation (`stdout` for JSON, `stderr` for logs).
2. **Release Packaging Scripts**:
   - `scripts/package_cli.sh`: Build, universal merge (`lipo`), strip, and compress into release tarball.
   - `Formula/ttzip.rb`: Homebrew formula template.
3. **End-to-End Test Suite**:
   - `Tests/TTZipTests/CLICommandE2ETests.swift`: Comprehensive multi-format compression, extraction, inspection, and error handling tests.
