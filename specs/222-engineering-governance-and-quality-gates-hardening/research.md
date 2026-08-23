# Phase 0 Research: Engineering Governance and Quality Gates Hardening

## 1. C-ABI Symbol Parity Verification Methodology

### Problem
Static libraries built via `cargo` (`staticlib`) and `clang` can drop symbols if header declarations diverge from Rust `#[no_mangle] pub extern "C"` functions or if stripping flags (`strip -x` vs `strip -S`) remove exported external symbols.

### Solution
Use `clang -Xclang -ast-dump=json` or standard regex scanning on `Sources/CTTZipBridge/include/ttzip_rust_glue.h` to extract all function prototypes. Then invoke `nm -gU` on `Vendor/TTZipVendor.xcframework/macos-arm64/libTTZipVendor.a` and assert that every header function corresponds to a global defined text symbol (`T`) in the archive.

```bash
# Example extraction pipeline:
nm -gU Vendor/TTZipVendor.xcframework/macos-arm64/libTTZipVendor.a | awk '{print $3}' | sed 's/^_//' | sort -u
```

---

## 2. Multi-Volume Virtual Reader Stress Testing

### Problem
When `.001` archives span multiple parts, boundary rollovers (e.g. crossing part 1 into part 2 mid-entry) must be verified without staging on disk.

### Solution
Construct a synthetic 3-part split TAR/ZIP archive in memory, write the slice files to temporary locations, and use `ArchiveReader.listEntries` and `ArchiveSelectiveExtractor.extractSelected` while verifying disk writes via `getfsstat` / file modification timestamps to prove 0 bytes written to `/tmp`.

---

## 3. ASan / TSan Integration on Apple Silicon

### Problem
SwiftPM and Cargo have differing flags for AddressSanitizer and ThreadSanitizer.

### Solution
- Swift: `swift test --sanitize=address` and `swift test --sanitize=thread`
- Rust: `RUSTFLAGS="-Zsanitizer=address" cargo test` (requires nightly) or building native codecs with `-fsanitize=address`.
Provide `./scripts/run_sanitizers.sh` with automatic compiler detection.
