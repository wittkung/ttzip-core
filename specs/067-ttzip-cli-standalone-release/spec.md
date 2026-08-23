# Feature Specification: 067-ttzip-cli-standalone-release

**Feature Title**: Standalone Release, Packaging, and Quality Hardening for `ttzip-cli`  
**Status**: Draft  
**Target Milestone**: TTZip CLI v1.0.0 Release  

---

## 1. User Scenarios & Problem Statement

### 1.1 User Scenarios
- **Scenario A (Systems / DevOps Engineer In-Terminal Usage)**: A macOS DevOps engineer wants a blistering fast, in-process archiving tool in their terminal or CI script. They install `ttzip-cli` (via Homebrew tap or standalone binary) and run `ttzip-cli create archive.tar.zst src/ -l 3` or `ttzip-cli extract bundle.7z out/`.
- **Scenario B (Performance Benchmarking & Hardware Validation)**: A developer wants to verify their Apple Silicon M-series compression throughput. They run `ttzip-cli bench 100MB` and receive a clean ANSI/Markdown report with physical monotonic MB/s throughput, compression ratios, and hardware SIMD acceleration metrics.
- **Scenario C (Integrity Testing & Password Recovery)**: A security researcher uses `ttzip-cli test archive.zip` to verify CRC32/SHA256 checksums without writing to disk, or runs `ttzip-cli recover protected.7z wordlist.txt` utilizing full-core parallel decryption.

---

## 2. Functional Requirements & Scope

### 2.1 CLI Interface & Command Ergonomics (P1)
- **FR-001 [POSIX Flag & Exit Code Standard]**: Ensure standard POSIX command line options (`-h`/`--help`, `-v`/`--version`, `-f`/`--format`, `-l`/`--level`, `-p`/`--password`, `-s`/`--split-size`, `-t`/`--threads`, `-o`/`--output`, `--json`).
  - Standard exit codes: `0` (Success), `1` (General error / Invalid arguments), `2` (File I/O error), `3` (Checksum / Integrity verification failure), `4` (Password / Authentication error), `5` (Security threat intercepted / Zip Slip blocked).
- **FR-002 [Full Matrix 16-Format CLI Support]**: Support creation, extraction, and inspection for all 16 supported formats via `--format` flag (`zip`, `7z`, `tar`, `zst`, `gz`, `bz2`, `xz`, `lz4`, `lzip`, `lrzip`, `brotli`, `snappy`, `aar`, `dmg`, `iso`, `wim`).
- **FR-003 [Structured JSON Output Mode]**: Provide `--json` flag across `inspect`, `test`, and `bench` subcommands for automated scripting and CI integration.

### 2.2 Distribution & Packaging (P1)
- **FR-004 [Standalone Universal Binary Build]**: Provide an automated packaging script (`scripts/package_cli.sh`) that builds a universal release binary (`arm64` + `x86_64`) with stripped symbols and minimal binary size.
- **FR-005 [GitHub Release & Homebrew Formula Template]**: Create a formula template (`ttzip.rb`) and release tarball generator for direct `brew install wittkung/tap/ttzip` integration.

### 2.3 Automated Testing & Hardening Matrix (P1)
- **FR-006 [CLI End-to-End Test Suite]**: Implement comprehensive end-to-end integration tests in `Tests/TTZipTests/CLICommandE2ETests.swift` covering `create`, `extract`, `inspect`, `test`, `split`, and encrypted archive cycles with round-trip bit-exact assertions.

---

## 3. Success Criteria & Quality Metrics

1. **Round-Trip Bit Fidelity**: 100% data integrity verified on files compressed and extracted across all 16 formats via `ttzip-cli`.
2. **Sub-Millisecond CLI Startup**: `ttzip-cli --version` and `ttzip-cli --help` execute in `< 10 ms`.
3. **Zero CLI Process Spawning**: 100% of operations execute in-process via C static libraries.
4. **Universal Binary Compliance**: Binary runs natively on both Apple Silicon (M1+) and Intel macOS 14+ machines.

---

## 4. Assumptions & Boundaries

- Target OS: macOS 14.0+ (Sonoma, Sequoia).
- Toolchain: Swift 6.0 / Xcode 16+.
- Release Target: Standalone CLI binary (`ttzip-cli`) and Homebrew formula.
