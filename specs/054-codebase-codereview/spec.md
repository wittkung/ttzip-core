# Feature Specification: Full Codebase Architecture & Safety Code Review

**Feature Branch**: `054-codebase-codereview`
**Created**: 2026-08-17
**Status**: Draft
**Input**: "分发多个 subagent，对目前的代码做严格 codereview，基于我们的 skill /speckit-specify"

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Systems C & Bridge Architecture Security Audit (Priority: P1)

As a systems architect and release engineer, I need an exhaustive code review across all C11 and POSIX bridge source files in `Sources/CTTZipBridge/` to ensure full compliance with memory bounds, integer clamping, dead-store elimination, SIMD 16KB padding, and POSIX AT-API path safety.

**Why this priority**: C bridge code forms the bare-metal foundation of TTZip; vulnerabilities or buffer mismanagement here compromise entire system security, stability, and throughput.

**Independent Test**: Can be validated by static inspection, adversarial scenario verification (integer overflow, abnormal EOF, malloc failure), and zero-regression compilation.

**Acceptance Scenarios**:
1. **Given** C source files with pointer dereferencing and arithmetic, **When** reviewing conversion of 64-bit offsets to `size_t`, **Then** verify mandatory clamping against `SSIZE_MAX` / boundary checks.
2. **Given** symmetric crypto, KDF, and sensitive password memory, **When** reviewing cleanup code before deallocation, **Then** verify `explicit_bzero` or `memset_s` / volatile pointer zeroing to prevent compiler dead-store elimination.
3. **Given** SIMD NEON vector operations (`vld1q_u8` / `vst1q_u8`), **When** processing trailing unaligned buffers, **Then** verify boundary padding or scalar tail handling to prevent page faults.

---

### User Story 2 - Swift 6 Core Engines & Concurrency Safety Review (Priority: P1)

As an engine maintainer, I need a comprehensive audit of `Sources/TTZipCore/` to verify Swift 6 Sendable correctness, hot-path zero-allocation invariants, thread safety, and fast-path bypass preservation.

**Why this priority**: TTZip relies on extreme parallel throughput; implicit heap allocations or lock contention in GCD worker loops degrade real hardware performance.

**Independent Test**: Can be verified by auditing parallel compression/decompression loops, Keychain secure enclaves, and fast-path bypass logic against the Constitution.

**Acceptance Scenarios**:
1. **Given** parallel compression routines in `Sources/TTZipCore/Zip/`, `SevenZip/`, and `Zstd/`, **When** reviewing inner task closures, **Then** confirm zero `Data(count:)` zero-filling and zero locks inside `concurrentPerform`.
2. **Given** format dispatchers and strategy factories, **When** routing archive requests, **Then** confirm dedicated hardware/format fast paths are preserved without generic slow-path fallback degradation.
3. **Given** password vault and Keychain helpers, **When** managing credential lifecycle, **Then** verify in-memory zeroing and secure token handling.

---

### User Story 3 - Design Pattern Compliance & Hot-Path Isolation Review (Priority: P2)

As a software craftsman, I need an audit of all 28 design patterns in `Sources/TTZipCore/` to ensure patterns are strictly isolated to orchestration/cold layers and never intrude into parallel data planes.

**Why this priority**: Architectural patterns must provide clarity and decoupling without introducing runtime overhead on the critical data path.

**Independent Test**: Can be verified by reviewing each pattern implementation directory against `.agents/skills/design-patterns-guide/SKILL.md`.

**Acceptance Scenarios**:
1. **Given** structural patterns (Composite, Visitor, Decorator), **When** checking usage sites, **Then** confirm they are used only for UI tree rendering and configuration, not inside archive byte streams.
2. **Given** creational/behavioral patterns (Flyweight, Template Method, Observer), **When** used in concurrent contexts, **Then** confirm no lock contention or per-file allocation overhead exists.

---

### User Story 4 - AppKit / SwiftUI UI Layer & macOS Native Integration Review (Priority: P2)

As a macOS desktop application developer, I need an audit of `Sources/TTZipApp/` for `@MainActor` thread safety, input method (TSM) compatibility, responsive glassmorphic rendering, and progress throttling.

**Why this priority**: UI responsiveness, glitch-free window rendering, and non-blocking input handling directly determine the user experience.

