# Implementation Plan: TTZip 工业级 Rust 属性测试、模糊测试与高精基准测试体系 (Feature 169)

**Feature ID**: `169-rust-industrial-test-and-fuzz-suite`  
**Created**: 2026-08-21  
**Status**: Planning Phase  
**Artifact**: Architecture & Implementation Plan

---

## 1. Technical Context & Constitution Check

### 1.1 Technical Context
- **组件目标**:
  - `rust/ttzip-glue/tests/property_tests.rs`: 基于 `proptest` 的属性化测试集；
  - `rust/ttzip-glue/tests/fuzz_harness.rs`: 覆盖率引导变异 Fuzzing Harness；
  - `rust/ttzip-glue/tests/differential_oracle.rs`: 跨生态双向差分测试与 CVE 语料库；
  - `rust/ttzip-glue/benches/`: 基于 `criterion.rs` 的 4 个独立 Benchmark 套件；
  - `scripts/run_rust_tests.sh` & `scripts/ci_gate.sh`: 本地 CI 门禁自动化集成。

### 1.2 Constitution Check
- [x] **I. 流式第一性**: Fuzzing 强制断言任意异常流下常驻内存 $\le 64\text{MB}$；
- [x] **II. 纵深防御**: Fuzzing 与属性测试重点覆盖 ZipSlip、`..` 穿越与符号链接劫持；
- [x] **III. 确定性确界**: 变异测试断言 100% 优雅返回 `Err`，0 Panic，0 段错误；
- [x] **IV. 真实预言机**: 与 `/usr/bin/unzip` / `/usr/bin/tar` 双向差分互测，纳秒级 Criterion 门禁防倒退。

---

## 2. Phase 0: Research Items Index

- - R001 [SUBAGENT:research] 《基于 proptest 的声明式属性化生成策略》：设计随机切片、虚拟目录树与参数生成器，支持失败样本自动最小化缩减。
- - R002 [SUBAGENT:research] 《覆盖率引导模糊测试 Harness 架构》：构建 CI 快速变异与专业 libFuzzer 双模 Fuzzing 架构。
- - R003 [SUBAGENT:research] 《基于 criterion.rs 的纳秒级高精微基准套件》：设计硬件密码、编解码器与多核并行基准套件。
- - R004 [SUBAGENT:research] 《Rust 原生双向差分预言机与 CVE 黄金语料库》：设计系统工具差分互测与 CVE 语料库内存自愈测试。

---

## 3. Phase 1: Design Artifacts Index

- **数据模型**: [`specs/169-rust-industrial-test-and-fuzz-suite/data-model.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/169-rust-industrial-test-and-fuzz-suite/data-model.md)
- **强类型契约**:
  - [SUBAGENT:research] `contracts/test_property_contract.json`
  - [SUBAGENT:research] `contracts/test_fuzz_contract.json`
  - [SUBAGENT:research] `contracts/test_benchmark_contract.json`
- **快速验证指南**: [`specs/169-rust-industrial-test-and-fuzz-suite/quickstart.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/169-rust-industrial-test-and-fuzz-suite/quickstart.md)

---

## 4. Component Changes

### 4.1 新建组件
- `rust/ttzip-glue/tests/property_tests.rs`
- `rust/ttzip-glue/tests/fuzz_harness.rs`
- `rust/ttzip-glue/tests/differential_oracle.rs`
- `rust/ttzip-glue/benches/bench_crypto.rs`
- `rust/ttzip-glue/benches/bench_codecs.rs`
- `rust/ttzip-glue/benches/bench_zip_parallel.rs`
- `rust/ttzip-glue/benches/bench_sevenz_solid.rs`
- `scripts/run_rust_tests.sh`

### 4.2 修改组件
- `rust/ttzip-glue/Cargo.toml`: 添加 `proptest`, `criterion`, `tempfile`, `flate2` 等 dev-dependencies 与 `[[bench]]` 配置；
- `scripts/ci_gate.sh`: 集成 Rust 属性测试与基准门禁。
