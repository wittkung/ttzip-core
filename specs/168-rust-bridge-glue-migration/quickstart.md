# Quickstart & Verification Guide: TTZip 核心胶水层 Rust 迁移 (Feature 168)

**Feature ID**: `168-rust-bridge-glue-migration`  
**Created**: 2026-08-21  
**Status**: Completed  
**Artifact**: Phase 1 Validation & Quickstart Guide

---

## 1. 验证场景 1: Rust 胶水层多架构构建与 Universal 静态库打包

### Command
```bash
./scripts/build_rust.sh --release
```

### Expected Output
```text
[INFO] Building ttzip-glue for aarch64-apple-darwin (release)...
[INFO] Building ttzip-glue for x86_64-apple-darwin (release)...
[INFO] Generating C headers via cbindgen: Sources/CTTZipBridge/include/ttzip_rust_glue.h
[INFO] Creating Universal static library via libtool / lipo: Vendor/libTTZipVendor.a
[SUCCESS] Rust glue universal library generated successfully.
```

### Failure Diagnostic
- **若 `cargo: command not found`**: 确保已安装 Rust 1.80+ (`rustup toolchain install stable`)，并在 PATH 中包含 `~/.cargo/bin`。
- **若 `cbindgen: command not found`**: 执行 `cargo install --locked cbindgen`。
- **若 `libtool` 合并报错缺少架构切片**: 检查 `rustup target list --installed` 是否包含 `aarch64-apple-darwin` 与 `x86_64-apple-darwin`。执行 `rustup target add aarch64-apple-darwin x86_64-apple-darwin`。

---

## 2. 验证场景 2: Rust 单元测试与 ASan / Miri 内存安全验证

### Command
```bash
cd rust/ttzip-glue && cargo test --release -- --nocapture
```

### Expected Output
```text
running 32 tests
test archive::tests::test_archive_reader_drop_safety ... ok
test crypto::tests::test_crc32_pmull_fold12_matches_oracle ... ok
test crypto::tests::test_adler32_udot_matches_zlib ... ok
test crypto::tests::test_aes256_cbc_8way_neon ... ok
test crypto::tests::test_sha256_kdf_524k_rounds ... ok
test zip::tests::test_zip_parallel_extract_corrupt_stream_resilient ... ok
test fs::tests::test_two_pass_deferred_permission_fixup ... ok

test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Failure Diagnostic
- **若 CRC32 / Adler32 测试失败**: 检查 CPU 向量指令特征探测宏 `#[cfg(target_arch = "aarch64")]` 是否正确，排查未对齐指针加载。
- **若出现内存泄漏或断言失败**: 使用 `RUSTFLAGS="-Zsanitizer=address" cargo +nightly test` 定位底层句柄析构缺失。

---

## 3. 验证场景 3: Swift 单元测试与端到端回归套件

### Command
```bash
swift test --filter TTZipTests
```

### Expected Output
```text
Test Suite 'All tests' passed at 2026-08-21 07:31:00.000.
	 Executed 525 tests, with 0 failures (0 unexpected) in 4.128 (4.135) seconds
```

### Failure Diagnostic
- **若 `import CTTZipBridge` 找不到符号 `ttzip_rust_*`**: 检查 `Sources/CTTZipBridge/include/module.modulemap` 是否已声明 `header "ttzip_rust_glue.h"`，并验证 `Package.swift` 中 `TTZipVendor.xcframework` 链接路径。
- **若发生 `SIGSEGV` 或 `Fatal Error`**: 检查 FFI 调用处指针传递是否满足内存对齐，确保 Rust 侧所有导出函数均包裹了 `std::panic::catch_unwind`。

---

## 4. 验证场景 4: A/B 吞吐基准测试与零性能倒退门禁

### Command
```bash
./scripts/benchmark_ab.sh HEAD~1 HEAD --runs 5
```

### Expected Output
```text
[BENCH-AB] Scenario 1: ZIP Level 1 Compression (1GB Corpus)
  Baseline: 1542.8 MB/s | Candidate: 1568.4 MB/s (Delta: +1.66%) -> PASSED
[BENCH-AB] Scenario 2: ZIP Multi-Core Decompression (1GB Corpus)
  Baseline: 4620.1 MB/s | Candidate: 4685.3 MB/s (Delta: +1.41%) -> PASSED
[BENCH-AB] Scenario 3: WinZip AES-256 Decompression (500MB Corpus)
  Baseline: 1845.2 MB/s | Candidate: 1890.6 MB/s (Delta: +2.46%) -> PASSED
[BENCH-AB] Evaluation Result: PASSED_NO_REGRESSION
```

### Failure Diagnostic
- **若吞吐倒退超过 2.0%**: 检查编译模式是否开启 `--release` (`opt-level = 3`, `lto = "thin"`)，确认 CRC32/Adler32 是否命中了 PMULL 12 路与 UDOT 硬件向量指令，避免退化为标量实现。
