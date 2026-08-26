# Data Model: 194-purge-dead-services-legacy-vendor-artifacts-and-cmake-relics

## 1. Streamlined Services Layer
- `Sources/TTZipCore/Services/`:
  - `ArchivePasswordStore.swift` (Password cache)
  - `ByteCountFormatterCache.swift` (Cached formatting)
  - `DateFormatterCache.swift` (Cached ISO/POSIX date formatting)
  - `DeepFileMetadataReader.swift` (File inspection)
  - `FinderSyncActionRequest.swift` (Finder Sync requests)

## 2. Streamlined Utilities Layer
- `Sources/TTZipCore/Utilities/`:
  - `Logger.swift` (Unified logging)
  - `SevenZipBinaryResolver.swift` (Binary fallback detection)
  - `SubprocessExecutor.swift` (Secure posix_spawn subprocess executor)
  - `TempDirectoryCleanUpManager.swift` (Atomic temp dir lifecycle)
