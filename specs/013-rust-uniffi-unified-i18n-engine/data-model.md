# Data Model & Type Definitions: 013 Rust & UniFFI Unified i18n Engine

- **Feature Directory**: `specs/013-rust-uniffi-unified-i18n-engine`
- **Created**: 2026-08-25
- **Author**: Antigravity AI & TTZip Architectural Governance Team

---

## 1. Rust Domain Enums & Structures

### `AppLanguage` (UniFFI Enum)
```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum AppLanguage {
    En,
    ZhHans,
    ZhHant,
    Ja,
    De,
    Fr,
    Es,
}
```

### `ByteSizeStandard` (UniFFI Enum)
```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ByteSizeStandard {
    MetricSI,
    BinaryIEC,
}
```

### `TTZipLocalizationEngine` (UniFFI Object)
```rust
#[derive(Default, uniffi::Object)]
pub struct TTZipLocalizationEngine;

#[uniffi::export]
impl TTZipLocalizationEngine {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }

    /// Retrieve localized string by key for given language, with fallback to En.
    pub fn get_string(&self, key: &str, lang: AppLanguage) -> String;

    /// Check if a key exists in dictionary.
    pub fn has_key(&self, key: &str) -> bool;

    /// Format byte sizes according to SI/IEC standards and language conventions.
    pub fn format_bytes(&self, bytes: i64, standard: ByteSizeStandard, lang: AppLanguage) -> String;

    /// Format throughput in MB/s according to language conventions.
    pub fn format_throughput(&self, mb_per_sec: f64, lang: AppLanguage) -> String;

    /// Translate standard archive error code into localized string.
    pub fn localize_error(&self, error_code: i32, param1: Option<String>, param2: Option<String>, lang: AppLanguage) -> String;
}
```
