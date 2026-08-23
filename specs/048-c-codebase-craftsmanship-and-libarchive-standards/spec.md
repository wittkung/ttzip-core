# Feature Specification: 048-c-codebase-craftsmanship-and-libarchive-standards

**Feature Name**: `c-codebase-craftsmanship-and-libarchive-standards`  
**Status**: `Draft`  
**Target Milestone**: Comprehensive C Codebase Modernization, Industrial Standards & libarchive Alignment  
**Created**: 2026-08-17  

---

## 一、 业务动机与第一性原理 (Context & First Principles)

TTZip 的底层由 46 个 C 源文件和 64 个头文件构成了 100% 进程内高性能计算基座。然而，现有 C 代码在工程规范、分层清晰度、错误处理与注释质量上与世界顶级开源库 `libarchive` 相比存在明显差距：
1. **全局头文件包含与类型定义碎片化**：缺少统一的上下文结构体与前置确界断言；
2. **内存所有权与生命周期未显式标定**：部分函数分配后未标注释放责任方，存在跨语言借用悬空隐患；
3. **缺少 Doxygen/HeaderDoc 标准自解释注释**：关键位运算、SIMD 展开与状态机缺少“为什么这样做 (Why)”的物理原理解释；
4. **系统调用缺少保护性 Clamp 与溢出防御**：未对 64 位偏移量向 32 位系统调用传递做确界保护。

本 Feature 旨在对 `Sources/CTTZipBridge/` 下的全量 C 代码进行**地毯式规范重构与工业级打磨**，全面对标 `libarchive`：

---

## 二、 用户故事 (User Stories)

### User Story 1 (P1): 全量 C 语言头文件达到 libarchive 黄金文档与契约标准
作为系统开发者与上游审核员，阅读 `Sources/CTTZipBridge/include/` 下的任意头文件时，能清晰看到 `@brief`, `@param [in]/[out]`, `@note [Ownership]`, `@return`, `@thread_safety`，彻底杜绝内存与并发歧义。

- **Acceptance Criteria**:
  - `Sources/CTTZipBridge/include/` 下所有头文件 100% 完成自解释规范重构。
  - 所有返回动态分配内存的函数均标明 `@note [Ownership] Caller must free`.

### User Story 2 (P1): C 底层编解码与工具模块进行防御性确界加固与解耦
作为性能与安全工程师，所有 C 实现文件（LZMA2、Tar、7z、Zstd、APFS、SIMD）均具备清晰的分层结构、防御性断言与 0 堆碎片设计。

- **Acceptance Criteria**:
  - 所有 64 位文件偏移转 `size_t` 均经过 `SSIZE_MAX` Clamp 保护。
  - 所有分配与释放成对遵循 `ttzip_platform_aligned_alloc` / `ttzip_platform_aligned_free`。

### User Story 3 (P2): 保持 100% 编译零警告与吞吐硬门禁
作为发布经理，重构后的 C 代码在 Debug / Release / AddressSanitizer 模式下编译 0 警告，且 100% 保持极致吞吐。

- **Acceptance Criteria**:
  - `swift build -c release` 0 warnings, 0 errors.
  - `./scripts/run_local_ci.sh --quick` 100% 通过。

---

## 三、 功能需求 (Functional Requirements)

1. **FR001**: 必须重构 `CTTZipCommon.h/.c`, `CTTZipIO.h/.c`, `CTTZipSysAlloc.h/.c`, `CTTZipDiagnostics.h/.c`。
2. **FR002**: 必须重构 `CTTZipBridge_Archive.h/.c`, `ttzip_tar_native.c`, `ttzip_native_archive.h/.c`。
3. **FR003**: 必须重构 `ttzip_7z_*.h/.c`, `CTTZipBridge_7z*.h/.c` 系列头文件与实现。
4. **FR004**: 必须重构 `ttzip_lzma2_*.h/.c` 与 `ttzip_bcj_*.h/.c` 算法注释与确界。
5. **FR005**: 必须在全量 CI 中保持 0 错误、0 警告与 100% 测试通过。

---

## 四、 成功指标 (Success Criteria)

- **SC001**: 全量 C 头文件注释覆盖率达 100%。
- **SC002**: 编译保持 0 警告，本地 CI <= 10s 全绿。
