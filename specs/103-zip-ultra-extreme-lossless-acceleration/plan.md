# Implementation Plan: Feature 103 (ZIP Tier 6/7 Lossless Acceleration)

## 1. Technical Context
- **语言与平台**：Swift 6.0 + C11 / ARM64 NEON，macOS 14+ (Sonoma)。
- **核心目标**：在压缩率 0 损失（Bit-Exact）的前提下，实现 L6/L7 的 2MB Tile 分块、不动点无损早退与 SIMD 加速。

## 2. Constitution & Rules Check
- [x] **性能铁律**：热路径零堆分配、零中间内存拷贝。
- [x] **零损失确界**：32KB 跨块历史字典无缝衔接，压缩后体积严禁任何膨胀。

## 3. Phase 0: Research Items
- - R001 [SUBAGENT:research] 《L2 缓存拓扑感知分块与滑动历史字典连续性研究》：已完成，见 [research.md](file:///Users/kevintung/Documents/dev/TTZip/specs/103-zip-ultra-extreme-lossless-acceleration/research.md)。
- - R002 [SUBAGENT:research] 《不动点决策向量严格无损自适应早退与 SIMD 代价计算研究》：已完成，见 [research.md](file:///Users/kevintung/Documents/dev/TTZip/specs/103-zip-ultra-extreme-lossless-acceleration/research.md)。

## 4. Phase 1: Design Artifacts
- **Data Model**: [data-model.md](file:///Users/kevintung/Documents/dev/TTZip/specs/103-zip-ultra-extreme-lossless-acceleration/data-model.md)
- **Contracts**: [contracts/zip-extreme-lossless.schema.json](file:///Users/kevintung/Documents/dev/TTZip/specs/103-zip-ultra-extreme-lossless-acceleration/contracts/zip-extreme-lossless.schema.json)
- **Quickstart**: [quickstart.md](file:///Users/kevintung/Documents/dev/TTZip/specs/103-zip-ultra-extreme-lossless-acceleration/quickstart.md)

## 5. Component Modification List
1. **[MODIFY] `Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift`**:
   - 优化分块自适应算法：针对 Tier 5/6/7 使用 2MB Tile 分块，将工作集压至 L2 缓存容量以内。
2. **[MODIFY] `Sources/CTTZipBridge/ttzip_zopfli_engine.c`**:
   - 支持不动点自适应早退与分块历史字典高速衔接。
3. **[MODIFY] `Tests/TTZipTests/ZipMultiCoreParetoFrontierPkTests.swift`**:
   - 验证加速后 L6/L7 吞吐提升，更新图表工件。
