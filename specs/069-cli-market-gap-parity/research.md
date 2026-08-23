# Phase 0 Research & Technology Selection: Feature 069

**Feature**: `069-cli-market-gap-parity` (Comprehensive Market Gap Parity & Terminal Ergonomics)  
**Date**: 2026-08-17  
**Status**: Completed  

---

## 1. Direct Memory/Stdout Streaming Extraction (`cat` / `extract -o -`)

- **Decision**: 
  1. Implement `ttzip-cli cat <archive> <entry>` and `ttzip-cli extract <archive> -o -` as first-class CLI verbs.
  2. Implement `ttzip_stream_archive_entries_to_fd` in `Sources/CTTZipBridge/CTTZipBridge_Archive.c` leveraging libarchive's `archive_read_open_filename` / `archive_read_open_fd(STDIN_FILENO)`, `archive_read_data_block()`, and direct POSIX `write(target_fd, ...)` in 64KB–1MB chunks with `archive_read_data_skip()` for non-matching entries.
  3. Enforce `isatty(STDOUT_FILENO)` detection. If stdout is connected to an interactive terminal and the first chunk contains binary control bytes (`\0` or non-printable control characters), halt immediately with an informative error on `stderr` unless overridden by `--force` / `-f`. If redirected or piped (`isatty == 0`), stream smoothly without interruption.
  4. Decouple and silence progress bars on `stdout` during streaming; all diagnostics and errors are directed exclusively to `stderr`.

- **Rationale**:
  - **Zero Disk Allocation & Constant Memory $O(1)$**: Streaming directly from the decompressor buffer to `STDOUT_FILENO` avoids creating temporary spool files on APFS/disk, maintaining a steady ~64KB working set even on 50GB+ payloads.
  - **Universal 16-Format Support**: Handles ZIP, 7Z, TAR, TAR.ZST, TAR.GZ, TAR.BZ2, TAR.XZ, LZ4, BROTLI, SNAPPY, RAR, CAB, ISO, and WIM symmetrically.
  - **Standard UNIX Conformance**: Matches the exact behavior of `tar -xOf -`, `bsdtar -O`, and `unzip -p`, enabling seamless UNIX pipe composition (e.g., `ttzip-cli cat archive.zip config.json | jq .`).

- **Alternatives Considered**:
  - *Extract to Temporary Disk File & Read-to-Stdout*: Rejected because it introduces severe disk I/O latency, APFS write amplification, flash wear, security/quarantine leakage, and failure risks on full disks or read-only containers.
  - *Full In-Memory Buffer Decompression (`ZipMemoryEngine` returning `Data`)*: Rejected due to OOM risk when extracting large payload files (e.g., 50GB database dump or VM image) and high memory allocation pressure.
  - *Child Process Spawning of `/usr/bin/tar` or `/usr/bin/unzip`*: Rejected to strictly adhere to TTZip’s fundamental architectural mandate of "100% In-Process C static library bindings with zero external CLI subprocess execution".

- **Source**:
  - `Vendor/TTZipVendor.xcframework/macos-arm64/Headers/archive.h` (`archive_read_data_into_fd`, `archive_read_data_block`)
  - `Sources/CTTZipBridge/CTTZipBridge_Archive.c` (lines 55–93, 160–260)
  - `Sources/TTZipCore/CLI/TerminalRenderEngine.swift` (lines 22–37: `isatty(STDOUT_FILENO)`)
  - `Sources/TTZipCore/CLI/StreamPipeAdapter.swift` (lines 9–11: `isStandardStream`)

---

## 2. POSIX Glob & Path Pattern Filtering with Component Stripping

- **Decision**: 
  1. Implement glob filtering via Darwin/POSIX native C `fnmatch(3)` (`Darwin.fnmatch`) combined with a two-tier matching strategy (root-anchored path matching with `FNM_PATHNAME` when pattern contains `/`, vs basename/component matching when pattern does not contain `/`) and zero-allocation inline fast-paths (prefix `*.<ext>` -> `hasSuffix`, exact string equality `==`).
  2. Implement single-pass zero-allocation path index scanner `stripLeadingComponents(_ path: String, count: Int) -> String?` in `TTZipCore` and native C bridge equivalent `ttzip_path_strip_components`. Redundant slashes and leading `./` are normalized in a single pass; if total component count $\le N$, the entry is skipped entirely.
  3. Expose first-class preset flags in `ArchiveFilterOptions` (`excludeVCS: Bool = false`, `noMacMetadata: Bool = true` / `skipMacJunk: Bool = true`) backed by $O(1)$ `Set<String>` lookup tables and C-level fast path filters.
  4. Implement `FileFilterListLoader` with asynchronous chunked line streaming using `FileHandle.bytes.lines` (AsyncLineSequence) for standard files or `stdin` (`-`), supporting comment stripping (`#`), whitespace trimming, and null-character (`\0`) delimiter support (`--null` / `-0`).

