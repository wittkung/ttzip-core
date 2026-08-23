# Data Model: ZIP Extreme Speed Multi-Core Block-Parallel Mode

## 1. `ZipExtremeBlockChunk`
- `index`: `Int` (分块序号，0-based)
- `isFinal`: `Bool` (是否为末尾分块)
- `uncompressedSize`: `Int` (原始未压缩分块字节数)
- `compressedData`: `Data` (压缩后且注入 RFC 1951 对齐标记的字节流)
- `chunkCrc32`: `UInt32` (分块 CRC-32 校验和)

## 2. `ZipExtremeCompressionOptions`
- `blockSize`: `Int` (默认 1024 * 1024 字节 / 1MB)
- `level`: `ArchiveCompressionLevel` (压缩档位，1..12)
- `maxInFlightSlots`: `Int` (最大在途环形队列插槽数，默认 32)
