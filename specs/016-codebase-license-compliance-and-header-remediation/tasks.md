# Tasks: Codebase License Compliance & Header Remediation

**Feature**: `016-codebase-license-compliance-and-header-remediation`  
**Classification**: `[Lean SDD]`  
**Status**: `COMPLETED`  

---

## Task Breakdown

- [x] **Task 1: Build Deterministic Header Cleaner Engine**
  - Path: `core/scripts/clean_license_headers.py`
  - Implemented AST/Regex stripper that safely extracts shebangs, removes stacked/duplicated legacy license headers, and standardizes to clean 5-line headers per directory tier.
  - Dependencies: None

- [x] **Task 2: Execute Global Header Cleansing Across All Tiers**
  - Remediated 938 active files across `core/`, `apple/`, `homebrew/`, `tests/`, and `scripts/`.
  - Cleaned all 232 double-SPDX stacked header files.
  - Cleaned all 754 duplicate-copyright files.
  - Standardized `core/rust`, `core/Sources/CTTZipBridge`, `core/Sources/TTZipCore`, `core/sdk`, `homebrew` to `BSD-3-Clause OR Apache-2.0`.
  - Standardized `apple/Sources`, `core/Sources/TTZipApp`, `core/Sources/TTZipFinderSync`, `core/Sources/TTZipQuickLook`, `core/Tests/TTZipAppTests` to `GPL-3.0-or-later`.
  - Dependencies: Task 1

- [x] **Task 3: Harden CI Linter Gates & Audit Scripts**
  - Upgraded `core/scripts/audit_licenses.py` to scan the first 25 lines of each file, asserting:
    - Exactly 1 `SPDX-License-Identifier` per proprietary file.
    - Exactly 1 `Copyright` notice per proprietary file.
    - Strict verification of license tier against directory structure.
  - Upgraded `core/scripts/lint_codebase_standards.sh` to run `audit_licenses.py` and `clean_license_headers.py --check`.
  - Replaced `core/scripts/inject_spdx_headers.py` with safe forwarding wrapper.
  - Dependencies: Task 2

- [x] **Task 4: Synchronize Package Manifests & Top-Level License**
  - Synchronized `core/Formula/ttzip.rb` with `homebrew/Formula/ttzip.rb` (`license any_of: ["BSD-3-Clause", "Apache-2.0"]`).
  - Created root `LICENSE` file articulating the tiered dual-license architecture.
  - Dependencies: Task 2

- [x] **Task 5: End-to-End Gate Verification & Regression Test**
  - Ran `python3 core/scripts/audit_licenses.py` (100% pass across all directories).
  - Ran `python3 core/scripts/clean_license_headers.py --check` (100% clean across `.`, `core`, `apple`, `homebrew`, `tests`).
  - Ran Rust crate test suite (`ttzip-engine` and `ttzip-tui` unit & integration tests: 100% pass).
  - Dependencies: Task 1, 2, 3, 4
