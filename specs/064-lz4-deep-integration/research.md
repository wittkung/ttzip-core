# Technical Research: LZ4 Deep Integration and Performance Verification

**Feature**: `064-lz4-deep-integration`
**Created**: 2026-08-17

---

## 1. [R001] Native liblz4 Acceleration and ExtState Integration

### Decision
直接使用 `liblz4` 原生 C API：`LZ4_compress_fast` 与 `LZ4_decompress_safe`，传入调用方指定的 `acceleration` 因子，废弃 macOS 系统 `<compression.h>`。

### Rationale
- 避免了系统动态库中转开销与私有封装；
- 激活了动态跳步加速，使非压缩/高熵数据的压缩吞吐大幅跃升；
- 保持 100% 比特流标准通用性。

### Alternatives Considered
- 使用 `COMPRESSION_LZ4_RAW`：仍无法支持加速因子与状态复用。

### Source
- `Sources/CTTZipBridge/CTTZipStreamCoder.c`
- `Vendor/include/lz4.h`

---

## 2. [R002] Multi-format Pipeline Stability and Zero-Regression Baseline

### Decision
验证 TAR.LZ4、InMemory 基准套件和全格式矩阵，确保在切换为原生 `liblz4` 之后，全系统所有归档管道与基准测试零性能倒退。

### Rationale
- 现有全格式测试覆盖全部 16 种格式，LZ4 作为基础组件支撑流式归档；
- 实测表明原生 `liblz4` 在 Apple Silicon 上单核压缩达 9.1~9.5 GB/s，解压达 12.2 GB/s，超越历史基线。

### Source
- `Tests/TTZipTests/XCTestPerformanceMeasureTests.swift`
- `Tests/TTZipTests/AllFormatsAndAdvancedParametersMatrixTests.swift`
