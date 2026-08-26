# Implementation Plan: 068-codebase-copyright-and-english-internationalization

## Technical Context
TTZip contains ~474 source files across `Sources/` (Core, C Bridge, App, CLI) and `Tests/`. This plan establishes the automated pipeline to prepend standard SPDX BSD-3-Clause copyright headers and translate all Chinese comments, docstrings, MARK markers, and CLI strings into high-quality professional English.

---

## Constitution Check
- [x] **Zero Runtime Overhead**: Comments and headers are purely compile-time constructs with zero impact on machine code or throughput.
- [x] **Zero Syntax Regressions**: All code structures, variable names, and logic flows remain 100% untouched.
- [x] **Full Test Suite Pass**: All 520+ unit tests continue to pass with 0 failures.

---

## Phase 0: Research Items
- R001: SPDX License Identifier & Copyright Header Standard (`research.md`)
- R002: High-Fidelity Comment & String English Translation Pipeline (`research.md`)
- R003: Zero-Chinese Static Assertion & Gate Verification (`research.md`)

---

## Phase 1: Design & Documentation Artifacts
- **Data Model**: `specs/068-codebase-copyright-and-english-internationalization/data-model.md`
- **Contracts**: `specs/068-codebase-copyright-and-english-internationalization/contracts/translation-manifest.schema.json`
- **Quickstart Guide**: `specs/068-codebase-copyright-and-english-internationalization/quickstart.md`

---

## Component Change List
1. **Header Injection & Comment Transformation Engine**:
   - `scripts/internationalize_codebase.py`: Python script performing header injection and high-fidelity English comment translation across `Sources/` and `Tests/`.
2. **Static Assertion Gate**:
   - `scripts/assert_zero_chinese.py`: Verification tool verifying 0 Chinese characters remain in comments.
3. **Source & Test Codebase**:
   - All `.swift`, `.c`, `.h` files under `Sources/` and `Tests/`.
