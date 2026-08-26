# Data Model: C-Bridge Integrity & Algorithm Tiering

**Feature**: `specs/100-zip-genuine-libdeflate-dag-and-audit`

## 1. C-Bridge Compressor Configuration (`CBridgeCompressorSpec`)
- `codecType`: String (enum: `libdeflate`, `liblzma`, `zstd`, `lz4`, `bzip2`, `snappy`)
- `requestedLevel`: Int (1..22)
- `effectiveLevel`: Int (clamped strictly within format's valid hardware domain without artificial degradation)
- `matchAlgorithm`: String (enum: `greedy_neon`, `lazy_rfc1951`, `dag_shortest_path`, `iterative_zopfli`, `dynamic_splitting`)

## 2. ZIP64 Central Directory Header (`Zip64HeaderSpec`)
- `totalEntries`: UInt64 (0 .. 18446744073709551615)
- `centralDirectoryOffset`: UInt64
- `centralDirectorySize`: UInt64
