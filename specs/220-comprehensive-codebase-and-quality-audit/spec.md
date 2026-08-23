# Feature Specification: Comprehensive Codebase Architecture & Quality Audit Remediation

**Feature Branch**: `220-comprehensive-codebase-and-quality-audit`  
**Classification**: `[Full SDD]`  
**Created**: 2026-08-24  
**Status**: Draft  
**Input**: User description: "详细审计代码仓库、代码架构与代码质量 /speckit-specify"

---

## 1. Problem Statement & Audit Executive Summary

A comprehensive multi-dimensional audit of the TTZip codebase (910 source files across 158,711 lines of code) reveals a high-performance, strictly-layered Dual-Core architecture (Swift 6 + Safe Rust microkernel via pure C-ABI), but identifies specific engineering quality latches and governance debts that require systematic remediation:

1. **License & Header Compliance**: While core C bridge and Rust crates adhere strictly to licensing standards, 146 Swift UI and service files in `Sources/TTZipApp/` are missing the standard `SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0` headers, causing `scripts/lint_codebase_standards.sh` to fail Step 1/3.
2. **Architecture Layering & Microkernel Boundaries**:
   - `Layer 0 (Safe Rust Core)`: Fully decoupled into pure Safe Rust `ttzip-engine` and C-ABI export layer `ttzip-glue`.
   - `Layer 1 (C Bridge ABI)`: Standardized in `Sources/CTTZipBridge/include/` with `ttzip_rust_glue.h`, `ttzip.h`, and `ttzip.hpp`.
   - `Layer 2 (Swift 6 Domain Core)`: Thin facade dispatching to C-ABI with strict `Sendable` types and actor isolation.
   - `Layer 3 (Presentation & CLI)`: AppKit/SwiftUI `TTZipApp` and standalone Rust `ttzip` TUI.
3. **Quality Gates & Invariant Defenses**:
   - Single-file LOC gate: 100% of 776 measured source files strictly pass $\le 800\text{ LOC}$ (maximum file is well under the ceiling).
   - Invariants check (`scripts/lint_codebase_invariants.py`): 0 rule violations.
4. **Multilingual SDK Ecosystem Alignment**: Python (PyO3), Node.js (N-API), C/C++ (CMake/pkg-config), Dart, JVM/Kotlin, and .NET SDKs require a unified automated regression test harness integrated into the local offline CI gate.

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 100% SPDX License Header & Quality Standards Latch Closure (Priority: P1)

Developers and open-source contributors running `./scripts/lint_codebase_standards.sh` or `./scripts/run_local_ci_gate.sh` receive an immediate 100% green pass without header omissions or non-ASCII violations across all 230 Swift files and supporting modules.

**Why this priority**: License header compliance is a mandatory pre-commit gate and prevents merge blockage in continuous integration.

**Independent Test**: Execute `./scripts/lint_codebase_standards.sh` and verify that all three verification stages (SPDX headers, ASCII C-bridge checks, zero-warning build) exit with code 0.

**Acceptance Scenarios**:

1. **Given** a complete scan of `Sources/`, `Tests/`, and `scripts/`, **When** the standard verification script runs, **Then** all files contain the valid SPDX dual-license header and 0 files report missing headers.
2. **Given** C-bridge and native deflate files, **When** non-ASCII encoding checks run, **Then** 100% of symbols and docstrings pass strict ASCII compliance.

---

### User Story 2 - Automated Multilingual SDK Matrix Verification (Priority: P2)

SDK consumers across Python, Node.js, C/C++, Java/Kotlin, Dart, and .NET can run a single unified verification command to test buffer compression, decompression, checksum verification, and archive extraction across all supported language bindings.

**Why this priority**: Ensures that changes in the underlying Rust engine (`ttzip-engine`) or C-ABI bridge (`CTTZipBridge`) do not cause regressions across client libraries.

**Independent Test**: Run `./scripts/run_all_sdk_tests.sh` and verify all available language suites pass in offline local environment.

**Acceptance Scenarios**:

1. **Given** installed language toolchains, **When** `./scripts/run_all_sdk_tests.sh` is invoked, **Then** each SDK executes its test suite and emits standard JUnit/TAP results.
2. **Given** a missing optional SDK toolchain (e.g. Dart or .NET), **When** the test runner executes, **Then** it cleanly marks the missing runner as SKIPPED without failing the overall suite.

---

### User Story 3 - C/C++ Header Synchronization & Modern CMake/pkg-config Integration (Priority: P3)

C and C++ native systems engineers can link `libttzip_glue` into external applications using modern `find_package(TTZip REQUIRED)` or `pkg-config --cflags --libs ttzip` with complete header declarations (`ttzip.h`, `ttzip.hpp`).

