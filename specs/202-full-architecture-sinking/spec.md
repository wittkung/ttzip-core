# Feature Specification: Full Architecture Sinking & Swift-Rust Boundary Execution

**Pipeline Level**: `[Full SDD]`
**Feature Branch**: `202-full-architecture-sinking`
**Created**: 2026-08-22
**Status**: Draft
**Input**: User description: "/Users/kevintung/Documents/dev/TTZip/docs/全面下沉计划.md /speckit-specify 全面实施，完成一个标记一个"

---

## Executive Summary & Objective

This feature executes the end-to-end architecture sinking plan for TTZip across all 373 tracked source and test files. The goal is to optimize execution throughput, minimize memory footprint, eliminate garbage collection and reference-counting contention on hot calculation paths, deliver a lightweight zero-dependency standalone command-line binary, solidify explicit cross-language boundaries, and preserve 100% native macOS user experience.

The execution plan categorizes all files into four distinct lifecycle paths:
1. **Terminal & CLI Engine Sinking**: 45 files transitioned into a high-performance, standalone command-line interface with instant startup and structured machine-readable output.
2. **Core Computational & Virtual File System Sinking**: 91 files transitioned into a memory-safe, hardware-accelerated core engine for format parsing, error correction, stream composition, and cached file tree navigation.
3. **Cross-Language Interface & Contract Hardening**: 19 files solidified as thin, bidirectional interface boundaries with strict data layout definitions.
4. **macOS Presentation & Native Service Preservation**: 188 files preserved natively in Swift to deliver uncompromising macOS system integration and desktop user interface fluidness.
5. **Test Suite Alignment**: 30 test suite files partitioned and aligned with their respective engine and user interface layers.

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Standalone High-Speed Command-Line Operations (Priority: P1)

As a command-line user and DevOps automation engineer, I want a single standalone executable binary that executes all archive subcommands (create, extract, list, inspect, test, diff, tree, split, join, repair, recover, bench, doctor, cat, comment, convert, delete, lock, update) with sub-5ms cold startup, zero external runtime dependencies, full POSIX exit code compliance, and structured JSON output.

**Why this priority**: Command-line automation and scripting require instant invocation speed, zero runtime overhead, and strict machine-readable output formats without invoking heavy desktop frameworks.

**Independent Test**: Can be tested completely independently via command-line invocation of all 18 subcommands across various archive formats, verifying zero dependency on external runtime environments, correct POSIX exit codes, and valid JSON output.

**Acceptance Scenarios**:
1. **Given** a user running a batch automation script, **When** invoking the archive CLI with any valid subcommand and arguments, **Then** the operation completes with exit code `0`, instant response under 5ms, and correct output format (plain text or structured JSON).
2. **Given** a corrupted or invalid archive input, **When** running integrity inspection or repair commands, **Then** the tool outputs diagnostic details, returns standard non-zero POSIX exit codes, and prevents file system corruption.

---

### User Story 2 - High-Throughput Core Compression & Virtual File System (Priority: P2)

As a power user processing massive multi-gigabyte archives containing hundreds of thousands of files, I want archive compression, decompression, checksum calculation, error correction recovery, and in-memory directory navigation to run at theoretical hardware limits with bounded memory usage and zero user-interface freezing.

**Why this priority**: Core computing tasks are latency-critical and throughput-critical. Offloading heavy computations to zero-overhead execution models prevents memory fragmentation and CPU stalls.

**Independent Test**: Can be tested independently by running multi-gigabyte compression, decompression, error-correction matrix recovery, and large-scale virtual directory hierarchy queries, validating sustained high throughput and bounded memory usage.

**Acceptance Scenarios**:
1. **Given** a multi-gigabyte archive with error-correction parity records, **When** damaged data blocks are encountered, **Then** the engine automatically reconstructs the original content at maximum hardware throughput.
2. **Given** an archive containing over 100,000 files, **When** the user searches or navigates the virtual file hierarchy, **Then** query responses are returned instantaneously without memory exhaustion or high memory churn.

---

### User Story 3 - Seamless Desktop Application & Native macOS Integration (Priority: P3)

As a macOS desktop user, I want the native desktop application to maintain fluid 60fps responsiveness, full Apple Silicon optimization, QuickLook previews, Drag-and-Drop file handling, Finder synchronization, and Touch ID biometric authentication while transparently leveraging the optimized computing engine underneath.

**Why this priority**: User interface responsiveness and macOS platform integration are the primary value drivers for end users on macOS.

