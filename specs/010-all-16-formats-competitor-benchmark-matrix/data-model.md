# Data Model: 16-Format Competitor Benchmark

## Benchmark Matrix Structure
```
CompetitorBenchmarkMatrix
  ├── Payload Dimensions (4)
  │    ├── 10MB 拟真日志文本 (Text/Log)
  │    ├── 10MB 海量小文件 (100 Files)
  │    ├── 100MB 高熵物理载荷 (High Entropy / Multimedia)
  │    └── 500MB 大文件数据块 (500MB Single Large File)
  │
  ├── Formats (16)
  │    ├── Core: ZIP, 7Z, TAR.GZ, TAR.ZST
  │    ├── Unix Extended: TAR.BZ2, TAR.XZ, TAR, LZIP
  │    ├── Fast Modern: LZ4, BROTLI, LRZIP, SNAPPY
  │    └── Systems & Images: AAR, WIM, DMG, ISO
  │
  └── Levels & Modes
       ├── Level 1 (Fast / Fastest)
       ├── Level 6 (Default / High)
       └── Encryption: None / AES-256 (for supported formats)
```
