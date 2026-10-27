# Tasks: TTZip 工业级 Rust 属性测试、模糊测试与高精基准测试体系 (Feature 169)

**Feature ID**: `169-rust-industrial-test-and-fuzz-suite`  
**Created**: 2026-08-21  
**Status**: Ready for Implementation  
**Specification**: [`specs/169-rust-industrial-test-and-fuzz-suite/spec.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/169-rust-industrial-test-and-fuzz-suite/spec.md)  
**Plan**: [`specs/169-rust-industrial-test-and-fuzz-suite/plan.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/169-rust-industrial-test-and-fuzz-suite/plan.md)

---

## Phase 1: 依赖与基准环境配置 (Dev Dependencies & Setup)

- [x] T001 [P] 在 `rust/ttzip-glue/Cargo.toml` 引入 `proptest`, `criterion`, `tempfile`, `flate2` 并配置 `[[bench]]` in `rust/ttzip-glue/Cargo.toml`
- [x] T002 [P] 编写 `scripts/run_rust_tests.sh` 自动化全域测试套件执行脚本 in `scripts/run_rust_tests.sh`

---

## Phase 2: 声明式属性化测试集 (US1 - Property-Based Testing)

- [x] T003 [P] [US1] 编写编解码器随机数据流往返属性测试 in `rust/ttzip-glue/tests/property_tests.rs`
- [x] T004 [P] [US1] 编写 ZIP / 7z 多级目录树、Unicode 与空文件属性测试 in `rust/ttzip-glue/tests/property_tests.rs`
- [x] T005 [P] [US1] 编写 WinZip AES 与 7z AES 随机密码加密/解密属性测试 in `rust/ttzip-glue/tests/property_tests.rs`

---

## Phase 3: 覆盖率引导模糊测试 Harness (US2 - Fuzzing Harness)

- [x] T006 [P] [US2] 实现 ZIP Central Directory 与 Extra Field 变异 Fuzz Target in `rust/ttzip-glue/tests/fuzz_harness.rs`
- [x] T007 [P] [US2] 实现 7z Header / Varint 变异 Fuzz Target in `rust/ttzip-glue/tests/fuzz_harness.rs`
- [x] T008 [P] [US2] 实现 ZipSlip 与恶意路径穿越注入 Fuzz Target in `rust/ttzip-glue/tests/fuzz_harness.rs`
- [x] T009 [P] [US2] 实现流式微缓冲故障注入 Fuzz Target in `rust/ttzip-glue/tests/fuzz_harness.rs`

---

## Phase 4: 跨生态双向差分与 CVE 黄金语料库 (US4 - Differential Oracle)

- [x] T010 [P] [US4] 实现与系统 `/usr/bin/unzip` 及 `/usr/bin/tar` 的双向进程差分测试 in `rust/ttzip-glue/tests/differential_oracle.rs`
- [x] T011 [P] [US4] 嵌入 ASCII 历史 CVE 畸变语料库并在内存中秒级测试安全拒绝 in `rust/ttzip-glue/tests/differential_oracle.rs`

---

## Phase 5: 高精微基准套件 (US3 - Criterion Micro-benchmarks)

- [x] T012 [P] [US3] 实现 ARM64 NEON PMULL CRC32, UDOT Adler32 与 AES-256 / SHA-256 Criterion 微基准 in `rust/ttzip-glue/benches/bench_crypto.rs`
- [x] T013 [P] [US3] 实现单核编解码器（libdeflate, zstd, fl2, snappy, lz4, lzfse）Criterion 微基准 in `rust/ttzip-glue/benches/bench_codecs.rs`
- [x] T014 [P] [US3] 实现 ZIP 多核并行压缩解压 Criterion 微基准 in `rust/ttzip-glue/benches/bench_zip_parallel.rs`
- [x] T015 [P] [US3] 实现 7z Solid 固实按需切片提取延迟 Criterion 微基准 in `rust/ttzip-glue/benches/bench_sevenz_solid.rs`

---

## Phase 6: 全量收敛与本地 CI 门禁集成 (Converge & CI Integration)

- [x] T016 集成 Rust 全域测试至 `scripts/ci_gate.sh` 并执行完整验证
- [x] T017 运行全量 `cargo test`、`cargo bench`、`swift test`，确认 100% 绿色通过
