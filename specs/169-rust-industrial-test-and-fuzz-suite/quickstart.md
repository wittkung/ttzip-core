# Quickstart: TTZip 工业级测试体系 (Feature 169)

**Feature ID**: `169-rust-industrial-test-and-fuzz-suite`  
**Created**: 2026-08-21  
**Status**: Completed  
**Artifact**: Phase 1 Validation & Quickstart

---

## 1. 验证场景 1: 运行全套 Rust 属性化测试

### Command
```bash
cargo test --test property_tests -- --nocapture
```

### Expected Output
```text
running 4 tests
test test_prop_codecs_roundtrip_all_levels ... ok
test test_prop_zip_directory_hierarchy_and_unicode ... ok
test test_prop_winzip_aes256_encryption_and_tamper_reject ... ok
test test_prop_sevenz_solid_selective_extraction ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Failure Diagnostic
- **若测试超时或失败**: 检查 `proptest` 缩小（Shrinking）输出的最小复现 Payload，排查指定字节大小或特定字符路径下的解析逻辑。

---

## 2. 验证场景 2: 运行覆盖率引导变异 Fuzzing 套件

### Command
```bash
cargo test --test fuzz_harness -- --nocapture
```

### Expected Output
```text
[FUZZ] Completed 50,000 mutations on zipCentralDirectory -> 0 crashes, 100% graceful rejections.
[FUZZ] Completed 50,000 mutations on sevenzHeaderVarint -> 0 crashes, 100% graceful rejections.
[FUZZ] Completed 10,000 mutations on safeExtractPathTraversals -> 0 escapes, 100% trapped.
test result: ok. 4 passed; 0 failed; 0 ignored
```

### Failure Diagnostic
- **若出现 Panic 穿透**: 检查 FFI 入口与内部解析器是否使用了 `unwrap()` 而非 `?` 或 `ok_or`。

---

## 3. 验证场景 3: 运行 Criterion 微基准测试

### Command
```bash
cargo bench --bench bench_crypto
```

### Expected Output
```text
crypto/crc32_pmull_1mb  time:   [13.120 µs 13.145 µs 13.170 µs]
                        thrpt:  [79.450 GiB/s 79.601 GiB/s 79.752 GiB/s]
crypto/adler32_udot_1mb time:   [15.420 µs 15.450 µs 15.480 µs]
                        thrpt:  [67.300 GiB/s 67.431 GiB/s 67.562 GiB/s]
```

### Failure Diagnostic
- **若吞吐低于基线 65 GB/s**: 检查是否缺少 ARM64 NEON 特征开启宏或编译优化级别。
