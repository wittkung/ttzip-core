# Data Model: Feature 132 - Full Codebase License & IP Compliance Audit

## 1. Core Entities

### 1.1 SourceFileHeaderAudit
Represents the license header audit state of an individual source file in the repository.

| Field Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `file_path` | `string` | Relative file path from repository root | Yes |
| `file_type` | `enum("swift", "c", "header", "objc", "script")` | File language category | Yes |
| `has_spdx_tag` | `boolean` | Whether file contains valid SPDX-License-Identifier | Yes |
| `spdx_identifier` | `string` | Detected SPDX expression | Yes |
| `has_copyright` | `boolean` | Whether file contains valid Copyright declaration | Yes |
| `compliance_status` | `enum("compliant", "missing_spdx", "missing_copyright", "foreign_license")` | Audit compliance result | Yes |

### 1.2 ThirdPartyDependencyLicense
Represents the legal attribution record for an external upstream library.

| Field Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `component_name` | `string` | Library name (e.g. `libdeflate`, `zlib-ng`) | Yes |
| `component_path` | `string` | Directory path in `Vendor/` | Yes |
| `license_type` | `string` (SPDX ID) | Detected license (e.g. `MIT`, `BSD-2-Clause`, `zlib`) | Yes |
| `copyright_holder` | `string` | Author / Copyright owner string | Yes |
| `license_text_path` | `string` | Path to raw LICENSE file | Yes |
| `copyleft_risk` | `enum("permissive_safe", "weak_copyleft_compliant", "viral_prohibited")` | Copyleft assessment | Yes |

### 1.3 FullRepositoryLicenseAuditSummary
Represents the aggregate result of the full-codebase compliance scan.

| Field Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `total_proprietary_files` | `integer` | Total proprietary source files scanned | Yes |
| `compliant_files_count` | `integer` | Files with 100% compliant headers | Yes |
| `third_party_components_count` | `integer` | Total upstream dependencies audited | Yes |
| `viral_copyleft_detected` | `boolean` | Flag if any viral GPL/AGPL is statically linked | Yes |
| `audit_passed` | `boolean` | Overall pass/fail status (must be true) | Yes |
