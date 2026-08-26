# Feature Specification: Engineering Governance and Quality Gates Hardening

**Feature Branch**: `222-engineering-governance-and-quality-gates-hardening`  
**Created**: 2026-08-24  
**Status**: Draft  
**Pipeline Mode**: `[Full SDD]`  
**Input**: Comprehensive post-mortem remediation and engineering governance hardening across development, architecture, and testing pipelines.

---

## 1. Executive Summary & Objective

Based on the architectural audit and post-mortem reflection, this feature establishes long-term systemic defenses and automated CI/CD quality gates for TTZip to permanently prevent:
1. **Cross-Language Boundary Drift**: Missing C-ABI exports, unchecked compiler flags, and static library symbol dropouts.
2. **Deceptive "Defensive" Anti-Patterns**: Full-directory backup copying instead of Write-Ahead differential journaling, physical disk temp-file concatenation for split archives, and uncontrolled heap allocations for sensitive bytes.
3. **Happy-Path Testing Gaps**: Missing multi-volume split archive stress tests, lock contention profiling, and memory sanitization (ASan/TSan) coverage.

---

## 2. User Scenarios & Testing *(Prioritized)*

### User Story 1 - Automated C-ABI & Static Library Symbol Gate (Priority: P1)

As a systems engineer or contributor, I need continuous integration to deterministically verify that all exported Rust C-ABI symbols in headers match the actual symbols present in `libTTZipVendor.a` and linked upstream codecs, preventing runtime unresolved symbol failures and dynamic link errors.

**Why this priority**:
Eliminates the class of defects where build scripts complete with exit code 0 but miss static codec symbols (`_libdeflate_adler32`, `_lzvn_encode_buffer`, `_zstd_compress`).

**Independent Test**:
Run `./scripts/verify_cabi_symbols.sh` to cross-validate header declarations against `nm -gU Vendor/TTZipVendor.xcframework/macos-arm64/libTTZipVendor.a`.

**Acceptance Scenarios**:
1. **Given** a new C-ABI function declared in `ttzip_rust_glue.h`, **When** the symbol is compiled and exported in Rust, **Then** the symbol verification gate passes with 0 missing symbols.
2. **Given** a header declaring an unexported or stripped symbol, **When** `./scripts/verify_cabi_symbols.sh` is executed, **Then** the script exits with non-zero code and lists the exact missing symbol name.

---

### User Story 2 - Real-World Large-Scale & Split Volume Verification Suite (Priority: P1)

As a user extracting massive archives (100GB+ / 100,000+ files) or multi-volume archives (`.001`, `.002`, `.z01`), I need operations to execute with zero intermediate temporary disk copies, differential failure rollbacks, and strictly bounded memory consumption.

**Why this priority**:
Prevents disk exhaustion on differential extraction and ensures `VirtualMultiVolumeReader` behaves reliably across boundary rollovers.

**Independent Test**:
Run high-volume automated integration tests creating and extracting synthetic 10,000-entry trees and multi-volume archives under physical disk accounting asserts.

**Acceptance Scenarios**:
1. **Given** an extraction targeting a non-empty directory that encounters a mid-flight error, **When** rollback is triggered, **Then** only newly created files are removed and pre-existing files remain untouched with zero prior full-directory copies.
2. **Given** a 3-part split archive (`.001`, `.002`, `.003`), **When** reading the catalog and extracting arbitrary single entries, **Then** zero bytes are written to `/tmp` or disk before the final destination file.

---

### User Story 3 - Sensitive Memory Sanitizer & Lifetime Hardening Gate (Priority: P2)

As a security-conscious user decrypting or encrypting sensitive archives, I need all key material to reside strictly in locked physical memory pages (`mlock`), zeroed on deallocation, with zero heap strings or Array copies in Swift ARC.

**Why this priority**:
Prevents passphrase retention in swap or core dumps.

**Independent Test**:
Run `VaultMemorySanitizationTests` and AddressSanitizer (ASan) runs verifying zero memory leakage and zero heap residue.

**Acceptance Scenarios**:
1. **Given** a `SecureBytes` instance created from a UTF-8 string, **When** inspecting allocations, **Then** memory is allocated exclusively via `posix_memalign` and locked with `mlock`, and wiped with zeroize upon `deinit`.

---

## 3. Requirements

### Functional Requirements

- **FR-001**: System MUST provide `./scripts/verify_cabi_symbols.sh` to validate 100% symbol parity between C-ABI headers and `libTTZipVendor.a`.
- **FR-002**: System MUST enforce Write-Ahead Journaling in `DifferentialExtractTransaction` such that rollback time and space complexity is strictly $O(\Delta)$ where $\Delta$ is the set of extracted files.
- **FR-003**: System MUST enforce zero temporary disk file creation when opening and inspecting split archives (`.001`, `.z01`, `.7z.001`).
- **FR-004**: System MUST maintain zero heap string allocations during `SecureBytes` initialization and destruction.
- **FR-005**: System MUST maintain stable pointer identity for `NSOutlineViewDataSource` items using `ArchiveOutlineItem: NSObject`.
- **FR-006**: System MUST integrate ASan/TSan-compatible test harness scripts in `./scripts/run_sanitizers.sh`.

### Key Technical Invariants

1. **SSOT Build Invariant**: Native codecs (`libdeflate`, `zstd`, `lz4`, `fast-lzma2`, `lzfse`) are compiled directly by `build.rs` from `Vendor/` into `libttzip_native_codecs.a` without external cmake/make dependencies.
2. **Zero-Boxing UI Invariant**: `NSOutlineView` adapters must never receive unboxed Swift structs as `item` parameters.
3. **No-Lock Path Invariant**: File path metadata mapping must never use global locks for unique path strings; only small-cardinality static tables are permitted.

---

## 4. Clarifications & Architecture Decisions

- **Q1: How should `verify_cabi_symbols.sh` extract required C symbols?**
  - **Decision**: Extract all `extern "C"` / `pub extern "C" fn` definitions in `Sources/CTTZipBridge/include/ttzip_rust_glue.h` and use `nm -gU` to assert their presence in `Vendor/TTZipVendor.xcframework/macos-arm64/libTTZipVendor.a`.
- **Q2: Where does the C-ABI verification gate fit in the CI pipeline?**
  - **Decision**: Integrated directly as Stage 1.5 in `./scripts/run_local_ci_gate.sh` and pre-push hook.
- **Q3: How are sanitizers (ASan/TSan) run without slowing down standard pre-push gates?**
  - **Decision**: Standalone script `./scripts/run_sanitizers.sh` with dedicated `--asan` and `--tsan` flags, invoked on-demand and in nightly CI jobs.

