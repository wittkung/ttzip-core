# Interface Contract: Python SDK (`ttzip`)

**Feature**: `217-python-native-pyo3-sdk`  
**Status**: `FROZEN`  

---

## 1. Top-Level Function Signatures (`python/ttzip/__init__.pyi`)

```python
from typing import List, Optional, Union, Callable
from pathlib import Path
from .models import EntryMetadata, ProgressInfo
from .exceptions import TTZipError, AuthenticationError, CorruptArchiveError, SecurityError

def compress(
    sources: Union[str, Path, List[Union[str, Path]]],
    destination: Union[str, Path],
    format: str = "auto",
    level: int = 6,
    password: Optional[str] = None,
    threads: int = 0,
    progress_callback: Optional[Callable[[ProgressInfo], bool]] = None
) -> None:
    """
    Compresses files or directories into an archive (ZIP, 7z, TAR, GZ, ZSTD).
    Releases Python GIL during execution.
    """
    ...

def extract(
    archive: Union[str, Path],
    destination: Union[str, Path],
    password: Optional[str] = None,
    threads: int = 0,
    progress_callback: Optional[Callable[[ProgressInfo], bool]] = None
) -> None:
    """
    Extracts an archive safely with built-in Zip Slip protection.
    Releases Python GIL during execution.
    """
    ...

def inspect(
    archive: Union[str, Path],
    password: Optional[str] = None
) -> List[EntryMetadata]:
    """
    Inspects archive entry metadata without disk extraction.
    """
    ...

def decompress_buffer(
    data: bytes,
    format: str = "deflate"
) -> bytes:
    """
    Decompresses an in-memory buffer (deflate, zstd, lz4, snappy, brotli).
    """
    ...

def compress_buffer(
    data: bytes,
    format: str = "deflate",
    level: int = 6
) -> bytes:
    """
    Compresses an in-memory buffer.
    """
    ...

def crc32(data: bytes, seed: int = 0) -> int:
    """
    Computes SIMD-accelerated CRC-32 checksum (>40 GB/s on Apple Silicon / AVX-512).
    """
    ...

def crc64(data: bytes, seed: int = 0) -> int:
    """
    Computes SIMD-accelerated CRC-64 checksum.
    """
    ...

def version() -> str:
    """Returns the underlying TTZip engine version string."""
    ...

def is_hardware_accelerated() -> bool:
    """Returns True if ARM NEON / PMULL or x86 AVX2/AES-NI acceleration is active."""
    ...
```
