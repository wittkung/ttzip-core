# Feature Specification: 工业级极端边界、安全漏洞与元数据测试体系 (Feature 163)

**Feature ID**: `163-industrial-testing-and-fuzzing-suite`  
**Created**: 2026-08-20  
**Status**: In Progress (Specification Phase)  
**Priority**: P0 (Security, Robustness, System-level Metadata, Fuzzing)

---

## 1. Executive Summary

TTZip 已经建立了 Apple Silicon 架构下的 5 重物理跑分闸门与 CPB 微架构测试体系（4.6 秒全绿通过）。本特性全面对标 `zlib-ng`（微观指令与漏洞回归极致）与 `libarchive`（全格式容错、系统元数据与 Fuzzing 极致），将两大开源基石库数十年的安全防护与历史兼容测试资产完整吸收进 TTZip 原生 C11 测试体系中：
1. **历史 CVE 恶意/畸变包防御回归库**：吸收 `CVE-2002-0059`, `CVE-2005-1849`, `CVE-2018-25032`, `GH-382` 等恶意样本，确保解码器 100% 优雅拦截，0 崩溃、0 内存溢出、0 假死；
2. **跨年代远古归档兼容性验证库**：吸收 `libarchive` 历史兼容测试集（包含 1990 年代 `pkzip 2.04`、早期 `7-Zip 4.20`、旧式 GNU Tar、非标准 EOCD 偏置包），确保面对老旧归档 100% 容错解压；
3. **macOS 系统级属性与 APFS 极端文件系统特性往返**：建立针对 `com.apple.quarantine`、自定义 `xattr`、Finder 标签、APFS 稀疏空洞文件（1GB 逻辑/4KB 物理）的无损打包解包测试；
4. **Clang 原生 LibFuzzer 与语法变异字典**：实现 `LLVMFuzzerTestOneInput` 入口与 CMake `ttzip_fuzzer` 目标，实现自动化比特流变异模糊测试。

---

## 2. User Scenarios

### User Scenario 1 (US1) - 恶意与畸变压缩包安全拦截 (CVE & Malformed Archive Defense)
- **As a**: TTZip 用户与系统级调用方
- **I want to**: 当应用程序尝试解压包含恶意构造的畸变头部、超出边界的匹配偏移或损坏的 Deflate/ZIP/7Z 流时
- **So that**: 解码引擎能够安全、毫秒级返回错误码（如 `TTZIP_ERR_CORRUPT`），绝不发生 SIGSEGV、堆破坏、整型溢出或 CPU 死循环。

### User Scenario 2 (US2) - 跨年代老旧与非标准格式归档容错解压 (Ancient Archive Compatibility)
- **As a**: 处理历史遗留数据的专业用户
- **I want to**: 解压由 1990~2000 年代老旧工具（PKZIP 2.04g, 7-Zip 4.20, 旧式 BSD/GNU Tar）创建的非标准归档
- **So that**: TTZip 能够自适应跳过非标准偏置，正确还原全部文件，实现 100% 数据一致性。

### User Scenario 3 (US3) - macOS APFS 扩展属性与稀疏空洞文件完整往返 (System Metadata & Sparse Files)
- **As a**: macOS 本地开发者与高级用户
- **I want to**: 对包含自定义 `xattr`、隔离属性（`com.apple.quarantine`）及 1GB APFS 稀疏空洞文件的工程目录进行打包与解包
- **So that**: 所有系统元数据无损还原，稀疏空洞文件在打包时不产生物理内存暴涨，解包后保持稀疏特性。

### User Scenario 4 (US4) - 持续模糊测试与变异防御 (Continuous Fuzzing & Mutation Testing)
- **As a**: 核心库维护者
- **I want to**: 借助 Clang LibFuzzer 和 ASan/UBSan 对解压和解析引擎进行数百万次随机比特变异测试
- **So that**: 在代码演进过程中随时发现任何隐蔽的边界越界与未定义行为。

---

## 3. Functional Requirements

- **REQ-001 (CVE Regression Fixtures)**: 在 `tests/fixtures/cve/` 中物理建立历史 CVE 与畸变测试样本（涵盖畸变 Gzip 头部、负距离回溯、畸变 Huffman 树、超界写偏移）。
- **REQ-002 (CVE Defense Test Suite)**: 实现 `tests/c/test_cve_regressions.c`，使用原生 C 驱动对所有样本执行解码测试，断言 100% 优雅返回错误且进程不崩溃。
- **REQ-003 (Compatibility Fixtures)**: 在 `tests/fixtures/compat/` 中提取并建立老旧与非标准格式测试包（PKZIP, 7z legacy, GNU Tar）。
- **REQ-004 (Ancient Format Test Suite)**: 实现 `tests/c/test_compat_archives.c`，验证对历史归档的容错解压与哈希校验。
- **REQ-005 (APFS Metadata & Sparse Suite)**: 实现 `tests/c/test_fs_metadata.c`，测试 macOS `xattr`、隔离属性、符号链接环及 APFS 稀疏文件的打包/解包往返。
- **REQ-006 (LibFuzzer Harness & Dictionary)**: 创建 `tests/fuzz/fuzz_extract_engine.c` 和 `tests/fuzz/ttzip_archive.dict`，实现 LLVM LibFuzzer 驱动。
- **REQ-007 (CMake Fuzzer & Test Integration)**: 更新 `CMakeLists.txt`，将 `test_cve_regressions`、`test_compat_archives`、`test_fs_metadata` 注册至 `ttzip_c_test_runner`，并提供可选的 `ttzip_fuzzer` 目标。
- **REQ-008 (Optimization Gate Integration)**: 更新 `scripts/run_optimization_gate.sh`，将新增安全与元数据测试集纳入 Gate 1 门禁，确保整体验收在 5 秒内完成。

---

## 4. Success Criteria

1. **CVE 漏洞防护率**: 100% 拦截全部恶意样本，0 崩溃、0 ASan/UBSan 警报、0 死循环；
2. **历史归档兼容率**: 100% 正确提取全部 `tests/fixtures/compat/` 归档并完成哈希对齐；
3. **macOS 元数据保留率**: `xattr` 属性与 APFS 稀疏空洞文件 100% 无损往返；
4. **门禁时效性**: 包含新增的全部 C 安全测试后，`./scripts/run_optimization_gate.sh` 依然在 5 秒内通过，全绿零回退。
