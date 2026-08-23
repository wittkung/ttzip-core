# Feature Specification: ZIP 全链路压缩流算法全景架构调研与外部依赖审计

**Feature ID**: `106-zip-compression-stream-architecture-audit`  
**Status**: DRAFT  
**Created**: 2026-08-19  
**Category**: Architecture Audit & Deep Systems Research  

---

## 1. Executive Summary & Goals

本项目（TTZip）作为面向 macOS 14+ 的高性能原生归档引擎，其 ZIP 格式支撑了从极速直通（Store $6.5\text{ GB/s}$）到极限重压（Zopfli 15 轮 Squeeze $2.85\text{ MB}$）的 8 大档位完整矩阵。

为摸清当前 ZIP 模块下所有压缩流算法的底层物理实现、外部 C 静态库/系统库绑定情况以及自研/优化算法的分布，本特性旨在进行**地毯式全景架构调研与深度代码审计**：
1. **全景路由拓扑映射**：绘制从 Swift 顶层调用（`ZipArchiver`、`ZipExtremeBlockWriter`、`ZipParallelWriter`、`ZipDirectoryScanner`）到底层 C 桥接中枢（`CTTZipBridge`）的完整数据流与控制流分发路径；
2. **底层实现剖析与技术源流归类**：逐一审查每个压缩流算法（Deflate、Zopfli、Store、bzip2、LZMA 等）是自研实现、内联 C 优化、调用 `Vendor/*.a` 静态库，还是调用 macOS 系统内置动态库（`libz`、`libbz2`）；
3. **内存与并发模型审计**：审计各流式算法在分块（Chunking）、跨块字典预热（32KB History Preconditioning）、位流拼接（`Z_SYNC_FLUSH` / Bit Accumulator）以及内存分配（零拷贝 vs 堆分配）上的真实物理行为；
4. **输出完整架构审计报告与调用依赖全景矩阵**：形成权威参考文档，指导后续 upstream 贡献与自主引擎演进。

---

## Clarifications

- **Q1: 调研范围是否覆盖多文件归档与流式管道？**  
  **A1**: 覆盖全量场景，包含：单文件极速/极限压缩流（`ZipExtremeBlockWriter`）、多文件高并发归档器（`ZipParallelWriter`）、流式管道（`ZipStreamPipeline` / `ZipStoreStreamWriter`）、加密流（AES-256 / ZipCrypto）与自适应路由中枢（`ZipCompressionProfile`）。
- **Q2: 外部依赖审计的完整边界？**  
  **A2**: 涵盖 3 大层级：
    1. **系统动态库 (`systemLibrary`)**：macOS 系统自带的 `libz.tbd` (zlib 1.2.12 / Apple NEON)、`libbz2.tbd`、`libiconv.tbd`、`libxml2.tbd`；
    2. **预编译静态库 (`Vendor/*.a`)**：`libdeflate.a`、`libarchive.a`、`liblzma.a`、`libzstd.a`、`liblz4.a`、`libb2.a`；
    3. **内嵌/自研 C 模块 (`Sources/CTTZipBridge/`)**：`zopfli/` (Google Zopfli C 源码)、`CTTZipBridge_ZipWrite*.c`、`CTTZipBridge_Crypto.c`、`ttzip_lzma2_*.c`、`uchardet/`。


---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: 压缩流算法全景资产清单 (Inventory & Classification)
- **Actor**: 架构师 / 核心开发人员
- **Scenario**: 需要明确 TTZip 在 ZIP 格式下到底支持哪些压缩方法（Method ID 0, 8, 12, 14, 93 等），每个方法的底层实现位于哪个文件、行号及符号。
- **Acceptance Criteria**:
  - 列出 ZIP 全压缩流算法矩阵，明确标注：算法名称、ZIP Method ID、Swift 调度入口、C 桥接符号、底层核心源文件路径及外部库依赖类型（`Vendor 静态库` / `系统 libc/libz` / `CTTZipBridge 自研/内嵌`）。

### User Scenario 2: 外部库依赖调用链路深度追踪 (Dependency Tracing)
- **Actor**: 安全审计员 / 开源合规专家
- **Scenario**: 审计所有调用外部 C 库或系统库的边界，确认是否存在未声明的动态链接、外部 CLI 子进程调用（零外部 CLI 铁律）或隐式全局锁竞争。
- **Acceptance Criteria**:
  - 100% 确认无任何外部 CLI 进程调用（`Process()` / `posix_spawn()` 仅用于黑盒测试预言机，核心引擎 100% 进程内）；
  - 明确列出所有调用的外部库：`libdeflate`、`libarchive`、`zlib` (系统/zlib-ng)、`Google Zopfli` (Vendor/CTTZipBridge)、`LZMA SDK`、`libzstd`。

### User Scenario 3: 8 大档位数据流动与并发拼接机制 (Pipeline & Concurrency Audit)
- **Actor**: 性能优化工程师
- **Scenario**: 深入理解 8 大档位从输入数据切分到 18 核心并发压缩、字典预热与 ZIP Local Header / Central Directory 写入的物理流程。
- **Acceptance Criteria**:
  - 绘制并文档化 8 大档位的流水线模型，包含内存生命周期与 RFC 1951 字节对齐机制。

---

## 3. Functional Requirements

- **FR-001**: 遍历并审计 `Sources/TTZipCore/Zip/` 下所有 Swift 文件，提取所有涉及压缩流构建、算法分发与写入的类与结构体。
- **FR-002**: 遍历并审计 `Sources/CTTZipBridge/` 下所有与 ZIP/Deflate/Zopfli 相关的 C 文件与头文件，建立 C 导出符号索引。
- **FR-003**: 审计 `Vendor/` 下各第三方静态库（`libdeflate.a`、`libarchive.a`、`liblzma.a`、`zopfli-upstream` 等）与 TTZip 的链接与调用关系。
- **FR-004**: 审计 macOS 系统动态库（`/usr/lib/libz.dylib`、`libbz2.dylib`、`libiconv.dylib` 等）的符号绑定。
- **FR-005**: 产出包含完整文件路径、行号与调用链的架构白皮书 `docs/architecture/zip_compression_stream_comprehensive_audit.md`。

---

## 4. Success Criteria

1. **覆盖度 100%**：覆盖 `Sources/TTZipCore/Zip/` 与 `Sources/CTTZipBridge/` 中 100% 的 ZIP 相关源文件；
2. **事实确界**：引用的每一个函数名、结构体、文件名与行号必须通过物理代码查阅确证，绝对零幻觉；
3. **分类清晰**：清晰区分自研优化、C 桥接内嵌与外部静态/动态库；
4. **工件完整**：完整交付全套 Spec Kit 设计与审计工件。
