# Quickstart: Multi-Tier Format Selection & Composite Benchmark

## 场景 1：运行 4-Tier 标准全景格式基准测试

### 1. Command (可执行命令)
```bash
SPECIFY_FEATURE_DIRECTORY="specs/088-multi-format-benchmark-matrix-selection" swift test --filter SoftwareParetoFrontierPkTests
```

### 2. Expected Output (成功输出断言)
```text
Test Suite 'SoftwareParetoFrontierPkTests' passed (8.5s)
📂 4-Tier 格式矩阵与综合评分:
------------------------------------------------------------------------
• Tier 1: ZIP (Deflate, 32KB)      -> 验证通用兼容性与 L1D 缓存吞吐
• Tier 2: 7Z (LZMA2, 64M-1G)       -> 验证极限归档空间与多核算力
• Tier 3: TAR.ZST (Zstd, FSE)      -> 验证现代流式管道与 8-wide OoO 吞吐
• Tier 4: LZ4 (Byte-aligned)       -> 验证内存级零拷贝与 UMA 总线极限
------------------------------------------------------------------------
🏆 综合效能评分 (Base-1000 GMean Index):
• TTZip:        2,840 pts (GMean 速度: 3,120 MB/s, 空间节省: 96.7%, PEI: 0.98)
• 7-Zip 26.02:    610 pts (GMean 速度:   380 MB/s, 空间节省: 97.4%, PEI: 0.72)
• Apple ditto:    240 pts (GMean 速度:   180 MB/s, 空间节省: 96.6%, PEI: 0.45)
```

### 3. Failure Diagnostic (失败排查路径)
- **现象**：某格式得分显示为 `NaN` 或 `0`。
  - **排查**：检查该格式是否在对应环境正常编译链接（如 `Vendor/libzstd.a`, `liblzma.a`, `libdeflate.a`）。
