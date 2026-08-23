# Research Findings: 148-frontend-c-wiring-and-swift-slimming

## R001: C11 Natural Numeric Sort Integration in SwiftUI List
- **Decision**: Update `DiskItemSorter.isOrderedBefore` to call `NativeMicrokernelBridge.naturalCompare(a.name, b.name)`.
- **Rationale**: `localizedStandardCompare` in Foundation performs heavy ICU regex matching and heap allocations. C11 `ttzip_strnatcasecmp` achieves 32.18 Million ops/s with zero heap allocation.
- **Alternatives Considered**: Swift native string comparison (causes `file10.txt` to sort before `file2.txt`, breaking natural user ordering).
- **Source**: `ttzip_strnatcmp.c`, `DiskItemSorter.swift`.

## R002: 0-Disk-IO Instant Preview Stream Factory
- **Decision**: Update `MediaPreviewFactory` to expose `detectTypeAndPayload(data: Data)` using `NativeMicrokernelBridge.sniffMagic` and `NSImage(data:)` / `AVAsset` in-memory streams.
- **Rationale**: Eliminates creating temporary files in `/tmp` for image/document preview, preventing disk wear and eliminating zombie temporary file leaks.
- **Alternatives Considered**: `/tmp/ttzip_preview_XXXXXX` disk cache (causes disk I/O latency and requires manual garbage collection).
- **Source**: `MediaPreviewFactory.swift`, `ttzip_magic_sniff.c`.
