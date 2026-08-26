# Contract Specification: UniFFI Localization Engine Interface

- **Target System**: `core/rust/ttzip-engine/src/uniffi_api/mod.rs` & Swift UniFFI Bindings
- **Version**: 1.0.0

---

## 1. Rust Export Signatures

```rust
pub fn ttzip_i18n_get_string(key: &str, lang: AppLanguage) -> String;
pub fn ttzip_i18n_format_bytes(bytes: i64, standard: ByteSizeStandard, lang: AppLanguage) -> String;
pub fn ttzip_i18n_format_throughput(mb_per_sec: f64, lang: AppLanguage) -> String;
pub fn ttzip_i18n_localize_error(error_code: i32, param1: Option<String>, param2: Option<String>, lang: AppLanguage) -> String;
```

## 2. Swift Foreign Interface

```swift
public protocol TTZipLocalizationEngineProtocol: Sendable {
    func getString(key: String, lang: AppLanguage) -> String
    func formatBytes(bytes: Int64, standard: ByteSizeStandard, lang: AppLanguage) -> String
    func formatThroughput(mbPerSec: Double, lang: AppLanguage) -> String
    func localizeError(errorCode: Int32, param1: String?, param2: String?, lang: AppLanguage) -> String
}
```
