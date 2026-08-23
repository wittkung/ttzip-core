# Implementation Plan: 7z 全链路原生压缩流算法全景调研与自主无依赖引擎演进

**Branch**: `108-7z-native-compression-pipeline` | **Date**: 2026-08-19 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/108-7z-native-compression-pipeline/spec.md`

---

## 1. Summary

本项目针对当前 TTZip 在 7z 格式下对外部静态库 `liblzma.a`（XZ Utils）与内嵌第三方源码库 `fast-lzma2/` 的依赖现状，展开全景底层架构调研与深度代码审计。结合 ZIP 引擎中成熟验证的 SWAR/NEON 混合匹配查找、无锁多核分块调度、APFS 磁盘预分配与自适应动态熵旁路机制，规划出 100% 纯自研、零外部依赖的高性能 7z/LZMA2 编解码流水线架构。

---

## 2. Technical Context

- **Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`) + C11 / POSIX APIs.
- **Primary Dependencies**: 100% 进程内纯原生 C 绑定（`CTTZipBridge`），淘汰 `liblzma.a` 与 `fast-lzma2/`，零外部 CLI 依赖。
- **Storage**: APFS 文件系统空间预分配（`fstore_t`）+ 直接 I/O `pwrite` 分块写入。
- **Testing**: `swift test` (525+ 单元测试), `XCTestPerformanceMeasureTests` 性能门禁测试, `ttzip-cli bench -f 7z` 基准测试。
- **Target Platform**: macOS 14.0+ (Apple Silicon M 系列优先，兼容 x86_64)。
- **Project Type**: 高性能系统级归档引擎与桌面应用。
- **Performance Goals**:
  - 7z Store (L0): $\ge 25,000\text{ MB/s}$ (历史最优 28,926 MB/s)
  - 7z Fast (L1): $\ge 3,800\text{ MB/s}$ (门禁 $\ge 3,200\text{ MB/s}$)
  - 7z Normal (L5): $\ge 600\text{ MB/s}$ (门禁 $\ge 480\text{ MB/s}$)
  - 7z Decompress: $\ge 7,500\text{ MB/s}$ (门禁 $\ge 6,600\text{ MB/s}$)
  - 7z AES-256 KDF: $\le 15\text{ ms}$ (实测 11ms)
- **Constraints**: 热路径零内存堆分配（Zero Dynamic Allocation in Inner Loops）、无全局锁、零 `Data(count:)` 内核清零页故障。
- **Scale/Scope**: 覆盖 7z 全部 14 个 Swift 源文件、16 个底层 C 源文件以及相关的 80+ 测试用例。

---

## 3. Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 宪章核心法则 | 状态 | 实施验证断言 |
| :--- | :--- | :--- |
| **热路径零成本抽象 (Zero-Cost Abstraction)** | **PASS** | 编码/解码热循环内部 100% 采用预分配 Pack Arena 与栈局部变量，零 `malloc`/`free`。 |
| **Fast-Path 旁路保留原则** | **PASS** | 保留单文件 mmap 零拷贝直通、高熵数据 0x01/0x02 原始块直通与 Store 极速直通。 |
| **吞吐硬门禁 (Hard Floor Enforcement)** | **PASS** | 7z L1 $\ge 3,200\text{ MB/s}$, 7z 解压 $\ge 6,600\text{ MB/s}$, 7z L5 $\ge 480\text{ MB/s}$ 全部设立物理断言。 |
| **四大系统工程铁律 (The Four Invariants)** | **PASS** | 流式第一性（微缓冲）、纵深防御（POSIX 原语）、确定性确界（Magic 结构体与凭据安全擦除）、真实预言机（双向差分测试）。 |
| **ZIP 冻结保护** | **PASS** | 本特性仅针对 7z 模块及通用 C 匹配查找器进行调研与自研演进，严禁修改 ZIP 冻结核心源文件。 |

---

## 4. Phase 0: Research Items Index

