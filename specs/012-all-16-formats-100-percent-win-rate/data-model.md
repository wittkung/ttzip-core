# Data Model: 16-Format Dominance Matrix

```
BenchmarkRecord
  ├── format: String (zip, 7z, tar.gz, tar.zst, tar.bz2, tar.xz, tar, lzip, lz4, brotli, lrzip, aar, snappy, wim, dmg, iso)
  ├── dimensionName: String (10MB Log, 10MB 100 Files, 100MB High Entropy, 500MB Large File)
  ├── level: Int (1, 6)
  ├── isEncrypted: Bool
  ├── ttzipCompressMBs: Double
  ├── compressThroughputMBs: Double (Competitor)
  ├── compressSpeedupVsCompetitor: Double (>= 1.00)
  ├── ttzipExtractMBs: Double
  ├── extractThroughputMBs: Double (Competitor)
  └── extractSpeedupVsCompetitor: Double (>= 1.00)
```
