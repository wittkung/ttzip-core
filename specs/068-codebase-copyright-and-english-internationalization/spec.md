# Feature Specification: 068-codebase-copyright-and-english-internationalization

**Feature Title**: Comprehensive Codebase Copyright Header Standardization and English Internationalization  
**Status**: Draft  
**Target Milestone**: TTZip v1.0.0 Open-Source International Compliance  

---

## 1. User Scenarios & Problem Statement

### 1.1 User Scenarios
- **Scenario A (International Contributor & Code Audit)**: An international systems engineer, security auditor, or open-source maintainer clones the TTZip repository. Every file they open contains a clean SPDX BSD-3-Clause copyright header and 100% idiomatic, professional English comments, docstrings, and MARK annotations.
- **Scenario B (Legal & IP Clarity)**: An enterprise or corporate developer inspecting the codebase finds consistent, unambiguous BSD 3-Clause legal headers on all source files (`.swift`, `.c`, `.h`, `.sh`), eliminating licensing ambiguity.
- **Scenario C (CLI & Logging Internationalization)**: A developer running `ttzip-cli` on macOS in any locale sees clean English output, standardized progress indicators, and informative English error messages.

---

## 2. Functional Requirements & Scope

### 2.1 File Copyright Header Standardization (P1)
- **FR-001 [SPDX License & Copyright Header]**: Prepend a standardized BSD 3-Clause header to every source file in `Sources/` and `Tests/`:
  ```swift
  // SPDX-License-Identifier: BSD-3-Clause
  //
  // Copyright (c) 2026, Weitao Kung (Witt Kung) <kevintungs@163.com>
  // All rights reserved.
  //
  // TTZip: High-performance native archiving and compression engine for macOS.
  ```
  And for C/C++ files:
  ```c
  // SPDX-License-Identifier: BSD-3-Clause
  //
  // Copyright (c) 2026, Weitao Kung (Witt Kung) <kevintungs@163.com>
  // All rights reserved.
  //
  // TTZip: High-performance native archiving and compression engine for macOS.
  ```

### 2.2 Complete Codebase English Internationalization (P1)
- **FR-002 [Comment & Docstring Translation]**: Translate all ~5,980 lines of Chinese comments, docstrings, and inline remarks across all ~474 Swift and C source files into idiomatic, concise, and technically accurate English.
- **FR-003 [MARK Annotation Normalization]**: Clean up and normalize all `// MARK: - 【X.X 模式名称】` annotations into standard English: `// MARK: - <Pattern Name> (<Context>)`.
- **FR-004 [CLI Console & Log English Localization]**: Standardize all CLI console outputs, progress messages, error descriptions, and help strings in `Sources/TTZipCLI/` into clean, professional English.

### 2.3 Verification & Quality Gate (P1)
- **FR-005 [Zero-Chinese Static Assertion]**: Implement a verification script (`scripts/assert_zero_chinese.py`) ensuring 0 non-ASCII Chinese characters remain in comments and logs (excluding intentional localized test fixtures/sample archives).
- **FR-006 [Full Test & Build Regression Safety]**: Ensure `swift build` and `swift test` continue to pass 100% with zero regressions on performance gates and functionality.

---

## 3. Success Criteria & Quality Metrics

1. **Zero Chinese in Codebase**: 0 Chinese characters detected in `Sources/` and `Tests/` (outside of intentional UTF-8 / GBK encoding test fixtures).
2. **100% Header Coverage**: 100% of `.swift`, `.c`, `.h`, `.sh` files contain the standard SPDX BSD-3-Clause copyright header.
3. **Zero Test Failure**: All 520+ unit and performance tests pass with 0 errors.

---

## 4. Assumptions & Boundaries

- Target source tree: `Sources/` and `Tests/`.
- Preserved: Legacy test fixtures specifically testing Chinese filename handling (e.g. GBK encoding fixtures).
