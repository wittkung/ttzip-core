# Data Model: Internationalization & Copyright Manifest

## Entity: CodebaseTranslationManifest
Defines metadata and statistics for the codebase internationalization pass.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `total_files_processed` | Integer | Yes | Total files inspected |
| `headers_injected` | Integer | Yes | Total SPDX BSD-3-Clause headers added |
| `comments_translated` | Integer | Yes | Total lines of Chinese comments converted to English |
| `remaining_chinese_lines` | Integer | Yes | Should be 0 after completion (excluding whitelisted fixtures) |
| `license_identifier` | String | Yes | `BSD-3-Clause` |
| `copyright_holder` | String | Yes | `Weitao Kung (Witt Kung)` |
