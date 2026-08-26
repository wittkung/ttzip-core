# Feature Specification: TTZip 核心胶水层全面迁移 Rust 架构方案 (Feature 168)

**Feature ID**: `168-rust-bridge-glue-migration`  
**Created**: 2026-08-21  
**Status**: In Progress (Specification Phase)  
**Priority**: P0 (Architectural Evolution, Memory Safety, Cross-Platform Engine)

---

## 1. Executive Summary & Background

TTZip 当前的高性能压缩与解压引擎采用了典型的双层/三层混合架构：
1. **上层表现与业务层**：由 Swift 6.0 实现的 SwiftUI 桌面客户端 (`TTZipApp`)、命令行工具 (`TTZipCLI`) 以及核心业务编排 (`TTZipCore`)。
2. **中间胶水与适配层 (`CTTZipBridge`)**：由大量 C11 代码（~45 个 C 源文件与 110+ 头文件）编写的桥接与中枢层，承担底层静态库封装、内存映射 (`mmap`)、无锁流式分块 (`CTTZipBridge_ZipChunkedStream`)、ARM64 NEON SIMD 向量加速、7z/ZIP 容器格式嗅探与解析、POSIX 权限与安全路径防御、以及跨线程池调度等高危与高密逻辑。
3. **底层编解码静态库 (`Vendor/*.a`)**：包括 `libarchive`, `libdeflate`, `zstd`, `fast-lzma2`, `snappy`, `lzfse`, `libbz2`, `uchardet`, `libxml2` 等原生 C/C++ 开源库。

### 现状痛点分析
- **内存安全性与生命周期隐患**：C11 缺乏所有权系统与编译期生命周期跟踪，全靠手动 `magic` 校验、手动配对 `posix_memalign`/`free` 以及严格的防御性注释。在复杂的流式解压、取消、多线程竞态与畸变压缩包对抗中，极易出现 Use-After-Free、Double Free 或野指针泄漏。
- **Swift <-> C FFI 样板代码沉重**：Swift 端需充斥大量的 `withUnsafePointer`, `UnsafeMutableRawPointer`, `CUnsafeBufferAdapter` 以及容易出错的结构体对齐和类型转换；错误传递受限于 C 返回值与错误码，缺乏类型安全的统一 Result / Error 模型。
- **多平台与跨架构维护成本**：C 胶水层为了实现线程池、内存预算控制与跨平台抽象，重复实现了较多底层基础设施，增加了长期演进与维护的认知负荷。

### 迁移目标
构建高内聚、强安全、零开销的 **Rust 核心胶水引擎 (`ttzip-rs` / `ttzip-glue`)**，全面替代 `CTTZipBridge` 及 Swift 端脆性不安全指针胶水代码：
1. **内存安全与编译期防御**：利用 Rust 的所有权（Ownership）、借用检查（Borrow Checker）与 RAII 自动资源管理，彻底根除 C 胶水层的内存泄漏、Use-After-Free 和数据竞争。
2. **零开销 FFI 与稳定 C-ABI**：通过 `extern "C"` / `#[repr(C)]` 导出干净的 C 接口，向上提供 Swift Package Manager (SPM) 零拷贝直接调用的头文件与模块，向下封装调用 Vendor 静态库。
3. **无畏并发与流式第一性**：采用 Rust 现代化流式微缓冲模型（`std::io::Read` / `std::io::Write` trait），常驻内存严控在 $\le 64\text{MB}$，结合 Rust 无锁/结构化并发替代 C 手写线程池。
4. **极致性能确界与硬件加速**：充分保留并发挥 Apple Silicon ARM64 NEON 与 Crypto 扩展（AES-256 / SHA-256 / CRC32）的硬件算力，严格守住 Constitution 规定的吞吐下限。

---

## 2. User Scenarios

### User Scenario 1 (US1) - 零崩溃对抗对抗性与畸变归档 (Zero-Crash Resilient Extraction)
- **As a**: 经常处理不可信网络下载、损坏或恶意构造归档文件的 macOS 终端与桌面用户
- **I want to**: 解压包含超长文件名、畸变文件头、目录穿越 (`../`)、非法 UTF-8、巨大解压比的“压缩炸弹”文件
- **So that**: Rust 胶水层在解压与流式解析的最前线完成严格的边界检查与安全收敛，以类型安全的 `Result::Err` 优雅通知上层 UI/CLI，保证进程绝对零崩溃、零内存越界、零段错误。

