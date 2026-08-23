# Data Model: 工业级极端边界、安全漏洞与元数据测试体系 (Feature 163)

## 1. CVE Defense Test Case Record (`CVETestRecord`)

Represents a single deterministic evaluation of a known malformed or CVE-triggering bitstream.

| Field Name | Type | Nullable | Constraints / Description |
| :--- | :--- | :---: | :--- |
| `cve_identifier` | `string` | No | CVE or bug ID (e.g. `CVE-2002-0059`, `CVE-2005-1849`, `CVE-2018-25032`, `GH-382`, `GH-1600`, `CVE-2022-37434`) |
| `attack_vector` | `string` | No | Type of corruption (e.g. `huffman_tree_overflow`, `negative_match_distance`, `unbounded_extra_field`, `window_overflow`) |
| `payload_size_bytes` | `integer` | No | Malformed payload size in bytes |
| `expected_error_code` | `string` | No | Expected non-zero error identifier (e.g. `BAD_DATA`, `CORRUPT_STREAM`) |
| `execution_verdict` | `string` | No | `INTERCEPTED_SAFE`, `UNCAUGHT_ERROR`, `SEGFAULT` |
| `elapsed_microseconds` | `number` | No | Time taken to reject in microseconds |
| `sanitizer_clean` | `boolean` | No | True if ASan/UBSan report 0 warnings/errors |

---

## 2. Backward Compatibility Fixture Record (`CompatFixtureRecord`)

Represents the extraction verification of a historical or non-standard container archive.

| Field Name | Type | Nullable | Constraints / Description |
| :--- | :--- | :---: | :--- |
| `fixture_name` | `string` | No | Fixture identifier (e.g. `compat_zip_split_junk`, `compat_zip_data_descriptor`, `compat_zip_sfx`, `compat_gtar_longlink`) |
| `source_generator` | `string` | No | Tool that created it (e.g. `PKZIP_2.04g`, `PowerShell_CompressArchive`, `GNU_Tar_1.13`, `7-Zip_4.20`) |
| `container_format` | `string` | No | Format: `zip`, `tar`, `7z` |
| `entry_count` | `integer` | No | Expected number of extracted files |
| `uncompressed_bytes` | `integer` | No | Total uncompressed payload in bytes |
| `roundtrip_exact` | `boolean` | No | True if all extracted files match expected SHA256 checksums |
| `status` | `string` | No | `PASS` or `FAIL` |

---

## 3. Metadata & Sparse Fidelity Record (`MetadataFidelityRecord`)

Records the verification of macOS extended attributes and sparse file allocation.

| Field Name | Type | Nullable | Constraints / Description |
| :--- | :--- | :---: | :--- |
| `test_case` | `string` | No | `quarantine_xattr`, `custom_security_tag`, `apfs_1gb_sparse_hole`, `deep_symlink_chain` |
| `source_stat_size` | `integer` | No | Logical file size in bytes (e.g. 1073741824 for 1GB sparse) |
| `physical_blocks_used` | `integer` | No | Physical 512-byte blocks allocated (must be < 2048 for 1GB sparse) |
| `xattr_count` | `integer` | No | Number of verified extended attributes |
| `fidelity_passed` | `boolean` | No | True if extracted metadata and sparse blocks match source |
