# Research Report: 048-c-codebase-craftsmanship-and-libarchive-standards

本研究报告基于对 `Sources/CTTZipBridge/` 下核心 C 语言源文件与头文件（`CTTZipCommon.h/.c`, `CTTZipIO.h/.c`, `CTTZipSysAlloc.h/.c`, `CTTZipBridge_Archive.h/.c`, `ttzip_tar_native.c`, `ttzip_7z_block_decoder.c`, `ttzip_lzma2_enc_native.c`, `CTTZipBridge_GzParallel.c`）的深度代码审计与工业级重构方案提炼而成。

---

## 一、 研究项清单 (Research Index)

- **R001 [SUBAGENT:research] 《TTZip C 桥接层全量代码规范、Arena 内存所有权、64 位 Clamp 与并发死锁防御深度审计》**

---

## 二、 深度研究报告 (Detailed Findings)

### R001: TTZip C 桥接层全量代码规范、Arena 内存所有权、64 位 Clamp 与并发死锁防御深度审计

- **Decision**:
  全面重构 `Sources/CTTZipBridge/` 下的核心 C 头文件与源文件：
  1. **HeaderDoc / Doxygen 契约化与所有权显式标定**：
     重构 `CTTZipCommon.h`, `CTTZipSysAlloc.h`, `CTTZipIO.h`, `CTTZipBridge_Archive.h`, `ttzip_7z_block_decoder.h` 等头文件，按四维标准（`@brief`, `@param [in]/[out]`, `@note [Ownership]`, `@return`）补齐完整自解释文档与 6 级错误码规范；
  2. **内存管理与 Arena 释放安全性加固**：
     在 `ttzip_lzma2_enc_native.c` 中建立结构化释放机制：仅当 `pack_arena == NULL` 时才释放分散的 `pack_buf`，彻底修复错误分支对 Arena 内部指针（Interior Pointer）非法调用 `free()` 导致的堆破坏崩溃；并在 `CTTZipIO.c` 中修复 `payload_buf` 释放遗漏；
  3. **64 位向 `size_t` 转换与 I/O 写入 Clamp 硬防护**：
     在 `CTTZipCommon.h` 中强化 `ttzip_clamp_size()` 与 `ttzip_clamp_ssize()`，替换所有裸强转；对所有 `write`/`writev`/`pread` 施加 `SSIZE_MAX` 分块与 `IOV_MAX`（1024 槽位）切片保护；
  4. **并发死锁消除与原子错误传播**：
     在 `CTTZipBridge_GzParallel.c` 失败分支中确保设置 `is_ready` 并触发 `pthread_cond_broadcast`，彻底消除条件变量死锁；在 `ttzip_7z_block_decoder.c` 中引入原子错误标志 `_Atomic int decode_error`，杜绝错误吞噬与脏数据输出；
  5. **统一 APFS 预分配实现 (DRY)**：
     收敛散落在 `CTTZipCommon.c`、`CTTZipIO.c`、`CTTZipSysAlloc.c` 中的重复实现，统一由 `CTTZipSysAlloc.c` 的 `ttzip_core_apfs_preallocate_file` 提供。

- **Rationale**:
  1. **根除未定义行为与堆损坏**：Arena 统一析构彻底消除 glibc/libmalloc 的 Heap Corruption Panic；
  2. **跨架构整型与流式安全**：大文件场景下杜绝大于 2GB/4GB 造成的整型溢出与 `writev` `EINVAL` 错误；
  3. **世界级代码自解释性**：让任何 C 工程师与开源维护者无需猜测内存所有权或并发生命周期。

- **Alternatives Considered**:
  - *方案 A: 仅修改头文件注释，不触碰 C 内部实现代码*:
    - *否决理由*: 治标不治本，C 内部如果缺少 Arena 释放保护与 64-bit clamp，依然存在跨平台运行崩溃隐患；
  - *方案 B: 完全废除 Arena 内存池，强制使用单块 malloc*:
    - *否决理由*: 会在多核高并发编码时产生大量堆碎片与内核态锁争用，破坏 TTZip 的 Zero-Cost Abstraction 与硬性能门禁。

- **Source**:
  - `Sources/CTTZipBridge/include/CTTZipCommon.h`
  - `Sources/CTTZipBridge/include/CTTZipSysAlloc.h`
  - `Sources/CTTZipBridge/include/CTTZipIO.h`
  - `Sources/CTTZipBridge/include/CTTZipBridge_Archive.h`
  - `Sources/CTTZipBridge/CTTZipCommon.c`
  - `Sources/CTTZipBridge/CTTZipSysAlloc.c`
  - `Sources/CTTZipBridge/CTTZipIO.c`
  - `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c`
  - `Sources/CTTZipBridge/CTTZipBridge_GzParallel.c`
  - `Sources/CTTZipBridge/ttzip_7z_block_decoder.c`
