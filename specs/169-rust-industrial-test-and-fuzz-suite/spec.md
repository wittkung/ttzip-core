# Feature Specification: TTZip 工业级 Rust 属性测试、模糊测试与高精基准测试体系 (Feature 169)

**Feature ID**: `169-rust-industrial-test-and-fuzz-suite`  
**Created**: 2026-08-21  
**Status**: In Progress (Specification Phase)  
**Priority**: P0 (Industrial Quality, Memory Safety Invariant, Coverage-Guided Fuzzing)

---

## 1. Executive Summary & Background

在 Feature 168 中，TTZip 已成功将核心胶水层、硬件向量与密码算子全面迁移至 Safe Rust（`ttzip-glue`），并通过了现有的 859 个 Swift 测试与 74 个 Rust 单元/集成测试。
然而，为了达到航天级/工业级可靠性标准，测试体系必须进一步升维：
1. **现有测试样本空间有限**: 传统基于固定 Mock 文件和静态 NIST 向量的测试无法穷举复杂文件系统树、极端边界（如空文件、0 字节流、跨 16KB 页边界、Zip64 阈值边界等）；
2. **缺乏覆盖率引导的模糊测试 (Coverage-Guided Fuzzing)**: 面对网络不可信下载的恶意畸变包（ZipSlip、损坏的 Varint、畸变 EncodedHeader、解压炸弹），必须通过 LLVM 覆盖率反馈深入算法内部状态机；
3. **性能基准测量精度与统计学分析**: 需要纳秒级微基准测试（`criterion.rs`）以建立精确的置信区间与回归门禁，防范微小的指令流水线退化。

本特性的目标是：**在 `rust/ttzip-glue` 中全面构建基于 `proptest` 的声明式属性测试集、基于 LLVM libFuzzer 的全覆盖率模糊测试 Harness、基于 `criterion.rs` 的高精微基准套件、以及 Rust 原生的双向差分预言机（Differential Oracle）与 CVE 黄金语料库自愈测试。**

---

## 2. User Scenarios

### User Scenario 1 (US1) - 属性化随机全空间往返验证 (Property-Based Roundtrip Oracle)
- **As a**: TTZip 核心开发者与开源维护者
- **I want to**: 让测试框架自动生成上万组随机字节流、多级随机目录树、随机压缩级别与密码组合
- **So that**: 框架自动验证 $\text{Decompress}(\text{Compress}(\text{data})) \equiv \text{data}$ 不变性，瞬间定位任何边界条件下的数据损坏或内存越界。

### User Scenario 2 (US2) - 覆盖率引导恶意畸变与安全防御确界 (Coverage-Guided Malformed Stream Fuzzing)
- **As a**: 关注安全性的终端用户与企业审计人员
- **I want to**: 确保归档解析引擎在遭遇百万级变异的畸变输入时具备绝对确定性
- **So that**: 引擎面对 ZipSlip 路径穿越、畸变 7z Header、非法 Varint 编码时 100% 优雅返回 `Err(TTZipStatus)`，绝对零崩溃、零段错误、零死循环、零内存爆炸。

### User Scenario 3 (US3) - 统计学纳秒级高精微基准与性能门禁 (High-Precision Statistical Micro-benchmarks)
- **As a**: 性能调优工程师
- **I want to**: 精确测量 ARM64 PMULL CRC32、UDOT Adler32、AES-256-CTR/CBC 以及流式微缓冲在不同数据尺寸下的吞吐
- **So that**: 获得剔除离群值后的置信区间与吞吐曲线，任何 $> 2.0\%$ 的指令流水线性能倒退均被门禁阻断。

### User Scenario 4 (US4) - 跨生态双向差分预言机 (Differential Oracle & Golden Corpus)
- **As a**: 保证 100% 生态兼容性的用户
- **I want to**: 验证 TTZip 生成的归档能被系统 `/usr/bin/unzip` 与 `/usr/bin/tar` 完美解压，且系统工具生成的归档能被 TTZip 正确解析
- **So that**: 保证跨平台、跨工具链的数据无损互操作性。

