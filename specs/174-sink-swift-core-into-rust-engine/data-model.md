# Data Model: 174-sink-swift-core-into-rust-engine

## 1. Rust Standards & Sniffing Models
- **`SignatureAnchor`**:
  - `Head { offset: u64 }`
  - `Tail { offset_from_eof: u64 }`
  - `Sector { sector_index: u32, byte_offset: u32 }`
  - `TarOffset { byte_offset: u64 }`
- **`ComplianceReport`**:
  - `format: TTZipArchiveFormat`
  - `is_compliant: bool`
  - `citation: Option<StandardCitation>`
  - `validated_headers: Vec<String>`
  - `warnings: Vec<String>`
  - `violations: Vec<String>`

## 2. Rust Zero-Copy Archive Models
- **`ZipEntryRef<'a>`**:
  - `raw_path: &'a [u8]`
  - `uncompressed_size: u64`
  - `compressed_size: u64`
  - `crc32: u32`
  - `compression_method: u16`
  - `lfh_offset: u64`
  - `extra_fields: ZipExtraFieldsRef<'a>`
- **`TarHeader`**:
  - `name: [u8; 100]`
  - `mode: [u8; 8]`
  - `size: [u8; 12]`
  - `chksum: [u8; 8]`
  - `typeflag: u8`
  - `magic: [u8; 6]`
  - `prefix: [u8; 155]`

## 3. Cryptography & Self-Healing Models
- **`ZipCryptoKeys`** (with `ZeroizeOnDrop`):
  - `key0: u32`
  - `key1: u32`
  - `key2: u32`
- **`RecoveryRecordHeader`**:
  - `magic: [u8; 4]` (`TTZR`)
  - `version: u16`
  - `slice_size: u32`
  - `total_k: u16`
  - `total_m: u16`
  - `protected_payload_length: u64`
  - `root_hash: [u8; 32]`
  - `data_slices_crc: Vec<u32>`
