# Quickstart & Verification Guide: Python SDK (`ttzip`)

**Feature**: `217-python-native-pyo3-sdk`  
**Status**: `READY_FOR_VERIFICATION`  

---

## 1. Quick Installation & Development Build

```bash
# 1. Build and install native extension into current Python environment
./scripts/build_python.sh

# 2. Or build using maturin directly
maturin develop --release
```

---

## 2. Basic Python Usage Examples

### 2.1 Extract an Archive with Zip Slip Protection
```python
import ttzip

# Extract a 7z or ZIP archive
ttzip.extract("archive.7z", destination="/tmp/extracted")
```

### 2.2 Compress Files with AES-256 Encryption
```python
import ttzip

ttzip.compress(
    sources=["folder/to/backup", "file.txt"],
    destination="secure_backup.7z",
    format="7z",
    level=9,
    password="SecretPassword2026"
)
```

### 2.3 In-Memory Buffer Compression & Hardware SIMD CRC
```python
import ttzip

data = b"Hello, TTZip High-Performance Python World!" * 1000

# Compress buffer using Zstandard
compressed = ttzip.compress_buffer(data, format="zstd", level=3)

# Decompress buffer
decompressed = ttzip.decompress_buffer(compressed, format="zstd")
assert decompressed == data

# Hardware SIMD CRC32 (>40 GB/s)
checksum = ttzip.crc32(data)
print(f"CRC32: {checksum:#010x}")
```

---

## 3. Run Automated Python Test Suite

```bash
pytest python/tests -v
```
