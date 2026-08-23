# Data Model: 148-frontend-c-wiring-and-swift-slimming

## Entity: `DiskItemInfo` (Swift Frontend)
- `name`: `String`
- `path`: `String`
- `isDirectory`: `Bool`
- `rawSizeBytes`: `Int64`
- `modificationDate`: `Date?`
- `sortKey`: `String` (evaluated via `NativeMicrokernelBridge.naturalCompare`)

## Entity: `MediaPreviewPayload`
- `data`: `Data`
- `mimeType`: `String`
- `kind`: `ttzip_file_kind_t`
- `isMemoryBacked`: `Bool` (true for 0-disk-IO streams)
