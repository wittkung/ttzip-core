# Tasks: Multiplatform SDK, Dual-Licensing & Repository Topology Architecture

**Feature**: `215-multiplatform-sdk-and-dual-license-architecture`  
**Directory**: `specs/215-multiplatform-sdk-and-dual-license-architecture`  
**Spec Path**: `specs/215-multiplatform-sdk-and-dual-license-architecture/spec.md`  
**Plan Path**: `specs/215-multiplatform-sdk-and-dual-license-architecture/plan.md`  

---

## Phase 0: Legal Migration, Upstream Governance & SPDX Replacement

**Purpose**: Establish `BSD-3-Clause OR Apache-2.0` on core engine/SDK, `GPL-3.0-or-later` on client UI applications, and execute upstream license harvesting.

- [x] T001 [P] Create root `LICENSE-BSD` containing standard BSD 3-Clause text with Witt Kung / TTZip Authors copyright.
- [x] T002 [P] Create root `LICENSE-APACHE` containing standard Apache License Version 2.0 text.
- [x] T003 [P] Create root `LICENSE-GPL` containing GNU General Public License Version 3 text for UI applications.
- [x] T004 [P] Create root `NOTICE` file specifying project origin, copyright attribution, and patent/trademark protections.
- [x] T005 [P] Run `python3 scripts/generate_acknowledgements.py` to harvest all third-party licenses into `docs/THIRD_PARTY_LICENSES.md` and `ACKNOWLEDGEMENTS.md`.
- [x] T006 Batch replace SPDX headers:
  - Core files (`rust/`, `Sources/CTTZipBridge/`, `Sources/TTZipCore/`): `SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0`
  - UI files (`Sources/TTZipApp/`, `Sources/TTZipQuickLook/`, `Sources/TTZipFinderSync/`): `SPDX-License-Identifier: GPL-3.0-or-later`
- [x] T007 Update `Package.swift`, `rust/Cargo.toml`, `rust/ttzip-glue/Cargo.toml`, and `README.md` to declare official license metadata.

---

## Phase 1: Rust Workspace 3-Crate Restructuring

**Purpose**: Establish a pure algorithm crate (`ttzip-engine`), thin C-ABI export layer (`ttzip-glue`), and standalone CLI (`ttzip-cli`).

- [x] T008 [P] [US1] Create `rust/ttzip-engine/` crate for pure algorithm logic (`rlib`), moving format kernels, SIMD detection, and crypto routines out of `ttzip-glue`.
- [x] T009 [US1] Update `rust/ttzip-glue/` to consume `ttzip-engine` as a dependency, retaining solely C-ABI FFI bindings, zero-copy buffer views, and `catch_unwind` wrappers.
- [x] T010 [US1] Rename `rust/ttzip-tui` to `rust/ttzip-cli`, updating `rust/Cargo.toml` workspace members and setting binary name to `ttzip`.

**Checkpoint**: `cargo check --workspace` passes cleanly across all crates.

---

## Phase 2: Public C-ABI Subset (`ttzip.h`) Definition

**Purpose**: Provide a clean, minimal public C11 SDK header ($\le 100\text{ LOC}$) while preserving internal `ttzip_rust_glue.h`.

- [x] T011 [P] Create `Sources/CTTZipBridge/include/ttzip.h` exporting `ttzip_create_archive`, `ttzip_extract_archive`, `ttzip_inspect_archive`, `ttzip_compress_buffer`, `ttzip_decompress_buffer`, and versioning helpers.
- [x] T012 [P] Update `Sources/CTTZipBridge/include/module.modulemap` to export `ttzip.h` alongside `ttzip_rust_glue.h` and `CTTZipBridge.h`.
- [x] T013 Verify all exported FFI entry points in `ttzip-glue` are wrapped with `std::panic::catch_unwind` returning `TTZipStatus`.

---

## Phase 3: Swift 6 SDK Consolidation (`TTZipCore`)

**Purpose**: Validate Swift 6 Strict Concurrency SDK facade consuming the C-ABI with `AsyncThrowingStream` progress.

- [x] T014 [P] [US2] Verify `ArchiveProgress`, `ArchiveCompressionOptions`, and `ArchiveEntryMetadata` conform to `Sendable`.
- [x] T015 [US2] Verify `ArchiveExtractor` and `ArchiveWriter` async progress pipelines and cooperative task cancellation.
- [x] T016 [US2] Run `swift test` ensuring 100% of core test suites pass without regressions.

---

## Phase 4: Standalone CLI `--json` NDJSON Streaming & Shell Completions

**Purpose**: Enhance the 19 existing CLI subcommands with machine-readable NDJSON streaming and auto-completion scripts.

- [x] T017 [P] [US3] In `rust/ttzip-cli/`, implement `--json` argument parsing emitting NDJSON progress and completion event objects.
- [x] T018 [US3] Implement shell completion generation for `bash`, `zsh`, `fish`, and `powershell`.
- [x] T019 [US3] Run CLI integration tests verifying stdout/stderr formatting and exit codes.

---

## Phase 5: Zero-Cloud-Cost Local CI/CD & Git Hook Hardening

**Purpose**: Enforce 100% local verification with 0 cloud runner minutes used.

- [x] T020 [P] [US4] Update `scripts/install_local_git_hooks.sh` to install `.git/hooks/pre-push` and `.git/hooks/pre-commit`.
- [x] T021 [US4] Verify `scripts/run_local_ci_gate.sh` passes all 4 stages locally (LOC gate $\le 800$, Swift facade, Deflate bench, Rust industrial).
- [x] T022 [US4] Execute `scripts/lint_loc_gate.sh` to enforce single-file $\le 800\text{ LOC}$ ceiling across all modified files.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [x] T023 [P] Run `python3 scripts/audit_licenses.py` to ensure zero unlicensed or conflicting dependencies.
- [x] T024 Execute full local gate `./scripts/run_local_ci_gate.sh --bail` ensuring 100% green status.
