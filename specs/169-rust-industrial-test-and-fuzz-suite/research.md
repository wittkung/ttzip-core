# Phase 0 Research: TTZip 工业级 Rust 属性测试、模糊测试与高精基准测试体系 (Feature 169)

**Feature ID**: `169-rust-industrial-test-and-fuzz-suite`  
**Created**: 2026-08-21  
**Status**: Completed  
**Artifact**: Phase 0 Technical Research & Architecture Invariants

---

## 1. 调研项与决策矩阵

### R001: 基于 `proptest` 的声明式属性化生成策略 (Property-Based Testing Strategies)

- **Decision (选定方案)**:
  在 `rust/ttzip-glue/Cargo.toml` 开发依赖中引入 `proptest = "1.5"`，构建以下三层生成策略（Generators）：
  1. `arb_byte_payload`: 生成 $0 \sim 1\text{MB}$ 的任意字节切片，涵盖全 0、全 0xFF、低熵重复文本、随机高熵数据；
  2. `arb_archive_tree`: 生成 $1 \sim 50$ 个条目的虚拟目录树（含空文件、单字节文件、深层嵌套目录、Unicode 路径名）；
  3. `arb_compression_params`: 生成随机压缩格式（Zip, 7z, TarGz）、压缩级别（$0 \sim 12$）、加密选项（明文, WinZip AES-256, 7z AES-256）与密码。
- **Rationale (选择理由)**:
  `proptest` 支持失败样本自动最小化缩减（Shrinking），一旦发现解压数据与源数据不匹配，能在毫秒级内自动找到最小复现 Payload（例如仅 3 字节的边界用例），极大提高排错效率。
- **Alternatives Considered (被否决方案)**:
  - *方案 A（手写 `rand::Rng` 随机循环）*：被否决。缺乏自动 Shrinking 机制，遇到崩溃时生成的巨大样本难以人工定位根因。
- **Source (查阅依据)**:
  - [proptest book documentation](https://proptest-rs.github.io/proptest/intro.html)
  - `rust/ttzip-glue/src/zip/mod.rs`
  - `rust/ttzip-glue/src/sevenz/mod.rs`

---

### R002: 覆盖率引导模糊测试 Harness 架构 (Coverage-Guided Fuzzing Architecture)

- **Decision (选定方案)**:
  采用 **双模 Fuzzing 架构**：
  1. **标准 CI 模式 (`tests/fuzz_harness.rs`)**: 编写可直接由 `cargo test` 运行的 100,000+ 轮变异测试循环，对格式解析器持续注入字节翻转、截断、溢出偏移与恶意路径；
  2. **LLVM libFuzzer 模式 (`fuzz/`)**: 声明结构化 Fuzz Target，供专业安全审计与长期演进使用。
- **Rationale (选择理由)**:
  双模架构确保在常规开发和快速本地 CI 门禁（`scripts/ci_gate.sh`）中秒级运行 100% 绿色，同时具备专业安全审计所必需的 LLVM 覆盖率引导深潜能力。
- **Alternatives Considered (被否决方案)**:
  - *方案 A（仅使用外部命令行 Fuzzer）*：被否决。无法平滑嵌入本地 pre-push 门禁与 `swift test` 自动化链路。
- **Source (查阅依据)**:
  - `rust/ttzip-glue/src/zip/parser.rs`
  - `rust/ttzip-glue/src/sevenz/header.rs`
  - `rust/ttzip-glue/src/fs/safe_extract.rs`

---

### R003: 基于 `criterion.rs` 的纳秒级高精微基准套件 (Criterion Micro-benchmarking)

- **Decision (选定方案)**:
  在 `rust/ttzip-glue/benches/` 建立独立 Benchmark Targets：
  - `bench_crypto.rs`: 评测 CRC32 PMULL 12-way, Adler32 UDOT, AES-256 CTR/CBC, SHA-256 KDF；
  - `bench_codecs.rs`: 评测 `libdeflate`, `zstd`, `fast-lzma2`, `snappy`, `lz4`, `lzfse` 块吞吐；
  - `bench_zip_parallel.rs`: 评测多核并行 ZIP 压缩与解压吞吐；
  - `bench_sevenz_solid.rs`: 评测 7z Solid 固实单条目流式切片提取延迟。
- **Rationale (选择理由)**:
  `criterion` 提供高斯统计模型分析、自动预热（Warmup）、消除离群值并自动生成 HTML 交互式吞吐图表，比传统粗粒度计时精确 1000 倍。
- **Alternatives Considered (被否决方案)**:
  - *方案 A（使用 Rust 内建 unstable `#[bench]`）*：被否决。必须依赖 nightly Rust，破坏当前 stable 工具链约束。
- **Source (查阅依据)**:
  - [Criterion.rs Documentation](https://bheisler.github.io/criterion.rs/book/index.html)
  - `rust/ttzip-glue/src/crypto/`

---

### R004: Rust 原生双向差分预言机与 CVE 黄金语料库 (Differential Oracle & Golden Corpus)

- **Decision (选定方案)**:
  在 `rust/ttzip-glue/tests/differential_oracle.rs` 中：
  1. 通过 `std::process::Command` 调用系统 `/usr/bin/unzip` 与 `/usr/bin/tar`，验证双向互解一致性；
  2. 嵌入已知历史 CVE 畸变样本的 Base64 / ASCII 编码，并在内存中还原，严格断言 Rust 解析器 100% 优雅拒绝且不崩溃。
- **Rationale (选择理由)**:
  符合宪法四大铁律之“真实预言机”，杜绝自欺欺人的单一逻辑闭环，以工业级系统工具为金标准。
- **Source (查阅依据)**:
  - `Tests/TTZipTests/DifferentialOracleTests.swift`
  - `Tests/TTZipTests/LibarchiveGoldenCorpusTests.swift`
