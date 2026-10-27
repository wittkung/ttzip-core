# Quickstart Validation Guide: Feature 069

**Feature**: `069-cli-market-gap-parity` (Comprehensive Market Gap Parity & Terminal Ergonomics)  
**Date**: 2026-08-17  
**Status**: Ready  

---

## 1. Prerequisites & Environment Setup

```bash
# Build the standalone release CLI binary
swift build -c release --product ttzip-cli

# Define alias for testing
alias ttzip="./.build/release/ttzip-cli"
```

---

## 2. Validation Scenarios

### Scenario 1: Direct Memory/Stdout Streaming Extraction (`cat`)

- **Command**:
  ```bash
  ttzip cat Tests/TTZipTests/Fixtures/sample.zip README.md | grep "TTZip"
  ```
- **Expected Output**:
  - Direct stream of matching lines without any intermediate files created on disk.
  - Zero terminal corruption or error logs.
- **Failure Diagnostic**:
  - If error `ttzip-cli: error: entry not found`: Check archive contents via `ttzip list Tests/TTZipTests/Fixtures/sample.zip`.
  - If error `ttzip-cli: error: stdout is a terminal and contains binary data`: Use `-f` / `--force` or redirect to pipe.

---

### Scenario 2: Glob Pattern Filtering & VCS / macOS Junk Exclusions

- **Command**:
  ```bash
  # Create archive with automatic VCS and macOS metadata exclusion
  ttzip archive /tmp/test_clean.tar.zst Sources/ --exclude "*.swift" --exclude-vcs --no-mac-metadata
  
  # Inspect resulting entries
  ttzip list /tmp/test_clean.tar.zst
  ```
- **Expected Output**:
  - Archive contains C and header files, with 0 `.swift` files and 0 `.git` / `.DS_Store` entries.
- **Failure Diagnostic**:
  - If `.swift` files are present: Verify `fnmatch` wildcard evaluation in `PathPatternFilterEngine`.

---

### Scenario 3: Hierarchical Visual Tree Rendering (`tree`)

- **Command**:
  ```bash
  ttzip tree Tests/TTZipTests/Fixtures/sample.zip --depth 2
  ```
- **Expected Output**:
  ```text
  sample.zip
  ├── docs/ (2 files)
  │   ├── manual.md (14.2 KB)
  │   └── arch.png (128.5 KB)
  └── src/ (3 files)
      ├── main.c (4.1 KB)
      └── utils.h (1.8 KB)

  2 directories, 4 files (Total: 148.6 KB)
  ```
- **Failure Diagnostic**:
  - If tree does not show branching glyphs: Ensure `TerminalRenderEngine` is using UTF-8 Unicode box-drawing characters.

---

### Scenario 4: Process-Safe Password Extraction

- **Command**:
  ```bash
  export TTZIP_PASSWORD="TestPassword123"
  ttzip extract Tests/TTZipTests/Fixtures/encrypted_aes256.7z -o /tmp/vault_out/
  ```
- **Expected Output**:
  - `✅ Extraction completed: /tmp/vault_out/`
  - Zero password visible in `ps aux` command arguments.
  - `TTZIP_PASSWORD` safely unset from memory after execution.
- **Failure Diagnostic**:
  - If exit code 4 (Auth error): Verify passphrase correctness and `readpassphrase` / env acquisition logic.

---

### Scenario 5: Machine-Readable NDJSON Streaming & POSIX Exit Codes

- **Command**:
  ```bash
  ttzip test Tests/TTZipTests/Fixtures/sample.zip --json
  ```
- **Expected Output**:
  ```json
  {"schema_version":"1.0.0","event":"completed","exit_code":0,"status":"success","payload":{"operation":"test","duration_seconds":0.004,"total_bytes":148600,"throughput_mbs":37150.0}}
  ```
- **Failure Diagnostic**:
  - Validate JSON against [`contracts/cli-json-output-contract.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/069-cli-market-gap-parity/contracts/cli-json-output-contract.json).
