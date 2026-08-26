# Feature Specification: 032-libarchive-hardware-crc32-acceleration

**Feature Name**: libarchive `archive_crc32.h` ARMv8 ACLE 与通用硬件加速改造 (032-libarchive-hardware-crc32-acceleration)  
**Status**: SPECIFIED  
**Priority**: P1 (Core Infrastructure & Peak Performance)  
**Target Module**: `Vendor/libarchive-upstream/libarchive/archive_crc32.h`, `Vendor/libarchive-upstream/libarchive/test/test_archive_string_conversion.c` (or dedicated CRC32 test)  

---

## 1. Background & Executive Summary

在 `Vendor/libarchive-upstream/libarchive/archive_crc32.h` 中，现有实现基于 2009 年编写的单字节 256 元素静态表查表算法（`crc_tbl[256]`）。由于循环内存在严格的数据依赖关系（Serial Data Dependency, ILP=1），每字节处理消耗 4~5 个 CPU 时钟周期，单核吞吐被锁死在约 **720 MB/s**。

本特性的目标是：
1. **引入 ARMv8 ACLE 硬件加速**：使用 `<arm_acle.h>` 中的 `__crc32b` 与 `__crc32d` 指令，配合 8 路展开（单次处理 64 字节）充分饱和超标量流水线，将单核 CRC32 吞吐提升至 **14+ ~ 16+ GB/s**（提升 20x+）。
2. **纯 C99 Slicing-by-8 / 经典查表兜底**：在无硬件 CRC 指令的平台保留 100% 兼容的通用纯 C 回退路径。
3. **保持 100% API/ABI 与 IEEE 802.3 多项式一致性**：确保 `archive_crc32.h` 计算结果与既有实现、标准 `zlib` 的 `crc32()` 逐字节指纹完全一致，零外部依赖，作为 drop-in 替换头文件。
4. **编译验证与全套单测回归**：在 `Vendor/libarchive-upstream` 下编译 `libarchive_test` 并通过全量测试；在 TTZip 中完成构建并确保全矩阵性能门禁零倒退。

---

## 2. Clarifications & Architectural Decisions

### Session 2026-08-15
- **Q1: 为什么不能直接在 x86 上使用 SSE4.2 `_mm_crc32_u64` 指令？**  
  **A1**: Intel SSE4.2 硬件指令硬编码了 Castagnoli 多项式（CRC-32C, `0x1EDC6F41`），而 ZIP / 7z / libarchive / PNG 标准均使用 IEEE 802.3 多项式（`0xEDB88320`）。因此在 x86 上若无 PCLMULQDQ 支持，必须使用通用 Slicing-by-8 或标准查表，严禁混淆多项式导致校验和损坏。
- **Q2: ARMv8 ACLE 指令是否与 IEEE 802.3 完全一致？**  
  **A2**: 100% 一致。ARMv8-A 规范明确定义 `__crc32b`/`__crc32w`/`__crc32d` 使用多项式 `0x04C11DB7`（反向表示 `0xEDB88320`），与 zlib / libarchive 的 CRC-32 逐位匹配。
- **Q3: 为什么要在主循环前进行内存指针 8 字节对齐？**  
  **A3**: 尽管现代 ARM64 允许非对齐内存加载，但在 64 字节（8 组 64-bit 字）高速循环中，保证 8 字节自然对齐可消除跨 Cache Line（64 字节）访问带来的额外周期惩罚，使 `__crc32d` 达到理论最高吞吐。

---

## 3. User Scenarios & Acceptance Criteria

### User Scenario 1 (US1): Apple Silicon / ARM64 硬件指令极速计算
- **场景**：在 Apple Silicon (macOS) 或 ARM64 Linux 平台上解压/校验大型 ZIP/7z/RAR 归档。
- **行为**：`crc32()` 自动进入 `LIBARCHIVE_HAS_ARM_CRC32` 路径，使用 `__crc32d` 8 路展开，单核处理吞吐达到 $\ge 12,000\text{ MB/s}$。

### User Scenario 2 (US2): 无硬件扩展平台的无缝兼容回退
- **场景**：在老旧 CPU、RISC-V 或未开启 CRC 扩展的编译器环境下构建。
- **行为**：预处理器条件宏自动降级至通用纯 C 回退分支，编译零警告、零报错，校验和输出 100% 正确。

### User Scenario 3 (US3): 上游 libarchive 测试套件与 TTZip 回归全绿
- **场景**：运行 `libarchive_test` 与 TTZip 全量测试。
- **行为**：所有校验和相关测试 100% 通过，无任何性能倒退。

---

## 4. Functional Requirements & Technical Boundaries

- **FR-01**: `Vendor/libarchive-upstream/libarchive/archive_crc32.h` 必须提供符合 `unsigned long crc32(unsigned long crc, const void *p, size_t len)` 签名的内联函数。
- **FR-02**: 必须通过 `#if defined(__ARM_FEATURE_CRC32) || (defined(__APPLE__) && defined(__MACH__) && (defined(__aarch64__) || defined(_M_ARM64)))` 自动检测并启用 ARM ACLE 硬件加速。
- **FR-03**: 必须包含 8 字节前置对齐、64 字节（8 路 `__crc32d`）超标量主循环、剩余 8 字节处理及尾部单字节处理。
- **FR-04**: 必须提供完整的纯 C 静态查表 fallback，严禁引入任何裸外部符号或未定义的第三方库依赖。
- **FR-05**: 针对空指针 `_p == NULL` 或 `len == 0`，必须保持原有的安全边界检查与返回值语义。
- **FR-06**: `Vendor/libarchive-upstream` 必须在 CMake 构建下成功编译 `libarchive` 与 `libarchive_test`，且全部 7z/zip 测试通过。

---

## 5. Success Criteria

1. **功能正确性**：Upstream 测试套件与 TTZip 测试套件 100% 绿灯。
2. **性能基准**：在 Apple Silicon 物理机上，单核 CRC32 纯吞吐测试达到 $\ge 12\text{ GB/s}$（相对原 256 表提升 $\ge 15\text{x}$）。
3. **零破坏性**：保持 100% C99 纯头文件内联形态，零 API/ABI 变更。
