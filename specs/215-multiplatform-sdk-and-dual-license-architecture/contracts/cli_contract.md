# Interface Contract: Standalone POSIX CLI (`ttzip`)

**Feature**: `215-multiplatform-sdk-and-dual-license-architecture`  
**Status**: `FROZEN` (Revised based on existing 19-command ttzip-tui codebase)  

---

## 1. Binary & Global Options

The binary executable is named `ttzip` (built from `rust/ttzip-cli`, previously `ttzip-tui`).

```text
ttzip [OPTIONS] <COMMAND> [ARGS...]
```

### Global Flags
- `-v, --version`: Displays version and target triple (e.g. `ttzip 1.0.0 (aarch64-apple-darwin)`).
- `-h, --help`: Displays help information.
- `--json`: Enables structured NDJSON output stream on stdout (disabling interactive TUI/progress bars).
- `-q, --quiet`: Suppresses non-error terminal output.
- `--threads <N>`: Thread budget (defaults to hardware P-core topology).

---

## 2. 19 Subcommand Specification

| Subcommand | Aliases | Description |
| :--- | :--- | :--- |
| `create` | `c`, `compress`, `a` | Compress files/directories into an archive (ZIP, 7z, TAR, GZ, ZSTD) |
| `extract` | `x`, `e` | Extract archive contents with directory traversal safety |
| `list` | `l`, `ls` | List archive entry table |
| `tree` | `t` | Render visual ANSI directory hierarchy |
| `bench` | `b` | Run in-memory MIPS / SIMD throughput benchmarks |
| `check` | `verify` | Verify header and CRC32/SHA256 checksum integrity |
| `repair` | `fix` | Salvage damaged ZIP/TAR headers via self-healing engine |
| `diff` | `d` | Compare contents of two archives |
| `split` | `part` | Split files or archives into multi-volume parts |
| `hash` | `sum` | Calculate hardware-accelerated CRC32, Adler32, CRC64, SHA256 |
| `convert` | `repack` | Convert between archive formats without full disk extraction |
| `recover` | `crack` | High-speed multi-core dictionary/brute-force password recovery |
| `cat` | `view` | Print content of a single file inside an archive to stdout |
| `comment` | - | Read or update archive comment field |
| `doctor` | - | Diagnostic report of host CPU SIMD, APFS, and memory topology |
| `info` | - | Detailed container metadata and compression ratios |
| `delete` | `rm` | In-place entry removal from an archive |
| `lock` | - | Set password / AES-256 encryption on existing archives |
| `update` | `u` | In-place addition/replacement of modified files |

---

## 3. Exit Code Specifications

| Exit Code | Identifier | Description |
| :--- | :--- | :--- |
| `0` | `SUCCESS` | Operation completed successfully with zero errors. |
| `1` | `GENERIC_ERROR` | Unspecified runtime error. |
| `2` | `INVALID_ARGUMENT`| Bad command-line arguments or unknown flags. |
| `3` | `IO_ERROR` | File not found, permission denied, or disk full. |
| `4` | `CORRUPT_ARCHIVE` | Checksum mismatch or invalid archive structure. |
| `5` | `AUTH_FAILURE` | Missing or incorrect password. |
| `6` | `SECURITY_ALERT` | Zip Slip path traversal attack detected and blocked. |
| `130` | `USER_CANCELLED`| Interrupted by SIGINT / Ctrl+C. |
