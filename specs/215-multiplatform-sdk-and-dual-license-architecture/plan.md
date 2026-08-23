# Implementation Plan: Multiplatform SDK, Dual-Licensing & Repository Topology Architecture

**Feature**: `215-multiplatform-sdk-and-dual-license-architecture`  
**Classification**: `[Full SDD]`  
**Status**: `PLANNED` (Revised for upstream governance & ttzip-apple)  
**Spec Path**: `specs/215-multiplatform-sdk-and-dual-license-architecture/spec.md`  

---

## 1. Technical Context

- **Runtime & Languages**: Safe Rust (2021 edition) + Swift 6.0 Strict Concurrency + C11 ABI.
- **Rust Workspace Layout**:
  - `rust/ttzip-engine` (Pure algorithm kernel, `#![forbid(unsafe_code)]`, `rlib`).
  - `rust/ttzip-glue` (C-ABI FFI wrapper with `catch_unwind`, `staticlib`/`cdylib`).
  - `rust/ttzip-cli` (19 subcommands + Ratatui TUI, binary `ttzip`).
- **C Bridge Layer**:
  - `Sources/CTTZipBridge/include/ttzip.h` (Public C11 SDK subset, $\le 100\text{ LOC}$).
  - `Sources/CTTZipBridge/include/ttzip_rust_glue.h` (Internal bridge, 550 LOC).
- **Swift Framework**: `Sources/TTZipCore` (Package.swift product `TTZipCore`).
- **Client Application**: `ttzip-apple` (macOS AppKit/SwiftUI + QuickLook + FinderSync + future iOS/iPadOS).
- **Licensing Tier**:
  - `ttzip-core`: `BSD-3-Clause OR Apache-2.0` (Permissive Dual-License for SDK/CLI).
  - `ttzip-apple`: `GPL-3.0-or-later` (Copyleft protection for end-user GUI application).
- **Upstream Governance**: Automated harvesting via `scripts/generate_acknowledgements.py` and pre-push audit via `scripts/audit_licenses.py`.
- **CI/CD Constraint**: 100% Zero-Cloud-Cost local verification via `.git/hooks/pre-push` and `scripts/run_local_ci_gate.sh`.

---

## 2. Constitution & Invariant Checks

- [x] **1. Safe Rust Single Source of Truth**: All core algorithms reside in `ttzip-engine`; client SDKs are thin idiomatic bindings.
- [x] **2. Zero Panic Escape**: All C-ABI FFI exported functions in `ttzip-glue` are wrapped in `catch_unwind`.
- [x] **3. Single-File LOC Gate**: Every source file must satisfy $\le 800\text{ LOC}$.
- [x] **4. Tiered & Upstream Licensing Compliance**: Standard `LICENSE-BSD`, `LICENSE-APACHE`, `LICENSE-GPL`, `NOTICE`, `ACKNOWLEDGEMENTS.md`, and `docs/THIRD_PARTY_LICENSES.md` must be maintained.
- [x] **5. Zero-Cloud Local CI**: All test gates pass locally via pre-push hook without cloud runner usage.

---

## 3. Phased Execution Roadmap

### Phase 0: Legal Migration & SPDX Batch Replacement
- [ ] Create root `LICENSE-BSD`, `LICENSE-APACHE`, `LICENSE-GPL`, and `NOTICE`.
- [ ] Execute `scripts/generate_acknowledgements.py` to refresh all upstream license manifests.
- [ ] Batch replace SPDX headers:
  - Core SDK & Rust files: `SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0`.
  - Client UI files (`Sources/TTZipApp/`): `SPDX-License-Identifier: GPL-3.0-or-later`.
- [ ] Update `Cargo.toml`, `Package.swift`, and `README.md` license declarations.

### Phase 1: Rust Workspace 3-Crate Restructuring
- [ ] Structure `rust/ttzip-engine` as the pure algorithm crate (`rlib`).
- [ ] Retain `rust/ttzip-glue` as the C-ABI FFI thin export layer (`staticlib`/`cdylib`).
- [ ] Rename `rust/ttzip-tui` to `rust/ttzip-cli`, setting binary name to `ttzip`.

### Phase 2: Public C-ABI Subset (`ttzip.h`) Definition
- [ ] Create `Sources/CTTZipBridge/include/ttzip.h` as a clean public C11 subset ($\le 100\text{ LOC}$).
- [ ] Verify `Sources/CTTZipBridge/include/module.modulemap` exports both headers cleanly.

### Phase 3: Swift 6 SDK Consolidation (`TTZipCore`)
- [ ] Ensure `TTZipCore` builds cleanly under Swift 6 Strict Concurrency with `AsyncThrowingStream` progress pipelines.
- [ ] Verify all Swift unit tests pass (`swift test`).

### Phase 4: Standalone CLI `--json` NDJSON Streaming & Shell Completions
- [ ] Add `--json` streaming NDJSON progress output to `ttzip-cli`.
- [ ] Generate automated shell completion scripts for `bash`, `zsh`, `fish`, and `powershell`.

### Phase 5: Zero-Cloud-Cost Local CI/CD & Git Hook Hardening
- [ ] Verify `.git/hooks/pre-push` triggers `scripts/run_local_ci_gate.sh`.
- [ ] Enforce single-file $\le 800\text{ LOC}$ ceiling across all modified files.

---

## 4. Verification Plan

1. **Local CI Regression Gate**:
   ```bash
   ./scripts/run_local_ci_gate.sh --bail
   ```
2. **Single-File LOC Audit**:
   ```bash
   ./scripts/lint_loc_gate.sh
   ```
3. **Rust Workspace Conformance**:
   ```bash
   cargo test --manifest-path rust/Cargo.toml --all-targets
   ```
4. **License Audit**:
   ```bash
   python3 scripts/audit_licenses.py
   ```
