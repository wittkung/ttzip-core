# Data Model: 183-archive-dispatch-decoupling-and-protocol-modularization

## 1. Engine Bridge Models
- **`EngineBridgeContext`**:
  - `format: ArchiveCompressionFormat`
  - `options: ArchiveOptions`
  - `progressHandler: ArchiveProgressHandler?`

## 2. Dispatch Target Routes
- **`ZipEngineRoute`**
- **`SevenZipEngineRoute`**
- **`TarEngineRoute`**
- **`RawStreamEngineRoute`**
