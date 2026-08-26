# Implementation Plan: Comprehensive Codebase Architecture & Quality Audit Remediation

**Branch**: `220-comprehensive-codebase-and-quality-audit` | **Date**: 2026-08-24 | **Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/220-comprehensive-codebase-and-quality-audit/spec.md)

**Input**: Feature specification from `specs/220-comprehensive-codebase-and-quality-audit/spec.md`

---

## Summary

Remediate identified quality latch debts across the TTZip codebase:
1. Programmatically inject standard SPDX dual-license headers (`BSD-3-Clause OR Apache-2.0`) across all 146 Swift UI and service files in `Sources/TTZipApp/`.
2. Re-synchronize C-ABI and C++ headers (`ttzip.h`, `ttzip.hpp`) with `ttzip_rust_glue.h`.
3. Harden the multilingual SDK test runner (`scripts/run_all_sdk_tests.sh`) with toolchain auto-discovery.
4. Verify all 4 stages of `scripts/run_local_ci_gate.sh` pass with zero warnings under `-warnings-as-errors`.

---

## Technical Context

**Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`), Rust (edition 2021), C11 / C++20, Python 3.10+, Node.js 18+  
**Primary Dependencies**: Safe Rust microkernel (`ttzip-engine`, `ttzip-glue`), AppKit / SwiftUI, Sparkle v2.6.0  
**Storage**: APFS filesystem, macOS Keychain (`PasswordVaultManager`), in-memory VFS  
**Testing**: `swift test`, `cargo test`, `scripts/run_local_ci_gate.sh`, `scripts/run_all_sdk_tests.sh`  
**Target Platform**: macOS 14.0+ (Apple Silicon ARM64 NEON primary, x86_64 compatible)  
**Project Type**: Native macOS desktop app + POSIX CLI + Safe Rust microkernel + Multilingual client SDKs  
**Performance Goals**: Hard throughput floors (ZIP Decompression >= 4500 MB/s, AES-256 >= 1800 MB/s)  
**Constraints**: Zero compiler warnings, single-file LOC <= 800, zero plain-text credential persistence  
**Scale/Scope**: 910 source files across 158,711 lines of code  

---

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Invariant / Gate | Status | Evidence / Verification |
| :--- | :--- | :--- |
| **I. Stream-First** | PASS | Micro-buffering pipeline (16KB to 1MB chunks), 0 monolithic allocations |
| **II. Invariant-First** | PASS | Secure extraction flags, O_NOFOLLOW verification, overflow builtins |
| **III. Bounds-First** | PASS | `zeroize` on credentials, SSIZE_MAX clamping, catch_unwind FFI boundaries |
| **IV. Oracle-First** | PASS | Golden Corpus fixtures, differential tests vs system unzip/tar |
| **Single-File LOC Gate** | PASS | 100% of 776 measured source files <= 800 LOC |
| **Zero Warnings Policy** | PASS | -warnings-as-errors enforced across all compilation targets |

---

## Project Structure

### Documentation (this feature)

```text
specs/220-comprehensive-codebase-and-quality-audit/
├── spec.md              # Feature specification
├── plan.md              # This implementation plan
├── research.md          # Phase 0 technical research and decisions
├── data-model.md        # Phase 1 data entities and schemas
├── quickstart.md        # Phase 1 verification and execution guide
├── checklists/
│   └── requirements.md  # Quality checklist
├── contracts/
│   ├── ci-gate-contract.json
│   ├── sdk-test-matrix.json
│   └── c-abi-signatures.h
└── tasks.md             # Phase 2 output (/speckit-tasks command)
```

### Source Code Targets

```text
Sources/
├── TTZipApp/            # 146 Swift files to receive SPDX dual-license headers
├── TTZipCore/           # Swift 6 domain facade and pipeline orchestration
├── CTTZipBridge/        # C11 ABI and C++20 header synchronization
└── TTZipBench/          # Microbenchmarking runner

rust/
├── ttzip-engine/        # Pure Safe Rust core microkernel (#![forbid(unsafe_code)])
├── ttzip-glue/          # C-ABI export layer with #[no_mangle] extern "C"
├── ttzip-tui/           # Standalone interactive TUI + CLI
└── ttzip-python/        # PyO3 Python native bindings

scripts/
├── lint_loc_gate.py     # Single-file LOC defense gate (<= 800 LOC)
├── lint_codebase_standards.sh # SPDX & ASCII validation latch
├── run_all_sdk_tests.sh # Multilingual SDK test runner
└── run_local_ci_gate.sh # Composite 4-stage regression gate
```

---

## Complexity Tracking

*No constitution violations or unjustified architectural complexities detected.*
