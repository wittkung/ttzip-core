# Feature Specification: 201-swift-to-rust-full-architecture-sinking

**Pipeline Level**: `[Full SDD]`
**Feature Branch**: `201-swift-rust-full`
**Created**: 2026-08-22
**Status**: Specified

## 1. Executive Summary & Strategic Motivation

TTZip contains 373 Swift source and test files. To achieve ultimate throughput, zero-copy I/O, SIMD hardware acceleration (ARM64 NEON / PMULL), and sub-5ms cold start across all platforms, this feature executes the 4-phase CTO Architecture Sinking Plan:

1. **Phase 1 (Standalone Cross-Platform CLI Engine)**: Sinks `Sources/TTZipCLI` (44 files) and `Sources/TTZipBench` into the standalone Rust CLI engine (`ttzip` binary in `rust/ttzip-tui`), achieving zero runtime dependency on Swift, native POSIX command routing, ANSI formatting, and `--json` structured outputs.
2. **Phase 2 (Core Computation, VFS & Streaming Sinking)**: Sinks CPU/SIMD-intensive algorithms (CRC64, ARM PMULL, Reed-Solomon FEC, Zip Extra Field parsing, magic signature detection, VFS LZ4 cache pool, and streaming pipelines) into `rust/ttzip-glue`, eliminating Swift ARC overhead on hot paths.
3. **Phase 3 (Unified Swift-Rust FFI Contract Solidification)**: Hardens `TTZipCore/Bridge` and `TTZipCore/Facades` with exact C-ABI struct layouts and memory safety boundaries, using Rust as the authoritative source of truth.
4. **Phase 4 (Pristine macOS GUI Native Experience)**: Preserves 100% of the 144 SwiftUI/AppKit GUI and macOS-specific integration files in Swift, ensuring uncompromising macOS design system elegance.

---

## 2. User Scenarios & Acceptance Criteria

### User Story 1 - Standalone Rust CLI Complete Feature Parity (Priority: P1)

As a command-line user and CI/CD automation engineer, I want a standalone native `ttzip` binary that executes all 18 archive operations (`create`, `extract`, `list`, `info`, `hash`, `diff`, `tree`, `split`, `join`, `repair`, `recover`, `bench`, `doctor`, `cat`, `comment`, `convert`, `delete`, `update`, `lock`) with instant startup ($< 5\\text{ms}$), zero external runtime dependencies, and standardized POSIX exit codes and JSON output.

**Acceptance Scenarios**:
1. **Given** the standalone Rust binary `ttzip`, **When** the user executes any valid archive subcommand, **Then** the command completes successfully with exit code `0` and displays structured ANSI or `--json` output.
2. **Given** an invalid archive or corrupted input, **When** invoking `ttzip check` or `ttzip repair`, **Then** clear diagnostic reports are rendered with proper POSIX error exit codes.

---

### User Story 2 - High-Throughput Core Algorithmic Sinking & VFS Pool (Priority: P2)

As a developer processing massive multi-gigabyte archives and millions of entries, I want CRC64/PMULL hardware checksums, Reed-Solomon FEC recovery, Zip Extra Field parsing, and VFS memory page pools executed entirely inside Rust with zero Swift ARC allocation overhead.

**Acceptance Scenarios**:
1. **Given** an archive with Reed-Solomon error correction data, **When** parity recovery is triggered, **Then** Rust SIMD Galois Field routines execute at maximum throughput.
2. **Given** virtual file system traversal on 100,000+ entries, **When** querying nodes, **Then** memory allocation remains flat without unbounded ARC reference churn.

---

### User Story 3 - Swift Thin Facade & macOS GUI Preservation (Priority: P3)

As a macOS GUI desktop user, I want the SwiftUI/AppKit desktop application (`TTZipApp`) and its macOS-native features (QuickLook, Dock progress, TouchID, Finder synchronization) to run natively with zero regression, communicating seamlessly with the Rust core via hardened C-ABI bridges.

**Acceptance Scenarios**:
1. **Given** `TTZipApp` performing an extraction or compression, **When** progress updates occur, **Then** 60fps throttled events are dispatched from Rust to SwiftUI view models without UI stutter.

---

## 3. Requirements

### Functional Requirements

- **FR-001**: Standalone Rust CLI `ttzip` MUST support all 18 standard archive subcommands with identical flags and argument parsing semantics.
- **FR-002**: Standalone Rust CLI MUST provide `--json` structured output for automation and machine consumption.
- **FR-003**: CRC64, CRC32, and Adler32 checksum engines in Rust MUST utilize ARM64 PMULL / NEON hardware intrinsics when available.
- **FR-004**: VFS LZ4 cache pool and memory page pools MUST operate with strict RAII lifetime management and zero unneeded allocations.
- **FR-005**: All C-ABI structs exposed between Rust and Swift MUST enforce 8-byte alignment, explicit field sizing, and zero undefined layout behavior.
- **FR-006**: Swift `TTZipCLI` and `TTZipCore` facades MUST seamlessly delegate operations to the Rust engine.
- **FR-007**: macOS GUI (`TTZipApp`) and associated AppKit/SwiftUI components MUST remain 100% in Swift.

---

## 4. Success Criteria

### Measurable Outcomes

- **SC-001**: Standalone Rust CLI cold-start latency is under $5\\text{ms}$.
- **SC-002**: 100% of the 18 CLI subcommands operate natively in Rust with complete test coverage.
- **SC-003**: All 525+ Swift unit and integration tests (`swift test`) pass without regression.
- **SC-004**: All Rust workspace unit and integration tests (`cargo test --workspace`) pass 100%.
- **SC-005**: Full repository passes LOC limits ($\\le 800\\text{ LOC}$ per file) via `./scripts/lint_loc_gate.sh`.
- **SC-006**: Full local CI/CD gate (`./scripts/run_local_ci_gate.sh`) passes 100% green.
