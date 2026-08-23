# Phase 0 Research: Dedicated Per-Format Benchmark Charts & Apple Native Expansion

## R001: macOS 系统自带归档工具链 (ditto, zip, tar, gzip, bzip2) 行为特征与测试接口调研

### 1. Decision (选定方案)
- **Apple Native 评测工具链扩充**：
  1. `/usr/bin/ditto -c -k --sequesterRsrc <src> <dst.zip>`：作为 macOS Archive Utility 与 Finder 默认系统级黄金对照标准（保留 xattr 与 `__MACOSX` 影子元数据）。
  2. `/usr/bin/zip -1 -r <dst.zip> <src>`：作为单线程 Info-ZIP 极速基准（关闭 Lazy Matching）。
  3. `/usr/bin/zip -6 -r <dst.zip> <src>`：作为单线程 Info-ZIP 默认标准基准（标准 Deflate）。
  4. `/usr/bin/tar -czf <dst.tar.gz> <src>`：作为 BSD tar + libz 流式 GZIP 基线。
- **单格式专属出图架构 (Dedicated Per-Format Chart Matrix)**：
  - 分别输出：
    - `pareto_pk_zip.png`（**ZIP 专场**：TTZip vs. 7-Zip vs. Apple ditto vs. Apple zip-1 vs. Apple zip-6）
    - `pareto_pk_7z.png`（**7Z 专场**：TTZip vs. 7-Zip 26.02 ARM64 Fast, Normal, Ultra）
    - `pareto_pk_tar_zst.png`（**TAR.ZST 现代流式专场**：TTZip Direct vs. 官方 zstd / 7-Zip）
    - `pareto_pk_lz4.png`（**LZ4 极速专场**：TTZip Direct In-Memory）
    - `software_pareto_pk.png`（4-Tier 全景图）

### 2. Rationale (选择理由)
1. **彻底解决 Apple 只有一个点的缺陷**：扩充后 Apple Native 拥有完整的 3 个点（ditto, zip -1, zip -6），能够连成 Apple 家族专属的性能轨迹线。
2. **聚焦单一格式真实对决**：在单一格式专场图表中，X 轴与 Y 轴视口完全针对该格式的最佳压缩率与吞吐动态展开，呼吸空间充裕，杜绝不同格式点位相互混淆。

### 3. Alternatives Considered (已否决方案)
- **仅使用 Info-ZIP**：丢失 macOS extended attributes 和 resource fork，无法反映 Finder 真实体验。
- **所有格式强制挤在一张图**：由于格式间压缩率跨度大（如 LZ4 90% vs 7Z 100%），导致同格式细分档位紧贴在一起。

### 4. Source (实际查阅资料)
- `man ditto`, `man zip`, `man tar` on macOS Sonoma 14+.
- `Sources/TTZipCore/Benchmark/CompetitorBenchmarkRunner+Executors.swift`.
- `Tests/TTZipTests/SoftwareParetoFrontierPkTests.swift`.
