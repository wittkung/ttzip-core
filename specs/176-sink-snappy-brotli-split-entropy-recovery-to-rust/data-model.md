# Data Model: 176-sink-snappy-brotli-split-entropy-recovery-to-rust

## 1. Multi-Volume Models
- **`VolumeNamingScheme`**:
  - `NumberedExtension`: `.7z.001`, `.zip.001`, `.tar.001`
  - `PkzipSpanned`: `.z01`, `.z02` ... `.zip`
  - `PartNumbered`: `.part1.rar`, `.part01.rar`, `.part1.tar.gz`
  - `RawSplit`: `.001`, `.002`

## 2. Analytics & Entropy Models
- **`EntropyProbeResult`**:
  - `shannon_entropy: f64`
  - `is_high_entropy: bool`
  - `is_compressible: bool`
  - `sample_bytes: usize`
  - `probe_duration_nanos: u64`
- **`CodecRecommendation`**:
  - `recommended_format: u32`
  - `recommended_level: i32`
  - `estimated_ratio: f64`
  - `reason: String`

## 3. VFS Cache Pool Models
- **`ChunkKey`**:
  - `session_id: u64`
  - `chunk_index: u64`
- **`LruNode`**:
  - `key: ChunkKey`
  - `prev: u32`
  - `next: u32`
  - `raw_size: u32`
  - `compressed_size: u32`
  - `is_spilled: bool`

## 4. Password Recovery Models
- **`EncryptionProbeTarget`**:
  - `ZipCrypto`: `enc_header: [u8; 12]`, `crc32_check_byte: u8`, `preview_ciphertext: [u8; 32]`
  - `WinZipAes256`: `salt: [u8; 16]`, `stored_pvv: [u8; 2]`, `ciphertext_preview: [u8; 32]`, `stored_mac: [u8; 10]`
  - `SevenZipAes`: `salt: Vec<u8>`, `num_cycles_power: u32`, `iv: [u8; 16]`, `encrypted_header_block: [u8; 32]`

## 5. Salvage & Repair Models
- **`SalvageReport`**:
  - `total_scanned_bytes: u64`
  - `recovered_entries_count: u32`
  - `corrupted_bytes_skipped: u64`
  - `header_rebuilt: bool`
  - `integrity_verified: bool`