**Independent Test**: Can be tested independently by launching the macOS application, executing drag-and-drop operations, viewing in-archive QuickLook previews, configuring compression settings, and verifying smooth UI updates during active archive tasks.

**Acceptance Scenarios**:
1. **Given** an ongoing archive compression or extraction task, **When** progress updates are emitted from the computing engine, **Then** the desktop interface updates progress indicators smoothly at 60fps without dropping frames or stalling user interactions.
2. **Given** an encrypted archive, **When** the user attempts to unlock the archive with biometric authentication, **Then** Touch ID authentication succeeds and securely delivers access without leaking credentials.

---

## Edge Cases

- **Corrupt Archive Headers & Stream Truncation**: System MUST cleanly detect invalid magic signatures, truncated streams, or cyclic directory references and abort with descriptive error codes without crashing or leaking memory.
- **Path Traversal & Zip Slip Exploits**: System MUST neutralize relative path manipulations (e.g. `../` components or absolute path escapes) before extracting any file to the target filesystem.
- **Resource Starvation on Extreme Files**: System MUST enforce bounded memory usage (e.g. streaming buffers $\le 64\text{MB} \sim 128\text{MB}$) regardless of total archive or file size.
- **Interrupted File Operations**: System MUST employ transactional temporary files and atomic rename operations so that aborted operations leave no corrupt half-written files at the destination.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The standalone command-line interface MUST support all 18 core archive subcommands (`create`, `extract`, `list`, `info`, `check`, `hash`, `diff`, `tree`, `split`, `join`, `repair`, `recover`, `bench`, `doctor`, `cat`, `comment`, `convert`, `delete`, `lock`, `update`) with full argument parity.
- **FR-002**: The standalone command-line interface MUST provide optional `--json` flag output formatting valid structured JSON across all inspection and execution commands.
- **FR-003**: The core calculation engine MUST provide hardware-accelerated CRC64, CRC32, and checksum verification routines.
- **FR-004**: The core calculation engine MUST support Reed-Solomon Forward Error Correction (FEC) recovery record generation and damaged block restoration.
- **FR-005**: The virtual file system (VFS) and memory page management MUST operate with bounded memory footprint and zero unnecessary heap allocations on hot traversal paths.
- **FR-006**: All cross-language data structures exchanged across subsystem boundaries MUST maintain explicit 8-byte aligned layouts with zero undefined behavior.
- **FR-007**: The desktop application and macOS-specific platform capabilities (AppKit, SwiftUI, QuickLook, Touch ID, Finder Sync, Sparkle updates) MUST remain 100% native in Swift.
- **FR-008**: The execution progress of the architectural sinking plan across all 373 files MUST be tracked deterministically with milestone check-offs ("完成一个标记一个").

---

### Key Entities

- **Archive Operation Session**: Represents an active compression, extraction, verification, or repair operation, including configuration options, cancellation tokens, and progress streaming channels.
- **Virtual File System Tree**: In-memory representation of archive directory hierarchies supporting indexed lookups, filtering, and rapid node traversal.
- **Integrity & Repair Report**: Structured diagnostic model capturing format compliance, checksum results, damaged sector maps, and recovery status.
- **CLI Subcommand Request & Response**: Normalized command-line parameter model and corresponding structured execution outcome.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Standalone CLI cold-start execution time is under $5\text{ms}$.
- **SC-002**: 100% of the 18 CLI subcommands operate natively with complete test coverage.
- **SC-003**: 100% of all existing test suites pass without regression.
- **SC-004**: Memory footprint during multi-gigabyte streaming operations remains bounded within configured buffer thresholds ($\le 128\text{MB}$).
- **SC-005**: All source files conform to codebase line-of-code guidelines ($\le 800\text{ LOC}$ per file).
- **SC-006**: Full repository automated verification gate passes 100% clean without errors or warnings.

---

## Assumptions

- **Target Platforms**: macOS 14.0+ on Apple Silicon (ARM64) and Intel (x86_64), with CLI binaries capable of cross-platform compilation.
- **Architecture Invariants**: Complies with the TTZip Engineering Constitution (stream-first, invariant-first defense, bounds-first memory safety, oracle-first validation).
- **Subsystem Boundary**: Desktop UI layers and macOS system frameworks stay in Swift; core arithmetic, data parsing, VFS structures, and standalone CLI stay in Rust; bidirectional FFI glue connects the layers.
