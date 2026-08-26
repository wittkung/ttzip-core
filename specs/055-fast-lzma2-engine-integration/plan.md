# Implementation Plan: Fast-LZMA2 Multi-Threaded Engine Integration

**Feature Directory**: `specs/055-fast-lzma2-engine-integration`

**Created**: 2026-08-17

**Status**: Ready for Tasks

---

## Technical Context

TTZip 当前在 7Z / XZ / TAR.XZ 压缩中采用分层设计：
1. **Level 1**：手写 ARM64 NEON 向量化匹配查找器（[ttzip_lzma2_fast_encoder.c](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c)），吞吐达到 3,200 ~ 3,900+ MB/s。
2. **Level 3 ~ 9**：依赖传统 `liblzma`（BT4/HC4 匹配器），单核计算复杂且多线程扩展受限于锁竞争与字典隔离，高压缩等级下吞吐（480 ~ 620 MB/s）无法充分吃满 8~24 核 CPU 算力。

引入 `conor42/fast-lzma2`（BSD 许可），通过其专有的**并行缓冲基数匹配查找器 (Parallel Buffered Radix Match-Finder)**，在 Level 3~9 下实现多核多线程近线性加速，同时保留 L1 原生 NEON 极速路径，构建自适应混合双引擎架构。

---

## Constitution Check

- [x] **Language & Platform Boundaries**: C11 In-tree 源码编译 + Swift 6.0 桥接，支持 macOS 14+ (Apple Silicon 16KB 页对齐) 与 Windows MSVC 编译。
- [x] **Zero-Cost Abstraction on Hot Paths**: 压缩数据平面零动态树分配，零内核页清零，零共享锁争用。
- [x] **Fast-Path Bypass Preservation**: 严格保留现有 Level 1 ARM64 NEON 与全零块快速旁路。
- [x] **Hard Performance Floor**:
  - 7Z Level 1: $\ge 3,200\text{MB/s}$ (Debug) / $\ge 3,900\text{MB/s}$ (Release) 零倒退。
  - 7Z Level 5: $\ge 800\text{MB/s}$ (Debug) / $\ge 1,200\text{MB/s}$ (Release) 翻倍提升。
- [x] **Bounds-First & Deterministic Cleanup**: 字典内存上限控制在 16MB~64MB，16~24 线程常驻内存 $\le 512\text{MB}$，所有 C 上下文嵌入 `0x464C3243` ("FL2C") Magic 标记。
- [x] **Stream-First & Oracle-First**: 支持流式微缓冲拉取，归档与官方 7-Zip、`/usr/bin/tar` 双向差分测试 100% 兼容。

---

## Phase 0: Research Index

- R001 [SUBAGENT:research] 《Fast-LZMA2 In-Tree 编译与 SPM/CMake 构建集成》：详见 [research.md](file:///Users/kevintung/Documents/dev/TTZip/specs/055-fast-lzma2-engine-integration/research.md#r001-fast-lzma2-in-tree-编译与-spmcmake-构建集成)
- R002 [SUBAGENT:research] 《C 桥接接口设计与混合双引擎路由架构》：详见 [research.md](file:///Users/kevintung/Documents/dev/TTZip/specs/055-fast-lzma2-engine-integration/research.md#r002-c-桥接接口设计与混合双引擎路由架构)
- R003 [SUBAGENT:research] 《Apple Silicon 拓扑调度与多核内存确界控制》：详见 [research.md](file:///Users/kevintung/Documents/dev/TTZip/specs/055-fast-lzma2-engine-integration/research.md#r003-apple-silicon-拓扑调度与多核内存确界控制)

---

## Phase 1: Architecture & Design Artifacts Index

- **Data Model**: [data-model.md](file:///Users/kevintung/Documents/dev/TTZip/specs/055-fast-lzma2-engine-integration/data-model.md)
- **Contract Schemas**:
  - `contracts/fl2-block-compression-contract.json` [SUBAGENT:research]
  - `contracts/fl2-stream-compression-contract.json` [SUBAGENT:research]
  - `contracts/fl2-engine-config-contract.json` [SUBAGENT:research]
- **Verification Quickstart**: [quickstart.md](file:///Users/kevintung/Documents/dev/TTZip/specs/055-fast-lzma2-engine-integration/quickstart.md)

---

## Component Changes Breakdown

### 1. C Bridge Layer (`Sources/CTTZipBridge/`)
- **[NEW] `Sources/CTTZipBridge/fast-lzma2/`**: 引入 `conor42/fast-lzma2` 核心 C 源文件（`fl2_compress.c`, `radix_mf.c`, `fl2_pool.c`, `fl2_threading.c`, `lzma2_enc.c` 等）。
- **[NEW] [ttzip_fl2_lzma2.h](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/ttzip_fl2_lzma2.h)**: 声明 `ttzip_fl2_compress_block`、`ttzip_fl2_stream_*` 与混合路由分发接口。
- **[NEW] [ttzip_fl2_bridge.c](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/ttzip_fl2_bridge.c)**: 实现 C 桥接包装层、P-Core 线程绑定、16KB 内存页对齐与 Magic 生命周期管理。
- **[MODIFY] [module.modulemap](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/module.modulemap)**: 导出 `ttzip_fl2_lzma2.h` 头文件。
- **[MODIFY] [Package.swift](file:///Users/kevintung/Documents/dev/TTZip/Package.swift)**: 为 `CTTZipBridge` 追加 `.headerSearchPath("fast-lzma2")` 路径。

### 2. Core Engine Layer (`Sources/TTZipCore/`)
- **[MODIFY] `SevenZipCAdapter.swift`**: 增加针对 `ttzip_fl2_compress_block` 与 Fast-LZMA2 参数的桥接适配。
- **[MODIFY] `NativeSevenZipEngine.swift`**: 在 7Z 归档编码阶段，根据压缩等级智能选择 L1 NEON 编码器或 FL2 多核流水线。
- **[MODIFY] `InMemoryBenchmarkEngine.swift`**: 在基准测试引擎中增加 Fast-LZMA2 多核测试矩阵支持。

### 3. Test & Verification (`Tests/TTZipTests/`)
- **[NEW] `FastLZMA2Tests.swift`**: 单元测试覆盖 FL2 块压缩、流式压缩、多线程调度、差分解压验证与内存释放。
- **[MODIFY] `SevenZipEngineTests.swift`**: 验证 7Z 归档在不同压缩等级下与系统工具及 7-Zip 的双向兼容性。
