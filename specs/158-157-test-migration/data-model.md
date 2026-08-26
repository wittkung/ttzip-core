# Data Model: Full 22-File Swift Test Migration & Suite Inventory

**Feature**: `158-157-test-migration`  
**Date**: 2026-08-20  
**Status**: Completed  

---

## 1. Entities & Data Structures

### 1.1 `MigrationBatchEntry`
Represents an individual batch of migrated Swift tests.

| Field | Type | Required | Description | Constraints / Validation |
| :--- | :--- | :--- | :--- | :--- |
| `cluster_id` | `string` | Yes | Identifier of the cluster | Enum: `["adler_crc64", "entropy_evaluator", "matchfinder_advanced", "blosc_slicing", "crypto_lz4_snappy"]` |
| `c_test_suite_path` | `string` | Yes | Relative path to C test file | Must start with `tests/c/test_` and end with `.c` |
| `swift_files_pruned` | `array<string>` | Yes | Array of pruned Swift test file names | `length >= 2` |
| `total_assertions` | `integer` | Yes | Total assertions in the C test suite | `>= 10` |

---

### 1.2 `FullMigrationReport`
Telemetry report capturing the complete 22-file migration.

| Field | Type | Required | Description | Constraints / Validation |
| :--- | :--- | :--- | :--- | :--- |
| `schema_version` | `string` | Yes | Version identifier | Constant `"1.0.0"` |
| `total_pruned_files`| `integer` | Yes | Count of pruned Swift test files | Exactly `22` |
| `total_c_suites` | `integer` | Yes | Total active CTest test suites | `>= 19` |
| `total_c_duration_ms`| `number`| Yes | Total execution duration of all C suites | `<= 15.0` |
| `migration_clusters` | `array<MigrationBatchEntry>` | Yes | Inventory of clusters | `length == 5` |
