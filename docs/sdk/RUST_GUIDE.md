# 🦀 TTZip Rust SDK Developer Guide

[![Crate](https://img.shields.io/badge/crate-ttzip--engine-orange.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/rust/ttzip-engine/Cargo.toml)
[![Documentation](https://img.shields.io/badge/docs-1.0.0-blue.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/docs/sdk/README.md)
[![Safety: Safe Rust Core](https://img.shields.io/badge/Safety-100%25%20Safe%20Rust%20Core-brightgreen.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/ARCHITECTURE.md)

The `ttzip-engine` crate is the native microkernel powering the entire TTZip ecosystem. It provides high-throughput, multi-threaded archive creation, zero-copy streaming extraction, in-place atomic mutations, SIMD-accelerated checksums, and Reed-Solomon error correction.

---

## 1. Installation & Cargo Configuration

Add `ttzip-engine` to your `Cargo.toml`:

```toml
[dependencies]
ttzip-engine = { path = "core/rust/ttzip-engine" }
rayon = "1.10"
tempfile = "3.10"
```

### Feature Flags

| Feature | Description | Default |
| :--- | :--- | :---: |
| `simd` | ARM NEON & x86 AVX2/AVX-512 hardware acceleration | ✅ |
| `zstd-backend` | Multi-threaded Zstandard codec with LDM | ✅ |
| `lzma-backend` | Fast-LZMA2 parallel 7z compression | ✅ |
| `crypto` | AES-256 CTR/CBC and ZipCrypto pipelines | ✅ |
| `uniffi` | Auto-generated multi-language foreign function bindings | ❌ |

---

## 2. Quickstart Examples

### 2.1 Multi-Core ZIP & 7z Compression

Create archives using the streaming parallel engine with APFS preallocation:

```rust
use std::path::PathBuf;
use ttzip_engine::{
    ArchiveBuilder, ArchiveFormat, CompressionLevel, EncryptionMethod,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sources = vec![
        PathBuf::from("docs/report.pdf"),
        PathBuf::from("assets/images/"),
    ];
    let output_archive = PathBuf::from("dist/bundle.zip");

    ArchiveBuilder::new()
        .sources(sources)
        .destination(&output_archive)
        .format(ArchiveFormat::Zip)
        .level(CompressionLevel::Normal) // Level 6
        .threads(0)                      // 0 = Auto-detect CPU topology
        .progress_callback(|processed, total, current_file| {
            let pct = if total > 0 { (processed as f64 / total as f64) * 100.0 } else { 0.0 };
            println!("[{pct:.1}%] Processing: {current_file}");
            true // return false to cancel
        })
        .build()?;

    println!("Archive created successfully at: {:?}", output_archive);
    Ok(())
}
```

### 2.2 Memory-Safe Extraction with Zip-Slip Prevention

Extract any supported archive container (ZIP, 7z, TAR, TAR.GZ, TAR.ZST) safely:

```rust
use std::path::PathBuf;
use ttzip_engine::ExtractBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let archive_path = PathBuf::from("dist/bundle.zip");
    let dest_dir = PathBuf::from("dist/extracted_output");

    ExtractBuilder::new()
        .archive(&archive_path)
        .destination(&dest_dir)
        .password(None)                // Or Some("secret_pass")
        .overwrite(true)
        .preserve_permissions(true)
        .progress_callback(|processed, total, current_entry| {
            println!("Extracted {processed}/{total} bytes: {current_entry}");
            true
        })
        .extract()?;

    println!("Extraction completed to: {:?}", dest_dir);
    Ok(())
}
```

### 2.3 Non-Extracting Archive Inspection & Charset Detection

Inspect file metadata (uncompressed size, CRC32, timestamp, permissions) without extracting payload bytes to disk:

```rust
use std::path::PathBuf;
use ttzip_engine::ArchiveReader;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let archive_path = PathBuf::from("dist/archive_legacy_sjis.zip");
    let reader = ArchiveReader::open(&archive_path, None)?;

    println!("Inspecting archive entries in: {:?}", archive_path);
    for entry in reader.entries() {
        println!(
            "Entry: {} | Size: {} bytes | CRC32: {:08X} | Detected Encoding: {}",
            entry.path,
            entry.uncompressed_size,
            entry.crc32,
            entry.detected_encoding.as_deref().unwrap_or("UTF-8")
        );
    }

    Ok(())
}
```

---

## 3. High-Performance In-Memory Buffer Codecs & SIMD Checksums

`ttzip-engine` exports low-overhead memory codecs operating directly on byte slices (`&[u8]`):

```rust
use ttzip_engine::codecs::{
    deflate_compress, deflate_decompress,
    zstd_compress, zstd_decompress,
    lz4_compress, lz4_decompress,
    snappy_compress, snappy_decompress,
};
use ttzip_engine::platform::{crc32_fast, crc64_fast};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let payload = b"Apple Silicon M-Series PMULL and NEON Vectorized Engine Payload 2026!";

    // 1. Hardware SIMD Checksums (>48 GB/s throughput)
    let crc_val = crc32_fast(0, payload);
    let crc64_val = crc64_fast(0, payload);
    println!("CRC-32: {:08X} | CRC-64: {:016X}", crc_val, crc64_val);

    // 2. High-speed DEFLATE (libdeflate levels 1..12)
    let mut compressed = vec![0u8; payload.len() * 2];
    let compressed_len = deflate_compress(payload, &mut compressed, 6)?;
    compressed.truncate(compressed_len);

    let mut decompressed = vec![0u8; payload.len()];
    let decompressed_len = deflate_decompress(&compressed, &mut decompressed)?;
    assert_eq!(&decompressed[..decompressed_len], payload);

    // 3. Ultra-fast LZ4 Block Compression
    let mut lz4_buf = vec![0u8; payload.len() * 2];
    let lz4_len = lz4_compress(payload, &mut lz4_buf)?;
    lz4_buf.truncate(lz4_len);

    let mut lz4_orig = vec![0u8; payload.len()];
    let lz4_orig_len = lz4_decompress(&lz4_buf, &mut lz4_orig)?;
    assert_eq!(&lz4_orig[..lz4_orig_len], payload);

    println!("In-memory roundtrips verified successfully!");
    Ok(())
}
```

---

## 4. In-Place Atomic Archive Mutations

TTZip provides transactional in-place mutation for ZIP and 7z archives without recompressing untouched entries:

```rust
use std::path::PathBuf;
use ttzip_engine::archive::in_place_edit::{InPlaceSession, ArchiveMutation};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let archive_path = PathBuf::from("dist/live_data.zip");

    // Begin atomic transaction
    let mut session = InPlaceSession::begin(&archive_path)?;

    // Queue mutations
    session.append("config/new_setting.json", &PathBuf::from("local/new_setting.json"))?;
    session.replace("README.txt", &PathBuf::from("local/UPDATED_README.txt"))?;
    session.delete("obsolete_cache.bin")?;

    // Atomic commit (writes to APFS shadow file, atomic swap on success)
    session.commit()?;
    println!("In-place mutations committed atomically.");
    Ok(())
}
```

---

## 5. Zero-Allocation Interactive VFS Search

For GUI/TUI applications with 100,000+ files, `RustVfsSession` maintains an in-memory tree with zero heap allocations during search queries:

```rust
use ttzip_engine::vfs::{RustVfsSession, VfsMatchDto};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let archive_path = "dist/large_project.7z";
    let session = RustVfsSession::open(archive_path, None)?;

    // Fixed-size preallocated match buffer
    let mut matches = [VfsMatchDto::default(); 50];
    let count = session.search_zero_alloc("Cargo.toml", &mut matches)?;

    println!("Found {} matches in <5ms:", count);
    for m in &matches[..count] {
        println!("  - {} (size: {} bytes)", m.path_str(), m.uncompressed_size);
    }

    Ok(())
}
```

---

## 6. Error Types & Safety Invariants

### Error Hierarchy

```rust
pub enum TTZipError {
    InvalidParameter(String),
    FileNotFound(PathBuf),
    CorruptHeader { offset: u64, reason: String },
    AuthenticationFailed,
    SecurityViolation(String), // Path traversal / Zip Slip attempts
    MmapFailed(String),
    IoError(std::io::Error),
    PanicCaught(String),
}
```

### Safety Guarantees
- **No Uncaught Panics**: All C-ABI boundary calls wrap internals in `std::panic::catch_unwind`.
- **Zero Memory Leaks**: All heap buffers use RAII `Vec<u8>` or `Box<T>` with deterministic drop semantics.
- **Crypto Zeroization**: Sensitive cryptographic keys implement `zeroize::ZeroizeOnDrop`.
