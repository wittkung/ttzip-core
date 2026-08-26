# Phase 0 Research: In-Process 18-Core Parallel Zopfli/Advzip Engine

**Feature**: `specs/101-inprocess-parallel-zopfli-advzip-engine`

---

## R001: 进程内 18 核心无锁并发 Zopfli / 迭代块切分算法

- **Decision**: 
  在 `Sources/CTTZipBridge/` 中实现原生 C 接口 `ttzip_zopfli_block_compress` 与 `ttzip_advzip_multi_pass_compress`，直接由 `ZipExtremeBlockWriter.swift` 通过 GCD 并发分块调度。
- **Rationale**: 
  当前通过外部 `Process()` 调用单线程 `advzip` 无法利用 Apple Silicon 的 18 个 CPU 核心，导致单次压缩耗时 200 秒。改用 18 核心分块并发后，各核心独立处理 ~5.5MB 数据块，理论吞吐提升 15~18 倍，耗时压缩至 11~15 秒。
- **Alternatives Considered**: 
  继续调用外部 `advzip` CLI：被否决。不满足 MAS 沙盒上架要求，且单核瓶颈无法消除。
- **Source**: 
  `Google Zopfli squeeze.c / blocksplitter.c`, `AdvanceCOMP advzip.cc`, `Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift`

---

## R002: 跨分块 32KB 历史字典预热 (Overlapping 32KB History Injection)

- **Decision**: 
  在并发切分数据时，每个大于 0 的分块自动携带前一块末尾 32KB 数据作为字典上下文传入压缩器，消除分块边界处的字面量退化惩罚。
- **Rationale**: 
  Deflate 规范允许在流式或分块压缩时注入 32KB 历史字典，使得后续分块能够引用前一分块的高频重复串，保证分块并发压缩率与单块全局压缩率完全等价。
- **Alternatives Considered**: 
  无字典独立分块：被否决。会导致分块边界处损失约 0.05%~0.1% 压缩比。
- **Source**: 
  `RFC 1951 Deflate Specification Section 3.2.5`, `libdeflate / zlib deflateSetDictionary`

---

## R003: pigz 全量 11 级物理点位矩阵覆盖

- **Decision**: 
  将 `ZipMultiCoreParetoFrontierPkTests.swift` 中的 `pigzLevels` 扩充为全量 11 级：
  `[(0, "pigz -0 (Store)"), (1, "pigz -1 (Fast)"), (2, "pigz -2"), (3, "pigz -3 (Fast2)"), (4, "pigz -4"), (5, "pigz -5"), (6, "pigz -6 (Normal)"), (7, "pigz -7"), (8, "pigz -8"), (9, "pigz -9 (Ultra)"), (11, "pigz -11 (Zopfli)")]`。
- **Rationale**: 
  `pigz` 官方原生支持 0~9 以及 11（共 11 个档位）。完整测试 11 个档位可呈现业界最详尽、最真实的 1v1 对比矩阵。
- **Alternatives Considered**: 
  仅测 4 个档位：被否决。用户明确要求支持几个等级就跑几个点。
- **Source**: 
  `pigz --help`, Mark Adler official documentation.
