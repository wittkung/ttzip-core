# Quickstart: 094 Entropy-Aware Tiered Chunking Engine

## Scenario 1: 验证分级自适应分块实测

### Command
```bash
swift test --filter EntropyTieredChunkingEngineTests
```

### Expected Output
- 验证低熵文件采用 2MB 块（压缩比提升 $\ge 15\%$）；
- 验证中熵文件采用 512KB 块（维持 5.5+ GB/s 吞吐）；
- 验证中高熵文件采用 128KB 块；
- 验证高熵文件采用 0 (Method 0 Store) 模式；
- 所有归档通过 `/usr/bin/unzip -t` 校验。
