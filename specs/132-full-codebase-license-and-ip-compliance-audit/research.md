# Technical Research & Architecture Decisions: Feature 132

## R001: SPDX-License-Identifier Custom LicenseRef Specification & Scope
- **Decision**: Adopt `SPDX-License-Identifier: LicenseRef-TTZip-Source-Available-1.0` uniformly across all proprietary Swift and C/H source files in `Sources/`, using the standardized SPDX custom license prefix (`LicenseRef-`).
- **Rationale**: SPDX (Software Package Data Exchange - ISO/IEC 5962:2021) requires custom/proprietary licenses not in the official OSI list to use the `LicenseRef-` namespace. This ensures automated compliance scanners (FOSSA, Snyk, GitHub License Scanner) recognize the identifier without syntax errors.
- **Alternatives Considered**: Using `SPDX-License-Identifier: TTZip-1.0` (rejected: non-standard and rejected by standard SPDX parsers).
- **Source**: SPDX 2.3 / 3.0 Specification, ISO/IEC 5962:2021.

---

## R002: Third-Party FOSS Attribution & Legal Notice Harvesting Architecture
- **Decision**: Implement an automated harvester script (`scripts/generate_acknowledgements.py`) that recursively scans `Vendor/` and `Sources/CTTZipBridge/fast-lzma2/` to extract verbatim LICENSE texts into `docs/THIRD_PARTY_LICENSES.md` and generate an `Acknowledgements.plist` resource for the macOS GUI About view.
- **Rationale**: Permissive licenses (MIT, BSD-2-Clause, zlib, Apache 2.0) have a single mandatory condition: preserving the copyright notice and license text in binary distributions. Automating this generation ensures zero compliance lapses when dependencies are updated.
- **Alternatives Considered**: Manual copy-pasting of license texts (rejected: brittle and easily desynchronized during upstream vendor updates).
- **Source**: MIT License Section 2, BSD 2-Clause Clause 1/2, Apple App Store Legal Review Guidelines.

---

## R003: Tri-Licensed Dependency (`uchardet`) Boundary & Copyleft Immunity
- **Decision**: Explicitly declare and document that TTZip links against `uchardet` under its **Mozilla Public License Version 1.1 (MPL 1.1)** or **LGPL 2.1** option, operating strictly as an unmodified component interface with zero copyleft contamination to the outer TTZip application shell.
- **Rationale**: `uchardet` is tri-licensed (GPL 2.0+ / LGPL 2.1+ / MPL 1.1+). Under MPL 1.1+ (Section 3.7 - Larger Works) and LGPL 2.1, proprietary applications can link with the library without making the larger work open-source, provided the library itself is unmodified and attribution is provided.
- **Alternatives Considered**: Replacing uchardet with an in-house charset detector (rejected: unnecessary engineering overhead given MPL 1.1 permits compliant proprietary integration).
- **Source**: Mozilla Public License 1.1 Section 3.7, uchardet LICENSE / Headers.

---

## R004: Root LICENSE Multi-Layered Protection (Source-Available + Upstream Carve-Out + Patent Peace)
- **Decision**: Maintain the 5-section structure in `LICENSE` (`TTZip-SAL-1.0`):
  1. Permitted Uses (Personal, Research, Upstream Carve-Out)
  2. Strict Redistribution & Anti-Copycat Prohibitions (No App Store / White-labeling / SaaS)
  3. Official Exclusive Distribution & Enterprise Licensing
  4. Trademark & Trade Dress Protection
  5. Patent Peace & Defensive Anti-Trolling Clause (Prohibition of reverse patenting & defensive termination).
- **Rationale**: This combination gives global developers freedom to study and inspect the code, while completely blocking commercial copycats, app store parasitism, and patent ambushes.
- **Alternatives Considered**: Switching to AGPLv3 (rejected: AGPL limits commercial flexibility and would prevent enterprise proprietary licensing) or standard MIT (rejected: leaves TTZip vulnerable to app store copycats).
- **Source**: Business Source License (BSL 1.1), Redis Source Available License (RSALv2), Apache 2.0 Patent Clause.
