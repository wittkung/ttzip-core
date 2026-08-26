# Research Report: 047-libarchive-elegance-and-decoupling

本研究报告基于对 `Vendor/libarchive-upstream/libarchive/` 原生头文件与核心实现（`archive.h`, `archive_entry.h`, `archive_read.c`, `archive_write.c`, `archive_read_private.h`, `archive_write_private.h`）的深入源码级剖析提炼而成。

---

## 一、 研究项清单 (Research Index)

- **R001 [SUBAGENT:research] 《libarchive 工业级代码注释契约与容器-滤镜正交解耦架构研究》**

---

## 二、 深度研究报告 (Detailed Findings)

### R001: libarchive 工业级代码注释契约与容器-滤镜正交解耦架构研究

- **Decision**:
  1. **C 桥接层与 Swift PAL 头文件注释全面对标 libarchive 黄金标准**：
     在 `Sources/CTTZipBridge/include/` 与 `Sources/TTZipCore/Platform/` 中建立统一的四维自解释契约注释：
     - `@brief`: 操作意图与状态机转换角色；
     - `@note [Ownership]`: 显式标定内存所有权归属（Borrowing 借用、Transfer 转移、Callee-Owned 内部缓存）与释放责任方；
     - `@param [in]/[out]/[in,out]`: 参数确界、生命周期、是否可空与 64-bit 溢出保护；
     - `@return`: 严格映射 6 级错误码体系 (`EOF`, `OK`, `RETRY`, `WARN`, `FAILED`, `FATAL`)；
     - `@thread_safety`: 线程安全性与并发保证。
  2. **容器格式（ContainerFormat）与流式压缩滤镜（StreamFilter）完全正交解耦**：
     - 拆分平铺复合格式，建立 `ContainerFormat`（Zip, 7z, Tar, Cpio, ISO, WIM）与 `StreamFilter`（None, Gzip, Bzip2, Xz, Zstd, Lz4, Brotli, Lzip, Lrzip）正交组合体系；
     - 面向 Peek/Consume 单向流式缓冲区（Reblocking Sliding Buffer）构建统一管道，杜绝 $M \times N$ 组合爆炸；
     - 显式保留高吞吐 Fast-Path 旁路（如 Zip 并行直通、Tar-Zstd 极速直通），确保硬性能门禁零倒退。
  3. **精细化错误恢复状态机 (Data Recovery State Machine)**：
     - 引入 `TTZipStatus.failed (-25)` 与 `dataRecovery` 状态，单个文件损坏时跳过 payload 并记录警告，自动推进至后续条目恢复解压。

- **Rationale**:
  1. **根除内存泄漏与野指针歧义**：C 与 Swift 互操作边界上清晰标注所有权与释放责任方，消除开发者的猜测与假设；
  2. **消除代码耦合与维护负担**：将格式结构编解码与压缩算法彻底解耦，后续增加新格式无需侵入压缩层；
  3. **兼顾优雅架构与极限性能**：在通用正交管道基础上保留已高度调优的 SIMD/APFS Fast-Path 旁路，实现零成本抽象。

- **Alternatives Considered**:
  - *方案 A: 在 C 桥接层与核心引擎中全面统一采用通用 Composite / Visitor 动态树包装一切数据流*:
    - *否决理由*: 严重违反 GEMINI.md 性能铁律（热路径零成本抽象）。在 10GB+/s 热路径中引入堆分配与动态多态会导致吞吐量大幅暴跌；
  - *方案 B: 仅编写外部 Markdown 文档而不更新源码内部头文件注释*:
    - *否决理由*: 极易发生文档漂移（Documentation Drift），头文件与内联注释是唯一直面开发者的第一信息源。

- **Source**:
  - `Vendor/libarchive-upstream/libarchive/archive.h` (L223-L245, L413-L606, L771-L850)
  - `Vendor/libarchive-upstream/libarchive/archive_entry.h` (L187-L300)
  - `Vendor/libarchive-upstream/libarchive/archive_read.c` (L78-L90, L575-L627, L742-L791, L1330-L1480)
  - `Vendor/libarchive-upstream/libarchive/archive_write.c` (L80-L320)
  - `Sources/CTTZipBridge/include/CTTZipBridge.h`
  - `Sources/TTZipCore/ArchiveCompressionTypes.swift`
