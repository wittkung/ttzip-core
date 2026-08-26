# Research & Technical Decisions: 013 Rust & UniFFI Unified i18n Engine

- **Feature Directory**: `specs/013-rust-uniffi-unified-i18n-engine`
- **Created**: 2026-08-25
- **Author**: Antigravity AI & TTZip Architectural Governance Team

---

## 1. Rust Zero-Allocation String Lookup Pattern

### Decision
Use compile-time static string slice arrays sorted by key combined with `binary_search_by_key`, or `match` statements generated per catalog.

### Rationale
- `match key { "common.ok" => "...", ... }` compiles in Rust directly to jump tables or optimized string hashing without pulling in heavy build-time dependencies.
- A sorted `&[(&str, &str)]` with `binary_search_by_key` takes ~2KB of `.rodata` per language and executes in $\approx 8\text{ ns}$ (at most 9 string comparisons for 398 keys), fully within L1 data cache.
- Heap allocation: **0 bytes**.

### Alternatives Considered
- `phf` crate: Excellent, but introduces build-script complexity and additional proc-macro compilation overhead. Sorted static slice arrays achieve identical latency with zero external dependencies.
- Runtime `HashMap<&str, &str>`: Requires heap initialization on startup and introduces lock contention or synchronization overhead.

---

## 2. UniFFI Export Pattern for Global Localization

### Decision
Export `TTZipLocalizationEngine` as an `Arc`-managed object via `#[uniffi::export]` or `#[derive(uniffi::Object)]`, with static methods or singleton instantiation.

### Rationale
- Mozilla UniFFI natively manages `Arc<TTZipLocalizationEngine>` life cycles across FFI barriers.
- Swift 6 strict concurrency marks UniFFI objects conforming to `Sendable`.
- Allows future multi-instance custom dictionaries (e.g. plugin extensions) while providing a default global singleton `TTZipLocalizationEngine::default()`.

---

## 3. CLDR Formatter Standards

### Decimal and Grouping Separators
- **English (`en`)**: Decimal `.`, Grouping `,` (e.g. `1,250.8 MB/s`)
- **German (`de`)**: Decimal `,`, Grouping `.` (e.g. `1.250,8 MB/s`)
- **French (`fr`)**: Decimal `,`, Grouping `\u{202F}` (narrow non-breaking space) or space (e.g. `1 250,8 MB/s`)
- **Spanish (`es`)**: Decimal `,`, Grouping `.` (e.g. `1.250,8 MB/s`)
- **Japanese (`ja`) / Chinese (`zh`)**: Decimal `.`, Grouping `,` (e.g. `1,250.8 MB/s`)

### Byte Size Units
- **Metric (SI - 1000 base)**: `B`, `KB`, `MB`, `GB`, `TB`
- **Binary (IEC - 1024 base)**: `B`, `KiB`, `MiB`, `GiB`, `TiB`