**Why this priority**: Standardizes system packaging across Homebrew, Linux distributions, and embedded integrations.

**Independent Test**: Compile and run `sdk/c/test_c_sdk.c` and `sdk/cpp/test_cpp_sdk.cpp` using the generated `ttzip.pc` and CMake config.

**Acceptance Scenarios**:

1. **Given** a system with `ttzip.pc` generated in `PKG_CONFIG_PATH`, **When** compiling a C consumer program, **Then** linking succeeds and compression/decompression operations return `TTZIP_STATUS_OK`.
2. **Given** a CMake project with `find_package(TTZip)`, **When** target `TTZip::Core` is linked, **Then** all exported C11 and C++20 helper methods compile with zero undefined symbols.

---

### User Story 4 - Local CI/CD Gate Consolidation & LOC Defense Hardening (Priority: P4)

Release engineers running `./scripts/run_local_ci_gate.sh` verify the full 4-stage pipeline (LOC defense, Swift high-level facade, 50-point benchmark gate, and Rust industrial test suite) with zero manual environment interventions.

**Why this priority**: Protects architectural boundaries and prevents performance regressions before commits are created.

**Acceptance Scenarios**:

1. **Given** the repository workspace, **When** `./scripts/run_local_ci_gate.sh --json reports/gate.json` is executed, **Then** a structured report is generated showing all stages passing.

---

### Edge Cases

- **Missing External Runtime**: When Python `maturin` or Java `javac` is not installed, SDK runners must gracefully detect toolchain absence and emit informative skip logs.
- **Deeply Nested File Trees**: Archive extraction and path sanitization must handle paths exceeding 4096 characters with `TTZIP_STATUS_ERR_PATH_TOO_LONG` or safe truncation.
- **Malformed Memory Buffers**: C-ABI and Rust FFI entry points must catch panics (`catch_unwind`) and return `TTZIP_STATUS_ERR_PANIC_CAUGHT` rather than aborting host process.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST enforce SPDX Dual License (`BSD-3-Clause OR Apache-2.0`) headers on all 146 currently untagged Swift source files in `Sources/TTZipApp/`.
- **FR-002**: System MUST maintain the Single-File LOC Defense Gate ensuring no source file in `Sources/` or `rust/` exceeds 800 LOC.
- **FR-003**: System MUST provide automated execution of all multilingual SDK tests (C, C++, Python, Node, Dart, Java, .NET) via `scripts/run_all_sdk_tests.sh`.
- **FR-004**: System MUST keep `Sources/CTTZipBridge/include/ttzip.h` and `ttzip.hpp` in 100% binary interface alignment with `ttzip_rust_glue.h`.
- **FR-005**: System MUST provide deterministic `pkg-config` generation script (`scripts/generate_pkg_config.sh`) supporting standard installation prefixes.
- **FR-006**: System MUST pass all 4 stages of `./scripts/run_local_ci_gate.sh` with zero compiler warnings under `-warnings-as-errors`.
- **FR-007**: System MUST maintain zero plain-text credential retention in memory buffers following cryptographic operations (`zeroize` / `memset_s`).

---

### Key Entities

- **AuditReport**: Structured audit metadata recording file counts, LOC breakdown, license compliance, invariant adherence, and test results.
- **CodeQualityGate**: Configurable gate definition verifying single-file LOC thresholds, SPDX headers, memory invariants, and benchmark floors.
- **MultilingualSdkSuite**: Matrix definition mapping client languages to compiler commands, test entry points, and validation assertions.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of source files in `Sources/`, `Tests/`, and `scripts/` contain valid SPDX license headers (0 missing files reported by `lint_codebase_standards.sh`).
- **SC-002**: 100% of 776+ source files remain strictly below the 800 LOC ceiling with an average file length < 250 LOC.
- **SC-003**: 100% pass rate across local regression and quality gate stages in `./scripts/run_local_ci_gate.sh`.
- **SC-004**: Multilingual SDK test runner executes with 0 unhandled failures across all available language runtimes.
- **SC-005**: Zero compiler warnings across all Swift and Rust build targets under strict warning-as-error compiler flags.

---

## Assumptions

- Target build platform is macOS 14.0+ (Apple Silicon ARM64 primary, Intel x86_64 compatible).
- Rust toolchain (edition 2021) and Swift 6 toolchain are installed locally.
- Optional multilingual SDK toolchains (Python, Node.js, Java, .NET, Dart) are tested when available and skipped gracefully when absent.
- The project enforces offline-first local CI validation without reliance on cloud continuous integration runners.
