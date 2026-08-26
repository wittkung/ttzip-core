# Tasks: Feature 132 - Full Codebase License & IP Compliance Audit

## Phase 1: Implementation & Tooling

- [x] T001 [P] [US1] Create automated license & SPDX header auditor in `scripts/audit_licenses.py`
- [x] T002 [P] [US2] Create automated third-party license harvester in `scripts/generate_acknowledgements.py`
- [x] T003 [US2] Generate comprehensive third-party attribution document in `docs/THIRD_PARTY_LICENSES.md`
- [x] T004 [US3] Implement copyleft & GPL static linking scanner in `scripts/audit_licenses.py`
- [x] T005 [US4] Audit and verify root `LICENSE` against SPDX naming and 5-section protection

## Phase 2: Verification & Codebase Sweep

- [x] T006 [US1] Execute full-codebase scan across all 600+ source files in `Sources/` and fix any missing headers
- [x] T007 [US2] Verify generated `docs/THIRD_PARTY_LICENSES.md` for complete verbatim upstream notices
- [x] T008 [US3] Verify zero viral copyleft static linkage across `Package.swift`
