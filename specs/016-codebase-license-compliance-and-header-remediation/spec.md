# Feature Specification: Codebase License Compliance & Header Remediation

**Feature**: `016-codebase-license-compliance-and-header-remediation`  
**Classification**: `[Lean SDD]` (Comprehensive codebase license header deduplication, tiered license alignment, and CI gate hardening)  
**Status**: `SPECIFIED`  

---

## 1. Context & Problem Statement

A thorough automated audit of the repository revealed severe license compliance and header integrity issues:
1. **Duplicate Header Stacking**: 232+ source files contain conflicting duplicate `SPDX-License-Identifier` blocks (notably `BSD-3-Clause OR Apache-2.0` prepended directly above `GPL-3.0-or-later`), caused by an older script (`core/scripts/inject_spdx_headers.py`) blindly prepending headers without cleaning prior header comments.
2. **Duplicated Copyright Notices**: 754+ files have duplicate `Copyright (c) 2026 Witt Kung` notices.
3. **Tiered License Alignment**: In accordance with the project architecture (`specs/215-multiplatform-sdk-and-dual-license-architecture`):
   - **Infrastructure & SDK Tier** (`core/rust/*`, `core/Sources/CTTZipBridge/*`, `core/Sources/TTZipCore/*`, `core/sdk/*`, `homebrew/*`): Dual-licensed under `BSD-3-Clause OR Apache-2.0`.
   - **Client Applications Tier** (`apple/Sources/*`, `core/Sources/TTZipApp/*`, `core/Sources/TTZipFinderSync/*`, `core/Sources/TTZipQuickLook/*`): Licensed under `GPL-3.0-or-later`.
   - **Third-Party Vendors** (`core/Vendor/*`, `core/Sources/CTTZipBridge/snappy|zopfli|fast-lzma2|lzfse`): Retain original upstream permissive licenses (MIT, BSD-2/3, Apache-2.0, 0BSD).
4. **Tooling & Gate Hardening**: `core/scripts/audit_licenses.py` and `core/scripts/lint_codebase_standards.sh` had detection blind spots (only checking first 5 lines or matching substring presence), allowing double headers to pass.
5. **Manifest & Formula Inconsistencies**: `core/Formula/ttzip.rb` vs `homebrew/Formula/ttzip.rb` and root `LICENSE` file declarations need absolute synchronization.

---

## 2. Scope & Requirements

### 2.1 Remediation Script Engine (`clean_license_headers.py`)
- Implement an AST/Regex-based deterministic script that:
  - Detects all existing license/SPDX/copyright comment blocks at the top of files (Swift, C, H, Rust, Python, Go, Java, TypeScript, Shell).
  - Completely strips legacy, duplicate, or stacked headers.
  - Injects the exact, standard 5-line header appropriate for the module tier.
  - Preserves shebangs (`#!/usr/bin/env ...`) on scripts.
  - Strictly ignores third-party embedded directories in `Vendor/` and third-party C bridges.

### 2.2 Global File Cleanup
- Execute clean remediation across all 232 double-SPDX files and 754 duplicate-copyright files.
- Ensure 0 files have multiple SPDX identifiers or multiple copyright blocks.

### 2.3 Linter & CI Gate Hardening
- Update `core/scripts/audit_licenses.py` to:
  - Read first 50 lines of each file.
  - Assert that `len(re.findall(r"SPDX-License-Identifier", head)) == 1`.
  - Assert that `len(re.findall(r"Copyright", head)) == 1`.
  - Verify that the SPDX identifier matches the expected tier for that directory.
- Update `core/scripts/lint_codebase_standards.sh` to enforce header uniqueness.
- Fix any self-referencing duplicates in `scripts/` (e.g. `install_local_git_hooks.sh`, `inject_spdx_headers.py`).

### 2.4 Manifest Synchronization
- Synchronize `core/Formula/ttzip.rb` with `homebrew/Formula/ttzip.rb`.
- Ensure `Cargo.toml`, `Package.swift`, `pyproject.toml`, `package.json` accurately reflect the respective module license.
- Ensure top-level `LICENSE` exists and documents the tiered licensing structure.

---

## 3. Success Criteria

1. **Zero Double Headers**: `count(files with >1 SPDX identifier) == 0`.
2. **Zero Duplicate Copyrights**: `count(files with >1 Copyright notice) == 0`.
3. **100% CI Gate Pass**: `python3 core/scripts/audit_licenses.py` and `bash core/scripts/lint_codebase_standards.sh` exit with code 0 without bypass.
4. **Zero Compilation Regressions**: All Rust crates, Swift packages, and SDK tests compile and pass.
