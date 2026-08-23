# Data Model: 190-domain-responsibility-realignment-and-core-lightweighting

## 1. Domain Target Architecture
- **`TTZipCore`**:
  - Pure production library (Zip, 7z, Tar, Gz, Zstd, Brotli, Split, Crypto, Security, Localization, VFS).
  - Target file count: $\le 50$ files.
- **`TTZipBench`**:
  - Independent benchmark CLI (`ttzip-bench`).
- **`TTZipCLI`**:
  - POSIX command-line tool (`ttzip-cli`).
- **`rust/ttzip-tui`**:
  - Standalone high-performance TUI binary (`bin/ttzip`).