### User Scenario 2 (US2) - 极速零拷贝流式压缩与解压 (Zero-Copy High-Throughput Streaming)
- **As a**: 批量备份几十 GB 视频、源码与大数据集的专业开发者
- **I want to**: 执行多核并行 ZIP/7z/Tar/Zstd 压缩与解压
- **So that**: Rust 胶水层维持裸指针切片零拷贝（Zero-Copy Slicing）与微缓冲流水线，充分榨干 Apple Silicon M 系列多核与 NEON 向量单元，达到与纯 C11 相同甚至更优的吞吐量（ZIP 解压 $\ge 4500\text{ MB/s}$，Level 1 压缩 $\ge 1500\text{ MB/s}$）。

### User Scenario 3 (US3) - 确定性取消与瞬时资源回收 (Deterministic Cancellation & Instant Cleanup)
- **As a**: 在压缩/解压 100GB 大文件过程中点击“取消”按钮的桌面用户
- **I want to**: 任务在毫秒级内安全终止
- **So that**: Rust 的 RAII `Drop` 语义自动回收所有预分配环形缓冲、关闭底层打开的文件描述符、安全终止工作线程，瞬间将系统内存归还操作系统，无任何悬挂句柄或临时文件残留。

### User Scenario 4 (US4) - 统一跨平台核心与 Swift/CLI 极简调用 (Unified Cross-Platform Core & Thin Swift Bindings)
- **As a**: TTZip 核心开发者与开源贡献者
- **I want to**: 为 Swift GUI、CLI 工具及未来跨平台目标提供统一、自包含且文档完备的 Rust 核心静态库
- **So that**: 仅需通过极简的 C ABI 接口与 Swift 进行高阶交互，无需在 Swift 端编写数百行脆弱的手动指针分配与 C 结构体内存对齐代码。

---

## 3. Functional Requirements

### REQ-001: Rust 胶水层工作区与架构布局 (Workspace & Crate Architecture)
- 在工程中建立自包含的 Rust 工作区 `rust/`，核心库为 `ttzip-glue`（或 `ttzip-rs`），输出 `crate-type = ["staticlib", "cdylib"]`。
- Rust 层通过 `Cargo.toml` 管理依赖，并支持生成统一的 C 头文件 `ttzip_rust.h` 与 `module.modulemap`，无缝嵌入 Swift Package Manager。

### REQ-002: Vendor 静态 C 库的安全 Rust 封装 (Safe Vendor Library FFI Wrappers)
- 对 `libarchive`, `libdeflate`, `zstd`, `fast-lzma2`, `snappy`, `lzfse`, `uchardet` 等 C 库提供经过严格生命周期与安全性包装的 Rust Safe API。
- 严禁在上层业务逻辑中暴露任何原始裸指针，所有 C 结构体句柄封装在实现了 `Drop` 的安全 Rust 包装器中。

### REQ-003: 跨 FFI 边界的异常屏障与强类型错误传递 (FFI Exception Barriers & Result Mapping)
- 所有的 `extern "C"` 导出的 FFI 函数入口处必须包裹 `std::panic::catch_unwind`，严禁让 Rust Panic 跨越 FFI 边界穿透到 Swift / C 运行时（导致未定义行为 UB）。
- 定义标准化的 `TTZipErrorCode` 枚举与错误消息回传机制，支持在 Swift 端直接转换为强类型 Swift `TTZipError`。

### REQ-004: 流式第一性与微缓冲内存管理 (Stream-First Micro-Buffering & Zero-Allocation)
- 遵循四大工程铁律之“流式第一性”，Rust 核心管道必须基于流式 Pull/Push 模型（`Read`/`Write`），单任务常驻内存严控在 $\le 64\text{MB}$。
- 热路径缓冲区必须使用预分配/重用机制，严禁在紧凑解压/压缩循环中频繁进行堆分配 (`Vec::new()` / `Box::new()`)。

### REQ-005: 硬件级向量与密码加速保持 (NEON SIMD & Hardware Crypto Acceleration)
- 利用 Rust 的 `core::arch::aarch64` 内部指令或经过验证的高性能底层汇编，实现 Apple Silicon ARM64 CRC32、Adler32、AES-256-CBC、SHA-256 的硬件加速，确保吞吐指标与现有 C11 NEON 实现 100% 对齐。

### REQ-006: 纵深防御的路径与权限安全体系 (Deep Path Sanitization & Deferred Fixups)
- 在 Rust 层实现严苛的路径规范化与穿越检测（防御 ZipSlip、绝对路径、多重 `..`、非法控制字符）。
- 文件系统写入时采用安全两步法：临时采用受限权限 (`0700`) 创建目录，提取完成后按深度倒序（Bottom-Up）回写 POSIX 权限与修改时间 (mtime)，彻底免疫 TOCTOU 符号链接劫持。

