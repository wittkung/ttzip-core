# Data Model & Python Type Definitions

**Feature**: `217-python-native-pyo3-sdk`  
**Status**: `SPECIFIED`  

---

## 1. Python Class Models

### 1.1 `EntryMetadata`
```python
from dataclasses import dataclass
from typing import Optional

@dataclass(frozen=True)
class EntryMetadata:
    path: str
    uncompressed_size: int
    compressed_size: int
    crc32: int
    mtime_epoch_secs: int
    is_directory: bool
    is_encrypted: bool
```

### 1.2 `ProgressInfo`
```python
@dataclass(frozen=True)
class ProgressInfo:
    bytes_processed: int
    bytes_total: int
    current_entry: str
    fraction_completed: float
```

---

## 2. Exception Hierarchy

```text
Exception
 └── TTZipError (Base)
      ├── AuthenticationError (Wrong or missing password)
      ├── CorruptArchiveError (Header damaged or checksum mismatch)
      ├── SecurityError (Zip Slip or path traversal attack)
      └── FormatNotSupportedError (Unknown container format)
```

---

## 3. Package Manifest (`pyproject.toml`)

```toml
[build-system]
requires = ["maturin>=1.5,<2.0"]
build-backend = "maturin"

[project]
name = "ttzip"
version = "1.0.0"
description = "High-throughput, SIMD-accelerated archive and compression engine for Python"
authors = [{ name = "Witt Kung", email = "witt.w.kung@gmail.com" }]
license = { text = "BSD-3-Clause OR Apache-2.0" }
readme = "README.md"
requires-python = ">=3.10"
classifiers = [
    "Programming Language :: Python :: 3",
    "Programming Language :: Rust",
    "License :: OSI Approved :: BSD License",
    "License :: OSI Approved :: Apache Software License",
    "Topic :: System :: Archiving :: Compression",
]

[tool.maturin]
module-name = "ttzip._ttzip"
features = ["pyo3/extension-module"]
python-source = "python"
```