- R001 [SUBAGENT:research] 《纯原生 LZMA2 Range Decoder 算法架构与 NEON 向量化加速设计》：调研纯原生 C11 LZMA2 解码器与无分支 Range Decoder，设计 Direct Linear Slicing 与 NEON 64B 向量匹配复制方案以替代 `liblzma.a`。
- R002 [SUBAGENT:research] 《Double-Fast / HC3 匹配查找器与极速 LZMA2 编码器架构（Level 1-2）》：调研基于 Double-Fast (DF-4/8) 512KB L2 缓存表、ARMv8 ACLE CRC32 硬件哈希与 2MB 规范分块的极速 LZMA2 编码器以替代 `lzma_raw_buffer_encode`。
- R003 [SUBAGENT:research] 《多核无锁 Radix / BT4 匹配查找器与代价驱动最优解析器设计（Level 5-9）》：调研自研 Radix-16 / BT4 匹配查找器与 Bit Cost DP 最优解析器，规划彻底剔除外部 `fast-lzma2/` 目录。

---

## 5. Phase 1: Design Artifacts Index

- **Research Document**: [research.md](./research.md)
- **Data Model**: [data-model.md](./data-model.md)
- **Contracts**:
  - [7z_encoder_contract.json](./contracts/7z_encoder_contract.json)
  - [7z_decoder_contract.json](./contracts/7z_decoder_contract.json)
  - [7z_audit_contract.json](./contracts/7z_audit_contract.json)
- **Quickstart & Verification Guide**: [quickstart.md](./quickstart.md)

---

## 6. Project Structure & Component Change Breakdown

```text
TTZip/
├── Sources/
│   ├── CTTZipBridge/
│   │   ├── CTTZipBridge_7z.c                  # [MODIFY] 7z 核心调度中枢与原生分发
│   │   ├── CTTZipBridge_7zStore.c             # [RETAIN] 7z Store 极速直通引擎 (28,000+ MB/s)
│   │   ├── CTTZipBridge_7zNativeDecoder.c     # [MODIFY] 7z 原生多块并行解压流程
│   │   ├── ttzip_7z_block_decoder.c           # [MODIFY] 7z 控制字节解析与纯原生多块分发
│   │   ├── ttzip_lzma2_dec_native.c           # [MODIFY] 纯自研 LZMA2 Range Decoder (消除 liblzma)
│   │   ├── ttzip_lzma2_enc_native.c           # [MODIFY] 纯自研多核 LZMA2 编码器调度与 Arena 管理
│   │   ├── ttzip_lzma2_fast_encoder.c         # [MODIFY] 完善自研 Double-Fast 极速编码器 (L1-2)
│   │   ├── ttzip_fl2_bridge.c                 # [MODIFY] 切换为纯自研引擎调度 (准备剔除 FL2)
│   │   ├── ttzip_7z_header_parser.c           # [RETAIN] 7z 元数据零拷贝解析器
│   │   ├── ttzip_7z_header_writer.c           # [RETAIN] 7z 元数据序列化与刷盘
│   │   ├── ttzip_7z_kdf_arm64.c               # [RETAIN] ARMv8 SHA-256 硬件 KDF 派生
│   │   ├── ttzip_7z_crypto_neon.c             # [RETAIN] ARM NEON AES-256 并发解密
│   │   └── ttzip_bcj_arm64_neon.c             # [RETAIN] ARM64 BCJ 分支过滤
│   └── TTZipCore/
│       ├── SevenZip/
│       │   ├── SevenZipEngine.swift           # [MODIFY] 7z 核心引擎调度入口
│       │   ├── NativeSevenZipEngine.swift     # [MODIFY] 原生 7z 引擎 Swift 门面
│       │   └── SevenZipModels.swift           # [MODIFY] 7z 领域模型与参数配置
│       └── Adapters/
│           └── SevenZipCAdapter.swift         # [MODIFY] C 桥接层 Swift 适配器
└── specs/
    └── 108-7z-native-compression-pipeline/
        ├── spec.md                            # 规格定义
        ├── checklists/requirements.md         # 质量检查单
        ├── plan.md                            # 架构规划 (本文件)
        ├── research.md                        # Phase 0 研究结论
        ├── data-model.md                      # Phase 1 数据模型
        ├── contracts/                         # Phase 1 JSON Schema 契约
        │   ├── 7z_encoder_contract.json
        │   ├── 7z_decoder_contract.json
        │   └── 7z_audit_contract.json
        └── quickstart.md                      # Phase 1 验证指南
```
