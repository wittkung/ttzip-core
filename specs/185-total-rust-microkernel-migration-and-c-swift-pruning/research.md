# Phase 0 Research: 185-total-rust-microkernel-migration-and-c-swift-pruning

## Research Item R001: C/C++ Tree Purging from Swift Package
- **Decision**: Remove `Sources/CTTZipBridge/zopfli/`, `fast-lzma2/`, `lzfse/`, `snappy/`, `CTTZipBridge.c`, `CTTZipBridge_Archive.c`.
- **Rationale**: 
  - All these codecs are already compiled into `Vendor/libTTZipVendor.a` via Cargo.
  - Having duplicate C sources in `Sources/CTTZipBridge` causes redundant compilation and creates dual-build friction on Linux/Windows.
- **Alternatives Considered**: 
  - *Keep C sources in SPM*: Prevents smooth Linux/Windows cross-compilation without Clang/CMake tooling.
- **Source**: 
  - `Sources/CTTZipBridge/`
  - `Vendor/libTTZipVendor.a`

---

## Research Item R002: Password Recovery Engine in Safe Rust
- **Decision**: Enhance `rust/ttzip-glue/src/crypto/password_recovery.rs` with Rayon chunked dictionary attacks, SIMD PMULL / CRC32 key testing, and zero-allocation key scheduling.
- **Rationale**: 
  - Yields $>500,000\text{ passwords/sec}$ attack speed while keeping memory overhead bounded.
- **Alternatives Considered**: 
  - *Swift GCD multi-threading*: High thread context switching overhead.
- **Source**: 
  - `Sources/TTZipCore/PasswordRecoveryEngine.swift`
  - `rust/ttzip-glue/src/crypto/password_recovery.rs`
