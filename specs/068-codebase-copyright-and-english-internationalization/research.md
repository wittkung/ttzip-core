# Phase 0 Research: Codebase Copyright Standardization & English Internationalization

## R001: SPDX License Identifier & Copyright Header Standard
- **Decision**: Standardize all source file headers across Swift, C, and Shell scripts to:
  ```swift
  // SPDX-License-Identifier: BSD-3-Clause
  //
  // Copyright (c) 2026, Weitao Kung (Witt Kung) <kevintungs@163.com>
  // All rights reserved.
  //
  // TTZip: High-performance native archiving and compression engine for macOS.
  ```
- **Rationale**: Complies with Linux Foundation SPDX 3.0 specification for machine-readable open-source licensing and provides clear copyright attribution.
- **Alternatives Considered**: Ad-hoc unstructured comments or omitting headers. (Rejected because professional open-source projects require unambiguous per-file licensing).
- **Source**: [SPDX Standard Specification](https://spdx.dev/ids/)

---

## R002: High-Fidelity Comment & String English Translation Pipeline
- **Decision**: Implement a dedicated AST-aware multi-pass translation engine in Python to process all 474+ files, converting:
  1. Header blocks and file descriptions;
  2. `// MARK: - 【X.X 模式名称】` to `// MARK: - <Pattern Name>`;
  3. Inline comments and docstrings (`///`, `//`, `/* */`);
  4. Console logging and user-facing CLI strings in `TTZipCLI`;
  while strictly preserving variable names, logic syntax, and test fixture binary data.
- **Rationale**: Enables rapid, uniform, and bit-exact translation across 5,980+ lines of comments without introducing syntactic regressions.
- **Alternatives Considered**: Manual line-by-line editing across 474 files. (Rejected due to high probability of human error and fatigue).
- **Source**: Industry standard linter/transformer methodologies.

---

## R003: Zero-Chinese Static Assertion & Gate Verification
- **Decision**: Implement `scripts/assert_zero_chinese.py` to scan `Sources/` and `Tests/` using Unicode range `[\u4e00-\u9fff]`, whitelisting only designated legacy encoding test files (such as `GBKEncodingTests.swift` testing charset recovery).
- **Rationale**: Prevents accidental re-introduction of non-English comments in future PRs and commits.
- **Alternatives Considered**: Manual code review only. (Rejected because automated CI assertion is deterministic).
- **Source**: Unicode 15.0 Character Database.
