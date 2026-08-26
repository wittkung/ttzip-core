# Quickstart: TTZip Engine Core & FFI Hardening Validation

This quickstart guide provides step-by-step instructions to validate and verify all 17 audited fixes and architectural enhancements across the Rust engine, C-ABI bridge, and Swift framework.

---

## 1. Prerequisites & Environment Setup

Ensure the following tools are available:
- **Rust Toolchain**: `rustc 1.80+` with `aarch64-apple-darwin` and `x86_64-apple-darwin` targets.
- **Xcode / Swift Toolchain**: `Swift 6.0+` on macOS 14/15.
- **Tools**: `cargo`, `lipo`, `otool`, `swift test`.

```bash
# Verify Rust targets
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

---

## 2. Automated Contract Verification

Run deterministic validation on all C-ABI and FFI contracts:

```bash
bash .specify/scripts/bash/lint-contracts.sh specs/001-engine-core-and-ffi-hardening/contracts
```
**Expected Outcome**: `All 4 contract schemas verified successfully.`

---

## 3. Universal Binary Build & Toolchain Validation

Build the Universal static library and verify that both `arm64` and `x86_64` architecture slices are preserved:

```bash
# Execute build script
bash apple/.build/checkouts/ttzip-core/scripts/build_rust.sh --release

# Inspect slice architectures
lipo -info core/Vendor/TTZipVendor.xcframework/macos-arm64_x86_64/libTTZipVendor.a
```
**Expected Outcome**: Output contains `Architectures in the fat file: ... are: arm64 x86_64`.

---

## 4. Rust Microkernel Automated Test Suite

Execute comprehensive unit, integration, and security regression tests in the Rust engine:

```bash
cd core/rust
cargo test --package ttzip-engine -- --nocapture
```

### Key Test Scenarios:
1. **7z Streaming Decompression**: `test_7z_streaming_decompression_bounded_memory` (extracts 1GB payload with $<10\text{MB}$ peak memory).
2. **ZIP Bounded Channel Compression**: `test_zip_bounded_channel_backpressure` (verifies memory stays bounded under slow disk writes).
3. **VFS Arena Slot Reuse**: `test_vfs_cache_slot_recycling_100k` (performs 100,000 insertions/evictions and asserts `shard.nodes.len() <= max_active_nodes`).
4. **WinZip Multi-Strength & ZipCrypto**: `test_winzip_aes128_192_256_and_zipcrypto` (validates correct decryption across all 4 cryptographic suites).
5. **Constant-Time Crypto**: `test_constant_time_ghash_and_mac` (proves constant time branch-free evaluation).

---

## 5. Swift Framework Integration & Concurrency Tests

Execute Swift package tests verifying strict concurrency, cancellation, error envelope propagation, and zero UI blocking:

```bash
cd core
swift test
```

### Key Test Scenarios:
1. **True Async Non-Blocking**: `testAsyncExtractDoesNotBlockMainActor` (proves `@MainActor` RunLoop executes smoothly during 10GB extraction).
2. **Cancellation Responsiveness**: `testTaskCancellationAbortsRustLoopInUnder10ms` (cancels an in-flight operation and asserts immediate termination).
3. **Structured Error Propagation**: `testRustErrorEnvelopeCapturesDiagnosticAcrossThreadHop` (triggers a corrupt header error and verifies Swift receives exact status, message, and offset).
4. **Exact Buffer Allocation**: `testQuickLookSingleEntryExtractsExactSize` (verifies small files allocate $<4\text{KB}$ and $>32\text{MB}$ files succeed without truncation).
