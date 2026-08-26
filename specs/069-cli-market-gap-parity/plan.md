# Implementation Plan: Comprehensive Market Gap Parity and Advanced Ergonomics for `ttzip-cli`

**Branch**: `069-cli-market-gap-parity` | **Date**: 2026-08-17 | **Spec**: [`spec.md`](spec.md)

**Input**: Feature specification from [`specs/069-cli-market-gap-parity/spec.md`](spec.md)

---

## 1. Summary

Elevate `ttzip-cli` into an industry-leading, full-featured archiving CLI that surpasses market tools (`7z`, `bsdtar`, `zip/unzip`, `ouch`) by combining TTZip's Apple Silicon in-process C performance with complete verb parity (`cat`, `tree`, `delete`, `update`, `hash`), zero-disk stdout stream piping, POSIX glob path exclusions/inclusions (`fnmatch`), process-safe non-echo credential handling (`readpassphrase`), interactive file collision resolution, and automated shell auto-completions for Zsh, Bash, Fish, and NuShell.

---

## 2. Technical Context

- **Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`) + C11 / POSIX APIs.
- **Primary Dependencies**: Native in-process `CTTZipBridge` static C library bindings (libarchive, libdeflate, LZMA SDK, zstd, ARM NEON SIMD).
- **Storage**: In-memory streaming and direct file system I/O (APFS / HFS+).
- **Testing**: `swift test` (XCTest), `Tests/TTZipTests/CLICommandE2ETests.swift`, `CLIPOSIXStandardTests.swift`.
- **Target Platform**: macOS 14.0+ (Sonoma, Sequoia) on Apple Silicon (ARM64) and Intel (x86_64).
- **Project Type**: Native High-Performance Command Line Tool (`ttzip-cli`).
- **Performance Goals**: Cold startup latency $< 8\text{ ms}$, constant streaming memory $O(1) \le 64\text{ KB}$, throughput $\ge 8,000\text{ MB/s}$ for decompression.
- **Constraints**: 100% In-Process (zero CLI child process spawning), volatile credential eradication, strict POSIX exit codes (`0..8`).

---

## 3. Constitution Check

*GATE: Verified against `.specify/memory/constitution.md`.*

| Invariant | Requirement | Status | Compliance Details |
| :--- | :--- | :--- | :--- |
| **I. Stream-First** | Zero whole-file memory assumption; micro-buffering pull pipeline | **PASS** | `ttzip-cli cat` and `extract -o -` stream in 64KB chunks directly to `STDOUT_FILENO` without whole-file allocation. |
| **II. Invariant-First** | POSIX原语级防御, TOCTOU免疫, `O_NOFOLLOW` | **PASS** | `--password-file` and path resolvers use `O_NOFOLLOW` and secure symlink validations. |
| **III. Bounds-First** | 敏感凭据防篡改擦除 (`memset_s`), 整数 Clamp | **PASS** | All credential buffers wiped via `ttzip_secure_zero` / `PlatformMemory.secureZero`; non-echo terminal acquisition. |
| **IV. Oracle-First** | 跨生态双向差分测试与黄金语料库验证 | **PASS** | CLI E2E tests assert round-trip exact matches and compatibility with system `/usr/bin/tar` and `/usr/bin/unzip`. |
| **Zero CLI Processes** | Zero external CLI subprocess invocations | **PASS** | 100% in-process execution via `CTTZipBridge` and `TTZipCore`. |

---

## 4. Phase 0: Research Summary

All research investigations were completed and verified by dedicated research subagents:

- R001 [SUBAGENT:research] 《Direct Memory/Stdout Streaming Extraction in Swift & POSIX C》: Resolved via `ttzip_stream_archive_entries_to_fd` using `archive_read_data_block` and POSIX `write(1, ...)`, with `isatty(STDOUT_FILENO)` binary protection guard. (See [`research.md#1`](research.md#1)).
- R002 [SUBAGENT:research] 《POSIX Glob & Path Pattern Filtering with Component Stripping》: Resolved via Darwin libc `fnmatch(3)` and single-pass pointer index scanner `stripLeadingComponents`, with built-in `--exclude-vcs` and `--no-mac-metadata` presets. (See [`research.md#2`](research.md#2)).
- R003 [SUBAGENT:research] 《Process-Safe Credential Input and Volatile Memory Eradication》: Resolved via 6-tier credential hierarchy using Darwin `readpassphrase(3)`, `--password-file`, and `TTZIP_PASSWORD`, with ISO C11 Annex K `memset_s` volatile memory zeroing. (See [`research.md#3`](research.md#3)).
- R004 [SUBAGENT:research] 《Modern Terminal Ergonomics, Interactive Overwrite Resolution, and Shell Integrations》: Resolved via `TerminalPagerEngine` (`less -RFX`), `ArchiveVisualTreeRenderer`, `FileCollisionResolver` (`/dev/tty` prompt), and declarative completion generators for Zsh/Bash/Fish/NuShell. (See [`research.md#4`](research.md#4)).

---

## 5. Phase 1: Design Artifacts & Contracts

- **Data Model**: Defined in [`data-model.md`](data-model.md) covering `CLIOptions`, `ArchiveFilterOptions`, `FileCollisionPolicy`, `ArchiveEntryInfo`, and `ArchiveTreeItem`.
- **System Interface Contracts**:
  - [`contracts/cli-options-contract.json`](contracts/cli-options-contract.json) [SUBAGENT:research]
  - [`contracts/cli-json-output-contract.json`](contracts/cli-json-output-contract.json) [SUBAGENT:research]
  - [`contracts/cli-progress-event-contract.json`](contracts/cli-progress-event-contract.json) [SUBAGENT:research]
- **Validation Guide**: [`quickstart.md`](quickstart.md) detailing 5 concrete runnable validation scenarios with failure diagnostics.

---

## 6. Architecture & Component Breakdown

```text
TTZip/
├── Sources/
│   ├── CTTZipBridge/
│   │   ├── CTTZipBridge_Archive.c    # [MODIFY] ttzip_stream_archive_entries_to_fd, glob streaming
│   │   ├── include/CTTZipBridge_Archive.h # [MODIFY] Header declarations
│   │   └── CTTZipUtils.c             # [MODIFY] Binary detection heuristic ttzip_is_buffer_binary
│   ├── TTZipCore/
│   │   ├── CLI/
│   │   │   ├── CLIOptions.swift      # [MODIFY] Extended options (filter, password, overwrite, tree)
│   │   │   ├── CLICommandSpec.swift  # [MODIFY] Added cat, tree, hash subcommands; fish/nu completions
│   │   │   ├── POSIXCLIArgumentParser.swift # [MODIFY] Parsing of --exclude, --include, --strip-components, etc.
│   │   │   ├── FileCollisionResolver.swift  # [NEW] Interactive [y/n/A/N/b/d] resolver via /dev/tty
│   │   │   ├── TerminalPagerEngine.swift    # [NEW] Auto-pager integration via less -RFX
│   │   │   └── ArchiveVisualTreeRenderer.swift # [NEW] Unicode tree renderer with depth filtering
│   │   └── Security/
│   │       └── PathPatternFilterEngine.swift # [NEW] Fast POSIX fnmatch pattern filtering engine
│   └── TTZipCLI/
│       ├── CLICommandRouter.swift    # [MODIFY] Route cat, tree, hash, in-place manipulation
│       └── TTZipCLIApp.swift         # [MODIFY] Wire global signal handling and credential safety
└── Tests/TTZipTests/
    └── CLICommandE2ETests.swift      # [NEW] Comprehensive E2E test suite for all new CLI capabilities
```
