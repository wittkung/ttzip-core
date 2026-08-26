# Data Model: Codebase Quality Audit and Optimization

**Feature Branch**: `153-codebase-quality-audit-and-optimization` | **Date**: 2026-08-20 | **Spec**: [spec.md](./spec.md)

---

## 1. Domain Entities & Type Definitions

### 1.1 `LocaleCatalogSchema`
Represents the multi-language string dictionary schema mapping localized keys to translated string templates across all 7 supported language locales.

- **`language_code`** (`string`, required): ISO 639-1 language code with optional script variant.
  - Allowed values: `"en"`, `"zh-Hans"`, `"zh-Hant"`, `"ja"`, `"de"`, `"es"`, `"fr"`.
- **`catalog_version`** (`string`, required): Semantic version string representing catalog format version (e.g., `"1.0.0"`).
- **`entry_count`** (`integer`, minimum: 1, maximum: 1000, required): Total number of localized key-value pairs contained in the catalog.
- **`strings`** (`object`, required): Key-value dictionary mapping dot-separated keys to localized strings.
  - Property name pattern: `^[a-z]+(\\.[a-z0-9_]+)+$` (e.g. `"error.file_not_found"`, `"compress.title"`).
  - Property values: Non-empty `string`.

---

### 1.2 `ArchiveErrorPayload`
Represents an archive subsystem failure event, encapsulating error code, domain category, technical diagnostics, and localized human-readable messages.

- **`error_code`** (`integer`, minimum: 1000, maximum: 9999, required): Unique numerical error code identifying the specific archive failure scenario.
- **`domain`** (`string`, enum: `["io", "format", "crypto", "bounds", "operation"]`, required): High-level operational failure domain.
- **`error_name`** (`string`, enum: `["fileNotFound", "readFailed", "invalidFormat", "passwordRequired", "wrongPassword", "corruptedData", "cancelled", "invalidState"]`, required): Symbolic enum name corresponding to Swift `ArchiveError`.
- **`localized_key`** (`string`, pattern: `^error\\.[a-z0-9_]+$`, required): Localization key in `L10n.Errors` matching this error type.
- **`localized_message`** (`string`, minLength: 1, maxLength: 1024, required): Rendered human-readable message localized in the active system locale.
- **`context_details`** (`object`, required, additionalProperties: false):
  - **`entry_path`** (`string` or `null`): Archive entry path associated with the failure if applicable.
  - **`os_error_code`** (`integer` or `null`): Underlying POSIX / errno return code if an IO system call failed.
  - **`encryption_method`** (`string` or `null`): Unrecognized encryption method identifier if format error occurred.

---

### 1.3 `LoggingEventPayload`
Represents a structured telemetry and diagnostic log record emitted through `TTLogger`.

- **`timestamp_ms`** (`integer`, minimum: 0, required): Unix timestamp in milliseconds since epoch.
- **`level`** (`string`, enum: `["debug", "info", "warning", "error"]`, required): Severity level of the diagnostic log event.
- **`subsystem`** (`string`, minLength: 1, maxLength: 64, required): Subsystem identifier emitting the log (e.g., `"TTZipCore.Zip"`, `"CTTZipBridge.Crypto"`).
- **`message`** (`string`, minLength: 1, maxLength: 4096, required): Rendered text content of the log event.
- **`metadata`** (`object`, required, additionalProperties: false):
  - **`file`** (`string`, minLength: 1, required): Source file path where log was triggered.
  - **`line`** (`integer`, minimum: 1, required): Source file line number.
  - **`function`** (`string`, minLength: 1, required): Calling function name.
  - **`thread_id`** (`integer`, minimum: 0, required): Operating system thread identifier.

---

## 2. Invariants & Lifecycle State Transitions

### 2.1 C Bridge Struct Lifecycle
```
[Allocated: malloc()] 
         │
         ▼
[Initialized: magic = 0x545A4950 ('TZIP')]
         │
   (Active Execution)
         │
         ▼
[Teardown: ttzip_secure_zero() on sensitive buffers]
         │
         ▼
[Poisoned: magic = 0xDEADBEEF]
         │
         ▼
[Deallocated: free()]
```

### 2.2 Error Localization Flow
```
[Archive Operation Failure]
         │
         ▼
[Instantiate ArchiveError (Swift enum)]
         │
         ▼
[Resolve LocalizationKey via ArchiveError+L10n]
         │
         ▼
[Query TTZipLocalizationManager for Active Locale]
         │
   ┌─────┴────────────────────────┐
   │ (Key Present)                │ (Key Missing)
   ▼                              ▼
[Localized String]        [Fallback to English Catalog]
         │                              │
         └──────────────┬───────────────┘
                        ▼
           [Output to UI / Alert / CLI]
```
