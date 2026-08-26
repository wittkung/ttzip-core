# Quickstart Guide: Testing & Verifying the Rust UniFFI i18n Engine

## 1. Rust Native Tests
Run unit tests verifying that all 7 language catalogs compile and pass key parity tests:
```bash
cargo test -p ttzip-engine --lib i18n
```

## 2. Generate UniFFI Bindings
Regenerate Swift bindings using UniFFI:
```bash
cd core/rust
cargo build -p ttzip-engine --release
cargo run -p ttzip-engine --bin uniffi-bindgen generate \
    --library target/release/libttzip_engine.dylib \
    --language swift \
    --out-dir ../Sources/CTTZipBridge/
```

## 3. Swift Verification
Run Swift test suites to verify zero regression in GUI and Core layers:
```bash
cd core && swift test --filter TTZipLocalizationSecurityTests
cd apple && swift test
```
