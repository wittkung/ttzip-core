# Data Model: 193-purge-dead-c-headers-dead-facades-and-linker-cleanup

## 1. Cleaned CTTZipBridge Directory
- `Sources/CTTZipBridge/include/`:
  - `CTTZipBridge.h` (Platform enums)
  - `ttzip_rust_glue.h` (Pure Rust C-ABI exports)
  - `module.modulemap` (Clang module definition)

## 2. Streamlined TTZipCore Facade Architecture
- `Sources/TTZipCore/Facades/`:
  - `TTZipEngineFacade.swift` (Unified high-level facade)
  - `TTZipEngineFacade+Compress.swift`
  - `TTZipEngineFacade+Extract.swift`
  - `TTZipEngineFacade+Inspect.swift`
  - `TTZipEngineFacading.swift`
  - `ArchiveBatchFacade.swift` (Batch maintenance operations)
  - `ArchiveBatchFacade+Parallel.swift`
  - `ArchiveBatchModels.swift`