**Independent Test**: Can be verified by auditing view models, NSOutlineView coordinator bridges, and progress update publishers.

**Acceptance Scenarios**:
1. **Given** high-frequency background progress events, **When** dispatching to UI, **Then** verify event throttling converges to $\le 60\text{Hz}$ to eliminate UI thread saturation.
2. **Given** popovers, sheets, and modal dialogs, **When** handling text and password inputs, **Then** verify absence of bare `SecureField` that causes macOS TSM Chinese IME system-wide hangs.
3. **Given** Direct vs App Store distribution channels, **When** inspecting Sparkle auto-updater references, **Then** confirm strict `#if !MAS_BUILD` conditional compilation isolation.

---

### User Story 5 - Benchmark Harness, Golden Oracle & Test Invariants Review (Priority: P3)

As a QA and performance engineer, I need an audit of `Tests/` and `Sources/TTZipCLI/` to verify differential test oracles, `.uu` golden defect corpus decoding, and strict historical peak regression gates.

**Why this priority**: Robust tests and real oracles prevent regressions and guarantee bit-level decompression accuracy.

**Independent Test**: Can be verified by auditing test suite coverage, oracle differential assertions, and fuzzing harness crash dumps.

**Acceptance Scenarios**:
1. **Given** golden defect fixtures, **When** loading test inputs, **Then** confirm ASCII `.uu` decoding without external network or filesystem mutation dependencies.
2. **Given** differential test suites, **When** verifying created archives, **Then** confirm dual verification against system native `/usr/bin/tar` and `/usr/bin/unzip`.

---

## Edge Cases

- **Abnormal EOF in Streams**: In-stream truncation during SIMD decompression must gracefully return error codes without segmentation fault.
- **32-bit Integer Wraparound**: Large 64-bit entry sizes must not truncate silently when passed to 32-bit system APIs.
- **TOCTOU Symlink Hijacking**: Archive directory extraction must delay permission and timestamp fixups until all child files are written, using `O_NOFOLLOW` validation.
- **Memory Allocation Failure**: Malloc/posix_memalign failures must cleanly release all allocated intermediate buffers and report `ENOMEM` / `ARCHIVE_FATAL`.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Review suite MUST categorize all findings using standardized severity tags: `[MUST]` (blocking / safety / invariant violation), `[SHOULD]` (strongly recommended quality improvement), `[NIT]` (minor style/clarity tweak), `[QUESTION]` (architectural inquiry), and `[PRAISE]` (exemplary design).
- **FR-002**: Review suite MUST verify all 12 points of the Systems C and Cross-Platform Checklist against `Sources/CTTZipBridge/`.
- **FR-003**: Review suite MUST verify hot-path zero-allocation and lock-free invariants across `Sources/TTZipCore/`.
- **FR-004**: Review suite MUST verify 28 design pattern boundary enforcement according to `design-patterns-guide`.
- **FR-005**: Review suite MUST verify macOS TSM IME safety, `@MainActor` thread safety, and MAS sandbox conditionality in `Sources/TTZipApp/`.
- **FR-006**: Review suite MUST verify test harness differential oracle integrity and historical peak benchmark protection.

### Key Entities

- **ReviewDomain**: Logical subsystem partition (Systems C, Core Engine, Design Patterns, Desktop App, Test & Benchmark).
- **ReviewFinding**: Atomic audit issue containing severity level, file path, line range, violation description, and concrete remediation code.
- **InvariantStatus**: Status of constitutional compliance across the 4 systemic invariants (Stream-First, Invariant-First, Bounds-First, Oracle-First).

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of major subsystems in `Sources/` and `Tests/` audited with zero uncovered critical modules.
- **SC-002**: 0 unaddressed `[MUST]` invariant violations remaining undetected in production pipelines.
- **SC-003**: Clear, actionable remediation proposals provided for every identified code smell or architectural debt.
- **SC-004**: Verification that frozen engine files remain intact and untouched without explicit unfreeze permission.

---

## Assumptions

- Code review is non-destructive during analysis phase (read-only auditing via specialized subagents).
- Findings will directly inform subsequent optimization and hardening tasks in the Spec Kit workflow.
- All reviews adhere strictly to the engineering constitution and project rules.