### REQ-007: 结构化并发与多核负载调度 (Fearless Structured Concurrency & Thread Budgeting)
- 使用 Rust 原生或精简的无锁工作窃取（Work-Stealing）机制替代 C11 手写线程池。
- 支持 Apple Silicon P/E 核感知与任务并发度限制，禁止在热路径并发闭包内使用阻塞性锁。

### REQ-008: 进度报告、取消令牌与日志中枢集成 (Progress, Cancellation & Logging Bridge)
- 导出类型安全的进度通知回调与原子取消令牌（`AtomicBool` / `AtomicU64`）。
- Rust 端的 `log` / `tracing` 框架接入统一 C 桥接日志回调，将所有诊断与告警无缝路由至 Swift `TTLogger`，全库严禁裸 `println!` / `eprintln!`。

---

## 4. Success Criteria

1. **内存安全确界 (Zero Memory Vulnerabilities)**:
   - 全套迁移模块在 AddressSanitizer (ASan)、UndefinedBehaviorSanitizer (UBSan) 与 Miri 检查下达到 0 泄漏、0 越界、0 未定义行为。
2. **测试与正确性全绿 (100% Test Green)**:
   - Swift 现存 525+ 单元测试 100% 通过；
   - 新增 Rust 单元测试与端到端差分测试（Differential Testing）100% 通过。
3. **性能底线无倒退 (Zero Performance Regression)**:
   - ZIP Level 1 压缩吞吐 $\ge 1500\text{ MB/s}$；
   - ZIP 解压吞吐 $\ge 4500\text{ MB/s}$；
   - ZIP AES-256 解密吞吐 $\ge 1800\text{ MB/s}$；
   - `./scripts/benchmark_ab.sh` 5 轮采样评测结果为 `PASSED_NO_REGRESSION`。
4. **编译与打包集成无摩擦 (Seamless SPM & CI Integration)**:
   - 支持 `swift build` 与 `swift test` 一键联动构建，无需手动预编译；
   - 生成的二进制包体积增量控制在 $\le 8\%$ 以内；
   - 符合 Mac App Store (MAS Sandbox) 审计规范。

---

## 5. Scope & Phasing Strategy (迁移范围与分期规划)

### Phase 1: 基础设施与最小可行 FFI (In-Tree Rust Setup & Core Utilities)
- 搭建 `rust/ttzip-glue` Cargo 工作区及 SPM/CMake 联合构建脚本；
- 实现基础内存缓冲区管理、错误处理屏障、日志中枢与取消令牌；
- 迁移基础算法：CRC32/Adler32 NEON 向量计算与字符编码探测 (`uchardet` 绑定)。

### Phase 2: 单格式编解码安全胶水迁移 (Single-Format Codec Safe Wrappers)
- 迁移 `libdeflate`, `zstd`, `lz4`, `fast-lzma2` 的 Rust 安全封装；
- 替换 `CTTZipBridge_ZipChunkedStream.c` / `CTTZipBridge_ZipWrite.c`，实现流式分块压缩与解压。

### Phase 3: 归档容器与复杂解析器迁移 (Archive Containers & libarchive Glue)
- 迁移 `libarchive` 深度绑定与安全文件系统落盘引擎；
- 迁移 7z 容器头解析器、Solid 固实流式跳过逻辑与 AES-256 / SHA-256 密码引擎；
- 移除遗留的 `CTTZipBridge` 冗余 C 文件。

### Phase 4: Swift 端瘦身与架构收敛 (Swift Slimming & Full Verification)
- 重构 `TTZipCore`，移除 Swift 中所有低级 C 指针适配器，直接接入 Rust 生成的干净 API；
- 全量运行 525+ 单元测试、性能压测与 A/B 差异化验证。

---

## 6. Assumptions & Non-Goals

### Assumptions
- 开发与构建环境具备 Rust 1.80+ 工具链（`cargo`, `rustc`）与 Clang/LLVM；
- 目标平台以 macOS 14.0+ (`aarch64-apple-darwin` 与 `x86_64-apple-darwin`) 为主。

### Non-Goals
- 不使用 Rust 重写纯 Swift 的 UI 渲染层 (`TTZipApp` SwiftUI 代码)；
- 不重新从零手写完整的 DEFLATE / ZSTD / LZMA2 编解码器（底层成熟算法库继续复用成熟的 `Vendor` C/C++ 静态库，Rust 负责胶水、编排、生命周期、安全防御与流式管道）。
