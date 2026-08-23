# Feature Specification: Full Codebase License, IP Compliance & Attribution Audit

**Feature Directory**: `specs/132-full-codebase-license-and-ip-compliance-audit`
**Created**: 2026-08-19
**Status**: Draft
**Input**: User description: "全面审计我们的 license /speckit-specify"

---

## 1. Executive Summary & Objective

TTZip is a high-performance compression system combining proprietary frontend/engine architecture (`TTZip-SAL-1.0`) with permissive upstream open-source engines (`libdeflate`, `zlib-ng`, `libarchive`, `zstd`, `fast-lzma2`). 

To ensure complete legal compliance, protect author intellectual property, and guarantee zero copyleft (GPL/AGPL) contamination across binary distributions (Mac App Store & Homebrew Tap), this feature executes an exhaustive, automated 360-degree license and attribution audit across the entire codebase.

---

## 2. User Scenarios & Testing *(mandatory)*

### User Story 1 - Full-Codebase SPDX & Copyright Header Audit (Priority: P1)

As the project maintainer, I want an automated scanner to inspect all 600+ source files in `Sources/`, `Tests/`, and build scripts, asserting that 100% of proprietary files have valid `SPDX-License-Identifier: LicenseRef-TTZip-Source-Available-1.0` and copyright headers, so that no file is left ambiguous or orphaned.

**Why this priority**: Ambiguous or missing source headers create legal loopholes where third parties could claim code was released without restrictions.

**Independent Test**: Run `scripts/audit_licenses.py --check-headers`; assert 0 missing SPDX tags across all `.swift`, `.c`, `.h`, and `.m` files.

**Acceptance Scenarios**:
1. **Given** all source files in `Sources/`, **When** the license auditor runs, **Then** it validates that every single proprietary file begins with `// SPDX-License-Identifier: LicenseRef-TTZip-Source-Available-1.0`.
2. **Given** any new or unmodified file lacking SPDX tags, **When** audited, **Then** the scanner flags the exact line and file path.

---

### User Story 2 - Third-Party Open Source Attribution & Legal Notice Generation (Priority: P2)

As an App Store and open-source distributor, I want all third-party licenses (`MIT`, `BSD-2-Clause`, `zlib`, `Public Domain`) in `Vendor/` and `Sources/CTTZipBridge/fast-lzma2/` to be automatically harvested into a clean `Acknowledgements.plist` and `docs/THIRD_PARTY_LICENSES.md`, so that MIT/BSD notice obligations are 100% satisfied.

**Why this priority**: MIT and BSD licenses legally require preserving copyright notices in binary and source distributions. Failing to bundle notices violates the license terms.

**Independent Test**: Run `scripts/generate_acknowledgements.py`; verify that all 6 upstream engines (`libdeflate`, `zlib-ng`, `libarchive`, `zstd`, `lz4`, `fast-lzma2`) have complete copyright notices extracted into structured documentation.

**Acceptance Scenarios**:
1. **Given** all upstream dependencies in `Vendor/`, **When** the attribution generator runs, **Then** it produces `docs/THIRD_PARTY_LICENSES.md` with exact verbatim license texts.
2. **Given** the macOS App bundle, **When** opening About / Settings, **Then** it provides access to third-party open-source acknowledgements.

---

### User Story 3 - Copyleft & License Conflict Immunity Audit (Priority: P3)

As a commercial software author, I want to mathematically verify that zero viral copyleft licenses (`GPLv2`, `GPLv3`, `AGPL`) are statically linked into the TTZip binary distribution, and that tri-licensed components (such as `uchardet`) are strictly linked under non-viral permissive clauses (`MPL 1.1+` / `LGPL 2.1+` dynamic linking).

**Why this priority**: GPL viral licenses would legally force the entire TTZip proprietary application to be licensed under GPL, destroying the `TTZip-SAL-1.0` source-available model.

**Independent Test**: Run the dependency license classifier; assert that 100% of statically linked code is strictly Permissive (MIT, BSD, zlib, Apache 2.0, Public Domain).

**Acceptance Scenarios**:
1. **Given** the binary link map and `Package.swift`, **When** scanning linked libraries, **Then** it verifies that all static symbols come exclusively from permissive or public domain libraries.
2. **Given** `uchardet`, **When** audited, **Then** it confirms adherence to the MPL 1.1 / LGPL 2.1 component boundary.

---

### User Story 4 - Upstream Carve-Out & Patent Peace Verification (Priority: P4)

As an active contributor to open-source foundations (e.g. `zlib-ng`, `libarchive`), I want Section 1 (Upstream Carve-Out) and Section 5 (Patent Peace & Anti-Trolling) in root `LICENSE` to be verified for legal consistency, ensuring our upstream PRs remain valid permissive contributions while our proprietary app core remains completely immune to patent ambushes.

**Why this priority**: Preserves seamless upstream collaboration without compromising commercial IP protection.

**Independent Test**: Audit root `LICENSE` text against the SPDX specification and assert that Section 1.4 and Section 5.1/5.2 are legally cohesive.

**Acceptance Scenarios**:
1. **Given** the root `LICENSE` file, **When** analyzed, **Then** it validates proper SPDX expression naming (`LicenseRef-TTZip-Source-Available-1.0`) and cohesive patent peace clauses.

---

## 3. Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST provide an automated header audit scanner (`scripts/audit_licenses.py`) that validates 100% SPDX header coverage across all proprietary source code.
- **FR-002**: The system MUST generate a comprehensive, human-readable third-party attribution document (`docs/THIRD_PARTY_LICENSES.md`) covering all bundled upstream components.
- **FR-003**: The system MUST verify that 0 GPL/AGPL viral code is statically linked into `TTZipApp` or `ttzip-cli`.
- **FR-004**: The system MUST verify that all tri-licensed components (e.g. `uchardet`) comply with non-viral permissive boundaries.
- **FR-005**: The system MUST validate that root `LICENSE` contains all 5 core sections (Permitted Uses, Strict Prohibitions, Exclusive Distribution, Trademark, Patent Peace).

---

## 4. Success Criteria *(mandatory)*

1. **Header Coverage**: 100% of proprietary source files in `Sources/` must possess valid SPDX license headers.
2. **Attribution Completeness**: 100% of third-party upstream components in `Vendor/` must have verbatim licenses recorded in `docs/THIRD_PARTY_LICENSES.md`.
3. **Zero Copyleft Contamination**: 0 viral GPL/AGPL dependencies in static link paths.
4. **Automated CI Integration**: Audit script must execute cleanly with code 0 in pre-commit and CI verification workflows.
