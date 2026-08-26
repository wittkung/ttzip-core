# Data Model: 180-architecture-streamlining-and-core-headless-purity

## 1. 7z Entry Descriptor Models (`Sources/TTZipCore/SevenZip/`)
- **`SevenZipEntryDescriptor`**:
  - `name: String`
  - `uncompressedSize: UInt64`
  - `compressedSize: UInt64`
  - `crc32: UInt32`
  - `isDirectory: Bool`
  - `mtime: Date`

## 2. Standards Compliance Models (`Sources/TTZipCore/Standards/`)
- **`ArchiveFormatStandardSpec`**:
  - `format: ArchiveCompressionFormat`
  - `isStandardCompliant: Bool`
  - `detectedSignatures: [String]`
  - `violations: [String]`
