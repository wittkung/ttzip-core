# Implementation Plan: Feature 132 - Full Codebase License & IP Compliance Audit

## Technical Context
- **Project**: TTZip Codebase & Dependency Ecosystem
- **Target Subsystems**:
  1. `scripts/audit_licenses.py` - Automated full-codebase header scanner and copyleft classifier.
  2. `scripts/generate_acknowledgements.py` - Dynamic FOSS notice harvester and documentation generator.
  3. `docs/THIRD_PARTY_LICENSES.md` - Comprehensive third-party legal attribution document.
  4. `LICENSE` - Verified 5-section `TTZip-SAL-1.0` root license.

## Constitution Check
- Strict compliance with `.specify/memory/constitution.md`.
- 100% Zero bare object contract schema.
- Full verification across all 600+ source files.

## Phase 0: Research & Architecture Decisions
- Completed in `research.md` (R001: SPDX custom LicenseRef, R002: Notice harvesting, R003: uchardet MPL 1.1 boundary, R004: 5-section root license).

## Phase 1: Data Model & Contracts
- Completed in `data-model.md` and `contracts/license-audit.schema.json`.

## Phase 2: Implementation Breakdown
- **Component 1**: Automated License & SPDX Header Auditor (`scripts/audit_licenses.py`)
- **Component 2**: Third-Party FOSS Acknowledgements Harvester (`scripts/generate_acknowledgements.py`)
- **Component 3**: Third-Party License Documentation (`docs/THIRD_PARTY_LICENSES.md`)
