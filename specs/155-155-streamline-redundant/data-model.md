# Data Model: Test Suite Inventory & Decoupling Mapping

**Feature**: `155-155-streamline-redundant`  
**Date**: 2026-08-20  
**Status**: Completed  

---

## 1. Entities & Data Structures

### 1.1 `TestSuiteAuditEntry`
Represents an individual test suite audit record and its classification.

| Field | Type | Required | Description | Constraints / Validation |
| :--- | :--- | :--- | :--- | :--- |
| `file_name` | `string` | Yes | Name of the Swift test file | Non-empty, ends with `.swift` |
| `layer` | `enum` | Yes | Target architectural layer | Enum: `["microkernel_c", "swift_bridge", "swift_architecture", "appkit_gui"]` |
| `action` | `enum` | Yes | Action taken | Enum: `["prune", "retain"]` |
| `c_equivalent_suite`| `string` | No | Corresponding C test file in `tests/c/` | Present if `action == "prune"` |
| `justification` | `string` | Yes | Rationale for pruning or retention | Non-empty string |

---

### 1.2 `DualEngineTestMatrix`
The complete inventory of active test suites across C and Swift layers.

| Field | Type | Required | Description | Constraints / Validation |
| :--- | :--- | :--- | :--- | :--- |
| `schema_version` | `string` | Yes | Version identifier | Constant `"1.0.0"` |
| `total_c_suites` | `integer` | Yes | Total C test suites registered in CTest | `>= 8` |
| `total_swift_suites`| `integer` | Yes | Total active Swift test suites | `>= 80` |
| `pruned_swift_suites`| `integer` | Yes | Count of pruned redundant test files | `>= 1` |
| `pruned_files` | `array<TestSuiteAuditEntry>`| Yes | List of pruned files with mappings | `length == pruned_swift_suites` |
