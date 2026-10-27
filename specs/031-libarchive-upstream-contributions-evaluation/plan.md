# Implementation Plan: 031-libarchive-upstream-contributions-evaluation

**Feature**: TTZip 技术沉淀向 Upstream libarchive 贡献评估与演进规划 (031-libarchive-upstream-contributions-evaluation)  
**Status**: DESIGN_COMPLETE  
**Phase**: Phase 1 Design Complete  

---

## 1. Technical Context & Upstream Architectural Baseline

本规划旨在系统梳理 TTZip 在原生 C 桥接与汇编加速层的技术积累，筛选除 PR #3388（7-Zip AES-256 解密支持）外最适合回馈开源上游 `libarchive` 的贡献点。

- **Upstream Baseline**: `Vendor/libarchive-upstream/libarchive/` (libarchive master 分支，C99/POSIX)。
- **TTZip Codebase**: `Sources/CTTZipBridge/`, `Sources/TTZipCore/`。
- **Key Invariant**: 向上游提交的改动必须 100% 保持 ABI/API 兼容性，硬件特化必须携带 C99 标量 fallback。

---

## 2. Constitution Check

- [x] **Zero-Cost Hot Paths**: 上游贡献均基于轻量 C 接口与宏包装，无多余堆内存分配与动态抽象。
- [x] **Platform Compatibility**: 覆盖 macOS Darwin (`F_PREALLOCATE`, CommonCrypto)、Linux/POSIX (`posix_fallocate`, OpenSSL/EVP) 与 Windows (CNG)。
- [x] **Freeze Files**: 未修改 TTZip 内部冻结文件（如 `ZipParallelExtractor.swift` 等）。
- [x] **Logging Discipline**: C 桥接层与 libarchive 模块绝不使用裸 `printf`/`fprintf`，统一经由 libarchive 内部错误机制 (`archive_set_error`) 传递。

---

## 3. Phase 0: Outline & Research Index

- - R001 [SUBAGENT:research] 《7-Zip AES-256 写入端加密支持》：评估 `archive_write_set_format_7zip.c` 补齐对称加密流水线与 Multi-Coder `BindPairs` 的可行性。
- - R002 [SUBAGENT:research] 《CRC32 硬件指令集加速》：评估 `archive_crc32.h` 引入 ARMv8 ACLE 与 x86 SSE4.2/PCLMUL 突破 300MB/s 标量瓶颈的可行性。
- - R003 [SUBAGENT:research] 《POSIX / Darwin 磁盘空间预分配》：评估 `archive_write_disk_posix.c` 引入 `fcntl(F_PREALLOCATE)` 与 `posix_fallocate` 的可行性。
- - R004 [SUBAGENT:research] 《ARM64 NEON BCJ 指令向量化》：评估 `archive_read_support_format_7zip.c` 引入 128 位 NEON 向量化跳步算法的可行性。
- - R005 [SUBAGENT:research] 《mmap 顺序文件读取后端》：评估 `archive_read_open_filename.c` 兑现行 439 TODO 引入 `mmap` + `madvise` 的可行性。
- - R006 [SUBAGENT:research] 《Apple BSD LZFSE 压缩过滤器》：评估引入独立 LZFSE 流过滤器的可行性。
- - R007 [SUBAGENT:research] 《libdeflate 与专有引擎边界界定》：评估 libdeflate 全缓冲特性与 libarchive 流式架构的冲突，界定内部保留边界。

*(完整研究结论见 [`specs/031-libarchive-upstream-contributions-evaluation/research.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/031-libarchive-upstream-contributions-evaluation/research.md))*

---

## 4. Phase 1: Design Artifacts & Schemas

- **Data Model**: [`specs/031-libarchive-upstream-contributions-evaluation/data-model.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/031-libarchive-upstream-contributions-evaluation/data-model.md)
- **Contracts**:
  - `contracts/upstream-contribution-schema.json` [SUBAGENT:research]
  - `contracts/crypto-writer-config-schema.json` [SUBAGENT:research]
  - `contracts/preallocation-descriptor-schema.json` [SUBAGENT:research]
- **Validation Guide**: [`specs/031-libarchive-upstream-contributions-evaluation/quickstart.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/031-libarchive-upstream-contributions-evaluation/quickstart.md)

---

## 5. Changes by Component & Contribution Roadmap

### Upstream libarchive PR Roadmap (演进规划路线图)

#### [Tier 1: 最高优先级 / 极高合并概率]
1. **PR 1: 7z AES-256 写入端加密支持**
   - 目标: `Vendor/libarchive-upstream/libarchive/archive_write_set_format_7zip.c`, `archive_cryptor.c`
   - 描述: 完善多 Coder `BindPairs`、随机 Salt/IV 生成、对称加密过滤器与 `kEncodedHeader` 加密。
2. **PR 2: `archive_crc32.h` ARMv8 ACLE & x86 SSE4.2 硬件加速**
   - 目标: `Vendor/libarchive-upstream/libarchive/archive_crc32.h`
   - 描述: 突破 300 MB/s 历史瓶颈，硬件指令单核吞吐提升至 10+ ~ 30+ GB/s。
3. **PR 3: 磁盘空间预分配 (`ARCHIVE_EXTRACT_PREALLOCATE`)**
   - 目标: `Vendor/libarchive-upstream/libarchive/archive_write_disk_posix.c`, `archive.h`
   - 描述: Darwin `fcntl(F_PREALLOCATE)` + Linux `posix_fallocate()`，消除碎片。

#### [Tier 2: 高优先级 / 明确独立优化]
4. **PR 4: ARM64 NEON BCJ 可执行指令向量化加速** (`archive_read_support_format_7zip.c`)
5. **PR 5: `mmap` + `madvise` 顺序文件读取后端** (`archive_read_open_filename.c`)
6. **PR 6: Apple BSD LZFSE 压缩流过滤器** (`archive_read_support_filter_lzfse.c`, `archive_write_add_filter_lzfse.c`)

#### [Tier 3: TTZip 专有保留]
7. **libdeflate 全缓冲多线程压缩引擎** (与 libarchive 流式模型冲突，TTZip 内部保留为 Fast-Path)
8. **Apple Archive (AAR) 专有容器引擎** (macOS 专属框架绑定，TTZip 内部保留)