- **Rationale**:
  - `fnmatch(3)` is the standard POSIX.2 pattern matching engine built directly into Darwin libc (`libSystem.B.dylib`), natively supporting shell wildcards (`*`, `?`, `[...]`, `[!...]`, `\`) and case-insensitive matching (`FNM_CASEFOLD`) with zero heap allocations per match (~25–50 ns/call vs 1,500–4,500 ns/call for `NSRegularExpression`).
  - `--exclude-vcs` standardizes exclusion of all VCS metadata directories (`.git`, `.svn`, `.hg`, `.gitignore`, etc.).
  - `--no-mac-metadata` filters `.DS_Store`, AppleDouble resource fork headers `._*`, and `__MACOSX/` directories.
  - Streaming line loading prevents OOM when loading 500,000+ line manifests.

- **Alternatives Considered**:
  - `NSRegularExpression`: Rejected due to heavy heap allocation per regex object, regex compilation latency on hot scanning paths, and divergence from POSIX glob wildcards.
  - `path.components(separatedBy: "/").dropFirst(n).joined(separator: "/")`: Rejected because allocating arrays of substring components for 100,000+ files causes heavy heap allocation and ARC churn.
  - Requiring users to manually pass dozens of `--exclude` globs: Rejected as verbose, error-prone, and inconsistent.

- **Source**:
  - `man 3 fnmatch` (Darwin POSIX specification)
  - `Sources/TTZipCore/ArchiveFilterOptions.swift` (lines 1–22)
  - `Sources/TTZipCore/Zip/ZipDirectoryScanner.swift` (lines 17–92)
  - `Sources/CTTZipBridge/CTTZipExtract.c` (lines 184–188)

---

## 3. Process-Safe Credential Input and Volatile Memory Eradication

- **Decision**: 
  1. Define a strict 6-tier credential resolution pipeline:
     - Tier 1: `--password` / `-p <pwd>` (CLI argument, emits visible security warning to stderr advising against passing secrets via CLI arguments)
     - Tier 2: `--password-file <path>` / `-P <path>` (Dedicated credential file opened with `O_RDONLY | O_NOFOLLOW`)
     - Tier 3: `TTZIP_PASSWORD` (Environment variable; immediately unset after reading via `unsetenv`)
     - Tier 4: `PasswordVaultManager` auto-unlock candidates (Keychain/Vault v4)
     - Tier 5: Interactive non-echo TTY prompt via native Darwin `readpassphrase(3)` (`RPP_ECHO_OFF | RPP_REQUIRE_TTY`) if `isatty(STDIN_FILENO)`
     - Tier 6: Fail-fast with `ArchiveError.passwordRequired` / `CLIExitCode.dataError`
  2. Implement volatile memory eradication across C and Swift layers:
     - **C Layer**: Use `ttzip_secure_zero(void* ptr, size_t len)` in `CTTZipCommon.h` which calls ISO C11 Annex K `memset_s(ptr, len, 0, len)` on Darwin/Apple with volatile pointer and compiler memory barriers `__asm__ __volatile__("" : : "r"(ptr) : "memory")`.
     - **Swift Layer**: Use `PlatformMemory.secureZero` to guarantee zero compiler dead-store elimination.
  3. Wrap credentials in a `SecureCredentialBuffer` struct rather than mutable Swift strings to avoid uncontrolled COW copies.

- **Rationale**: 
  - `argv` passed to `execve` is stored in the OS process table and process memory (`KERN_PROCARGS2`), visible in `ps aux` / Activity Monitor / audit logs.
  - Darwin's `readpassphrase(3)` (in `<readpassphrase.h>`) is the official Apple BSD security API for terminal password collection. It opens `/dev/tty`, disables echo, traps interrupt signals, and writes directly into a caller-supplied fixed buffer.
  - Standard `memset(ptr, 0, len)` is routinely eliminated by LLVM's `DeadStoreEliminationPass` in Release builds (`-O3`). `memset_s` guarantees physical wiping.

- **Alternatives Considered**: 
  - *Overwriting `argv[i]` in `main()`*: Rejected because macOS kernel retains original arguments in `KERN_PROCARGS2` which `ps` reads via `sysctl`; there is also an unavoidable race window between `execve` and user-space wiping.
  - *POSIX `getpass(3)`*: Rejected because it is marked legacy/obsolete (removed in POSIX.1-2008), uses a static internal 128-byte buffer, is thread-unsafe, and silently truncates long modern passphrases.
  - *Standard `memset(buf, 0, len)`*: Rejected because Clang/LLVM dead-store elimination removes the write in Release builds, violating Constitution Invariant III.

- **Source**: 
  - macOS Darwin `<readpassphrase.h>` (`readpassphrase(3)`)
  - `Sources/CTTZipBridge/include/CTTZipCommon.h` (`ttzip_secure_zero`)
  - `Sources/CTTZipBridge/CTTZipBridge_Crypto.c` (lines 570–624)
  - `Sources/TTZipCore/Platform/PlatformMemory.swift` (lines 68–109)
  - `Sources/TTZipCore/Services/ArchivePasswordStore.swift` (lines 87–95)

---

## 4. Modern Terminal Ergonomics, Interactive Overwrite Resolution, and Shell Integrations

- **Decision**: 
  1. **Terminal Auto-Pager (`TerminalPagerEngine`)**: Auto-detect `isatty(STDOUT_FILENO)`. When interactive and output lines exceed terminal height (`ioctl(TIOCGWINSZ)`), pipe output through `$TTZIP_PAGER` / `$PAGER` / `less -RFX` (`-R` ANSI color passthrough, `-F` auto-exit if one screen, `-X` preserve screen on exit). Bypass completely when redirected to pipe/file or in `--json` mode.
  2. **Visual Hierarchy Tree (`ArchiveVisualTreeRenderer`)**: Implement `ttzip-cli tree <archive> [--depth N]` using `ArchiveComponentProtocol` composite tree, rendering Unicode branches (`├──`, `└──`, `│   `), file icons, human-readable sizes, and summary totals (`N directories, M files`).
  3. **Interactive File Collision Resolver (`FileCollisionResolver`)**:
     - Support `--overwrite [always|never|newer|backup|prompt]` (defaulting to `prompt` in interactive TTY and `always` under `-y`).
     - In interactive TTY, prompt `Overwrite? [y]es / [n]o / [A]ll / [N]one / [b]ackup / [d]iff / [q]uit: ` via `/dev/tty` (preventing stdin stream pollution).
     - Support dynamic session upgrade to `always` (`A`) or `never` (`N`) and `.bak` generation.
  4. **Declarative Shell Auto-Completions**: Expand `CLICommandSpec` to generate complete, high-precision auto-completion scripts for Zsh, Bash, Fish, and NuShell, plus UNIX groff Section 1 man page (`ttzip-cli.1`).

- **Rationale**: 
  - Aligns with modern CLI standards set by Git, Cargo, Ripgrep, and Ouch.
  - Interactive TTY prompt via `/dev/tty` allows collision resolution even when `stdin` is piped.
  - Declarative single-source-of-truth completion generator ensures zero drift across 4 major shells.

- **Alternatives Considered**: 
  - *Custom TUI Scroll View in Swift*: Rejected due to high complexity and breaking user muscle memory with `$PAGER`.
  - *Reading prompts from `stdin`*: Rejected because piping archives (e.g. `cat file.zip | ttzip-cli extract -`) would cause `readLine()` to ingest archive binary data.
  - *Maintaining static completion files*: Rejected due to maintenance drift.

- **Source**: 
  - `Sources/TTZipCore/CLI/TerminalRenderEngine.swift` (lines 22–37)
  - `Sources/TTZipCore/ArchiveComponentProtocol.swift` (lines 94–115)
  - `Sources/TTZipCore/CLI/CLICommandSpec.swift` (lines 38–171)
  - `Sources/TTZipCLI/CLICommandRouter.swift` (lines 158–166)