---

## 3. Functional Requirements

### REQ-001: 基于 `proptest` 的声明式属性化测试 (Property-Based Testing)
- 在 `rust/ttzip-glue/tests/property_tests.rs` 中实现：
  - 单格式编解码器属性测试：`libdeflate` (0~12 级), `zstd` (1~22 级), `fast-lzma2`, `snappy`, `lz4`, `lzfse`；
  - 容器结构属性测试：随机嵌套目录（深度 $0 \sim 10$）、随机文件尺寸（$0 \text{B} \sim 20\text{MB}$）、极端 Unicode/空白路径名；
  - 密码加密属性测试：WinZip AES-256 与 7z AES-256 随机密码/Salt/IV 往返。

### REQ-002: 覆盖率引导模糊测试 Harness (Coverage-Guided Fuzzing)
- 在 `rust/ttzip-glue/tests/fuzz_harness.rs` 实现全套 Fuzz Target：
  - `fuzz_zip_cdfh`: 变异 ZIP Central Directory 与 Extra Fields；
  - `fuzz_sevenz_header`: 变异 7z SignatureHeader、Varint 编解码器与 EncodedHeader；
  - `fuzz_safe_extract`: 注入数万种畸变绝对路径、`..` 相对路径、符号链接与环形引用；
  - `fuzz_stream_fault_injection`: 模拟随机 I/O 截断、网络超时与虚假字节计数。

### REQ-003: 基于 `criterion.rs` 的高精微基准套件 (Criterion Micro-benchmarking)
- 在 `rust/ttzip-glue/benches/` 实现微基准集：
  - `bench_crypto.rs`: CRC32 PMULL 12-way (16B ~ 1MB), Adler32 UDOT, AES-256 CTR/CBC 8-way, SHA-256 KDF (524k 轮)；
  - `bench_codecs.rs`: `libdeflate`, `zstd`, `fl2`, `snappy`, `lz4`, `lzfse` 单核吞吐；
  - `bench_zip_parallel.rs`: 多核并行 ZIP 压缩与解压吞吐；
  - `bench_sevenz_solid.rs`: 7z Solid 固实单条目流式切片提取延迟。

### REQ-004: Rust 原生双向差分预言机与黄金语料库 (Differential Oracle & Golden Corpus)
- 在 `rust/ttzip-glue/tests/differential_oracle.rs` 中实现：
  - 与 macOS 原生 `/usr/bin/unzip` 与 `/usr/bin/tar` 的双向进程差分测试；
  - 历史 CVE 漏洞 ASCII `.uu` 黄金语料库在内存中秒级解码与安全拒绝断言。

### REQ-005: 自动化测试运行与本地 CI 门禁集成 (Automated Runner & Local CI Gate)
- 编写 `scripts/run_rust_tests.sh` 支持运行全套单测、集成测试、属性测试与基准测试；
- 更新 `scripts/ci_gate.sh` 将 Rust 工业级测试套件纳入本地 Pre-Push 门禁。

---

## 4. Success Criteria

1. **属性测试空间覆盖**: `proptest` 至少执行 500+ 轮随机样本组合，100% 往返无损；
2. **Fuzzing 0 崩溃确界**: 对抗 100,000+ 轮伪随机与结构化变异样本，0 段错误、0 Panic 泄漏、0 内存爆炸（常驻 $\le 64\text{MB}$）；
3. **微基准稳定性**: `criterion` 生成完整的吞吐曲线与统计分布，标准偏差 $\le 3.0\%$；
4. **差分测试 100% 吻合**: 与系统原生 `/usr/bin/unzip`、`/usr/bin/tar` 100% 互解一致；
5. **本地 CI 门禁全绿**: 运行 `scripts/ci_gate.sh` 全流程无错误通过。
