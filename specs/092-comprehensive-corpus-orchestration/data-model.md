# Data Model: Comprehensive Corpus Orchestration & Geometric Mean Benchmark Matrix

## 1. `BenchmarkTierCategory`
- `tier1Text`: Large Text & Web (`enwik8`, `webster`, `dickens`, `reymont`) - Weight 0.25
- `tier2Binary`: Binary Executable (`mozilla`, `ooffice`) - Weight 0.20
- `tier3Structured`: Structured Data & DB (`nci`, `osdb`, `xml`) - Weight 0.20
- `tier4SourceTree`: Mixed SourceTree & VFS (`samba`, `MicroCorpus 500f`) - Weight 0.20
- `tier5DenseMatrix`: Scientific & Dense Matrix (`mr`, `x-ray`, `sao`) - Weight 0.15

## 2. `CorpusPayloadDescriptor`
- `id`: `String` (如 "silesia_dickens", "enwik8_100mb", "vfs_tree_500f")
- `name`: `String`
- `tier`: `BenchmarkTierCategory`
- `fileCount`: `Int`
- `totalBytes`: `Int64`
- `isDirectoryTree`: `Bool`
- `sourcePath`: `String`

## 3. `AlgorithmCompositeScore`
- `id`: `String`
- `algorithm`: `String`
- `level`: `Int`
- `geomCompSpeedMBs`: `Double`
- `geomDecompSpeedMBs`: `Double`
- `geomCompressionRatio`: `Double`
- `geomSpaceSavingsPct`: `Double`
- `compositeEfficiencyIndex`: `Double`
- `normalizedSpecScore`: `Double`
- `isParetoOptimal`: `Bool`
- `tierBreakdown`: `[BenchmarkTierResult]`
