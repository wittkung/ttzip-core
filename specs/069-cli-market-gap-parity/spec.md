# Feature Specification: Comprehensive Market Gap Parity and Advanced Ergonomics for `ttzip-cli`

**Feature Branch**: `069-cli-market-gap-parity`  
**Created**: 2026-08-17  
**Status**: Draft  
**Input**: Comprehensive comparative gap analysis between `ttzip-cli` and mature market CLI tools (`7z`, `bsdtar`, `zip/unzip`, `ouch`, `ripgrep/fd`), establishing full feature parity and industry-leading terminal ergonomics.

---

## 1. Executive Summary & Market Competitor Benchmark

A systematic comparative evaluation of `ttzip-cli` against mature industrial archiving tools (`7z`/`7zz`, `bsdtar`, `zip`/`unzip`, `ouch`, `keka-cli`) reveals that while TTZip possesses unmatched in-process C-level Apple Silicon throughput (up to 28+ GB/s), its CLI interface has key functional and ergonomic gaps across ten critical dimensions:

| Dimension | Market Benchmark (`7z` / `bsdtar` / `ouch`) | Existing `ttzip-cli` | Target Parity & Enhancement Specification |
| :--- | :--- | :--- | :--- |
| **1. Command Verbs & Actions** | `create`, `extract`, `list`, `test`, `delete`, `update`, `rename`, `cat`, `tree`, `diff`, `hash` | `archive`, `extract`, `list`, `test`, `bench`, `recover`, `repair`, `diff` | Add `cat`/`view` (stdout stream), `tree` (visual hierarchy), `delete` (in-archive deletion), `update` (sync/freshen newer files), `hash` (standalone entry checksums). |
| **2. Pattern Filtering & Exclude** | `--exclude <glob>`, `--include <glob>`, `--files-from <list>`, `--strip-components <N>`, `--flatten` | Direct path inputs only; no glob/exclude filtering | Full glob exclusion/inclusion (`-x`/`--exclude`, `-i`/`--include`), `--strip-components <N>`, `--flatten`/`-j`, and `--files-from <file>`. |
| **3. UNIX Pipe & Filter Composability** | Full `stdin`/`stdout` streaming (`-so`, `unzip -p`, `tar -cf -`) | Primitive stdin temporary file spooling | True streaming stdout extraction (`ttzip-cli cat archive.zip path/file.txt`), stdout stream creation, direct pipeline integration without mandatory temp disk files. |
| **4. macOS Metadata & VCS Hygiene** | `--no-mac-metadata`, `--exclude-vcs`, `--strip-quarantine` | None (packages all hidden files by default) | Built-in smart exclusion presets: `--exclude-vcs` (git/svn), `--no-mac-metadata` (`.DS_Store`, `__MACOSX`, resource forks), `--strip-quarantine`. |
| **5. Overwrite & Collision Policies** | Interactive collision prompt `[y/n/A/N/r/d]`, `--no-clobber`, `--backup` | Boolean `--yes` only (unpredictable overwrite) | Granular collision policy: `--overwrite [always|never|newer|backup|prompt]`, `-n`/`--no-clobber`, interactive TTY resolution dialog. |
| **6. Security & Credential Safety** | Non-echo TTY password prompt, `--password-file`, env vars | Plaintext `-p <password>` CLI flag only | **Process-table leak prevention**: Secure hidden stdin password prompt on encrypted archives, `--password-file <path>`, and `TTZIP_PASSWORD` environment variable. |
| **7. Terminal Ergonomics & Rendering** | Auto-paging (`$PAGER`), 60Hz adaptive ETA/speed, ANSI themes | Basic single-line progress, flat text lists | Auto-pager integration for large archives (`less -RFX`), live dynamic TTY progress with ETA, colorful tree rendering, NDJSON non-TTY emission. |
| **8. Multi-Volume Cataloging** | Auto continuous volume discovery (`.7z.001`, `.z01`), health diagnostics | Basic split size `-s` creation only | Multi-volume auto-sequencing on extraction, missing chunk diagnostic warnings, volume manifest inspection. |
| **9. Machine-Readable Automation** | Unified JSON Schema across all commands, strict exit code taxonomy | Partial JSON mode on limited commands | 100% JSON Schema coverage via `--json` across all commands; POSIX exit code taxonomy (`0`..`8`). |
| **10. Shell Ecosystem & Docs** | Completions for Zsh/Bash/Fish/Nushell, groff mdoc man page | Basic Zsh/Bash templates | Production Zsh, Bash, Fish, and NuShell completions with dynamic archive format/level flags; Section 1 UNIX mdoc man page. |

---

## Clarifications

### Session 2026-08-17
- Q: What is the delivery scope and implementation strategy to fully catch up with and surpass mature market CLIs? → A: Deliver all 7 prioritized user stories (P1 to P3) and 9 functional requirements (FR-001 through FR-009) in a unified, zero-compromise implementation, combining 7z's command verbs & in-archive manipulation, bsdtar's UNIX pipeline composability, ouch's terminal ergonomics, and enterprise-grade process-safe credential handling.
- Q: How will streaming extraction without disk spooling be realized for POSIX composability? → A: Direct C in-process memory stream extraction (`ttzip-cli cat` and `extract -o -`) routing decompressed chunks straight to STDOUT_FILENO with binary terminal TTY safety warnings.
- Q: What is the credential protection mechanism for encrypted archives? → A: Non-echoing terminal reading via POSIX termios / readpassphrase, support for `--password-file <path>` and `TTZIP_PASSWORD` environment variable, with mandatory volatile zeroing (`memset_s`) upon completion.

---

## 2. User Scenarios & Testing *(mandatory)*

### User Story 1 - Advanced Pattern Filtering & Path Transformations (Priority: P1)

As a DevOps engineer or software developer, I want to create or extract archives while excluding unnecessary build artifacts (like `node_modules`, `.git`, `.DS_Store`) or stripping leading directory paths, so that my deployment packages remain lean and cleanly structured.

**Why this priority**: Filtering and path manipulation are the #1 daily requirement for CI/CD workflows and developer packaging.

**Independent Test**: Can be tested independently by running `ttzip-cli archive bundle.tar.zst src/ --exclude "*.git/*" --exclude ".DS_Store" --exclude-vcs` and verifying the resulting archive contains zero excluded files.

**Acceptance Scenarios**:
1. **Given** a directory with source files, `.git/` history, and `.DS_Store` files, **When** running `ttzip-cli archive out.zip src/ --exclude-vcs --no-mac-metadata`, **Then** the created archive must contain source files but omit all `.git` and `.DS_Store` entries.
2. **Given** an archive `archive.tar.gz` containing `app-v1.0.0/bin/exec`, **When** extracting with `ttzip-cli extract archive.tar.gz -o dist/ --strip-components 1`, **Then** `exec` is written directly into `dist/bin/exec` without the `app-v1.0.0` prefix.
3. **Given** a file list `includes.txt` with relative paths, **When** running `ttzip-cli archive bundle.7z --files-from includes.txt`, **Then** only paths enumerated in `includes.txt` are archived.

---

### User Story 2 - UNIX Stream Piping & Single-Entry Inspection (`cat` / `pipe`) (Priority: P1)

As a terminal power user or script author, I want to stream a specific file from an archive directly to `stdout` or pipe raw uncompressed streams into `ttzip-cli`, so that I can inspect logs, parse JSON configs, or chain UNIX filters (`grep`, `jq`, `sh`) without extracting entire archives to disk.

**Why this priority**: UNIX pipeline composability is the cornerstone of command-line tool utility.

**Independent Test**: Can be tested independently by running `ttzip-cli cat bundle.zip config.json | jq .version` and asserting the output matches the exact embedded JSON content without writing temporary files to disk.

**Acceptance Scenarios**:
1. **Given** a compressed archive `logs.tar.zst` containing `system.log`, **When** running `ttzip-cli cat logs.tar.zst system.log | grep "ERROR"`, **Then** only matching log lines are printed to stdout, with zero temporary files left on disk.
2. **Given** standard input stream from `tar -c src/`, **When** piping to `tar -c src/ | ttzip-cli archive - -f zst -o out.tar.zst`, **Then** `out.tar.zst` is created properly with valid Zstandard frames.
3. **Given** an encrypted archive `secrets.7z`, **When** running `ttzip-cli cat secrets.7z secret.txt -p pass123`, **Then** the decrypted file contents are streamed directly to stdout.

---

### User Story 3 - Process-Safe Credential & Password Management (Priority: P1)

As a security-conscious engineer, I want `ttzip-cli` to securely prompt for passwords without echoing keystrokes or accept passwords via environment variables/files, so that secrets are never leaked into shell history or `ps aux` process tables.

**Why this priority**: Exposing passwords in plaintext CLI arguments is a major security vulnerability.

**Independent Test**: Can be tested independently by running `ttzip-cli extract protected.zip -o out/` on an encrypted archive without `-p`, asserting a non-echoing terminal prompt appears, and confirming `ps aux` shows no plaintext password.

**Acceptance Scenarios**:
1. **Given** an AES-256 encrypted archive and no `-p` argument provided on an interactive TTY, **When** executing `ttzip-cli extract protected.zip -o out/`, **Then** the CLI prompts `Enter decryption password:` with disabled terminal echo, reads the password, and decrypts successfully.
2. **Given** the environment variable `TTZIP_PASSWORD=MySecret123!`, **When** executing `ttzip-cli extract protected.zip -o out/`, **Then** the CLI uses the environment variable automatically if `-p` is absent.
3. **Given** a file `secret.key` containing the password, **When** running `ttzip-cli archive secure.7z src/ --password-file secret.key`, **Then** the password is read securely from the file and wiped from memory after use via volatile zeroing.

---

### User Story 4 - Visual Tree Hierarchy & Auto-Paging Navigation (Priority: P2)

As a developer exploring large archives (e.g. multi-gigabyte ZIP or DMG files with 50,000+ files), I want a visual tree hierarchy (`ttzip-cli tree`) and automatic terminal paging (`$PAGER`), so that I can intuitively navigate folder structures without my terminal screen being flooded with thousands of unpaged lines.

**Why this priority**: Dramatically improves interactive user experience for complex archives.

**Independent Test**: Can be tested independently by running `ttzip-cli tree project.zip --depth 2` and verifying the structured Unicode/ASCII tree representation.

**Acceptance Scenarios**:
1. **Given** an archive with nested directory hierarchies, **When** running `ttzip-cli tree archive.zip`, **Then** a tree diagram with directory branch glyphs (`├──`, `└──`), entry counts, and sizes is rendered.
2. **Given** an archive listing exceeding current terminal height (e.g. > 25 entries) on an interactive TTY, **When** running `ttzip-cli list large_archive.zip`, **Then** output is automatically passed through `$PAGER` (defaulting to `less -RFX`).
3. **Given** output redirected to a pipe or file (`ttzip-cli list archive.zip > list.txt`), **Then** auto-paging is disabled, colors are stripped if requested, and plain lines are emitted.

---

### User Story 5 - In-Archive Manipulation: Delete, Update, and In-Place Sync (Priority: P2)

As a sysadmin or release engineer, I want to delete outdated files from an existing archive or update only modified files (`freshen` / `update`), so that I don't have to decompress and recompress massive archives from scratch.

**Why this priority**: Saves significant I/O and time when modifying existing release artifacts.

**Independent Test**: Can be tested independently by creating `test.zip` with 3 files, running `ttzip-cli delete test.zip file2.txt`, and verifying `test.zip` now only contains `file1.txt` and `file3.txt`.

**Acceptance Scenarios**:
1. **Given** an existing archive `bundle.zip` containing `deprecated.log`, **When** running `ttzip-cli delete bundle.zip deprecated.log`, **Then** `deprecated.log` is removed from the archive central directory and index.
2. **Given** an archive `app.zip` and updated local source files, **When** running `ttzip-cli update app.zip src/`, **Then** only files with newer modification timestamps or different checksums are re-compressed and updated in the archive.

---

### User Story 6 - Granular Overwrite Policies & Collision Resolution (Priority: P2)

As an automated build operator or interactive terminal user, I want explicit control over file collision behavior during extraction (skip, backup, overwrite newer, or interactive prompt), so that critical local files are never unintentionally clobbered.

**Why this priority**: Prevents silent data loss during archive extractions.

**Independent Test**: Can be tested independently by extracting an archive into a directory with pre-existing conflicting files under `--overwrite never` and confirming existing files are untouched.

**Acceptance Scenarios**:
1. **Given** an existing file `dist/README.md`, **When** extracting with `ttzip-cli extract archive.zip -o dist/ --overwrite never` (or `-n`/`--no-clobber`), **Then** existing files are skipped with a warning log.
2. **Given** an existing file `dist/config.json`, **When** extracting with `ttzip-cli extract archive.zip -o dist/ --overwrite backup`, **Then** the original file is preserved as `dist/config.json.bak` and the new file is extracted.
3. **Given** an interactive TTY and no overwrite flag specified, **When** a collision occurs, **Then** an interactive prompt appears asking `[y]es / [n]o / [A]ll / [N]one / [b]ackup / [d]iff`.

---

### User Story 7 - Complete Shell Auto-Completions & Man Page Documentation (Priority: P3)

As a system administrator using Zsh, Bash, Fish, or NuShell, I want full tab auto-completion for subcommands, format types, compression levels, and paths, as well as a complete UNIX man page (`man ttzip-cli`), so that the tool feels completely native to the macOS CLI ecosystem.

**Why this priority**: Essential for tool discovery, developer adoption, and professional polish.

**Independent Test**: Can be tested independently by running `ttzip-cli completion fish` and `ttzip-cli completion nushell` and validating syntax correctness.

**Acceptance Scenarios**:
1. **Given** a Fish shell environment, **When** generating completions with `ttzip-cli completion fish`, **Then** a valid Fish completion script is emitted.
2. **Given** a terminal user invoking `man ttzip-cli`, **Then** a well-formatted groff mdoc man page displays all commands, options, environment variables, exit codes, and usage examples.

---

### Edge Cases & Boundary Conditions

- **EC-001 (Piping to stdout on TTY)**: When `ttzip-cli cat` is invoked and the output is binary data sent directly to a TTY (not redirected to a file or pipe), the CLI MUST display a safety warning and require `--force-binary` or confirmation to prevent garbling the user's terminal.
- **EC-002 (Zip Slip & Path Traversal via Glob)**: When glob inclusions or extractions are specified, any path containing `../` or absolute leading slashes MUST be securely sanitized or intercepted before disk I/O according to `ARCHIVE_EXTRACT_SECURE_NODOTDOT`.
- **EC-003 (Empty Inclusions)**: If `--include "*.pdf"` is provided and zero matching files exist in the archive, the CLI MUST exit with code `1` (No match) and an informative message rather than creating an empty file.
- **EC-004 (Password Prompt in Non-Interactive Headless CI)**: If an encrypted archive requires a password in a non-TTY headless environment (e.g. GitHub Actions without stdin), the CLI MUST fail immediately with exit code `4` (Auth/Password error) and a diagnostic message rather than hanging indefinitely waiting for stdin.

---

## 3. Requirements *(mandatory)*

### 3.1 Functional Requirements

- **FR-001 [New Subcommands & Verbs]**:
  - `cat` / `view`: Decompress and stream a specified entry or set of entries directly to `stdout`.
  - `tree`: Render an ASCII/Unicode hierarchical tree representation of archive contents with depth limiting (`--depth <N>`).
  - `delete` / `d`: Remove specified files or glob patterns from supported archives (`zip`, `tar`, `7z`).
  - `update` / `u`: Update existing archive entries or append newer files matching modification timestamps.
  - `hash` / `checksum`: Calculate and display CRC32, MD5, SHA-1, SHA-256 digests for entries inside an archive without disk extraction.

- **FR-002 [Pattern Filtering & Glob Matchers]**:
  - `-x`, `--exclude <glob>`: Exclude files matching wildcards or regex patterns during creation or extraction.
  - `-i`, `--include <glob>`: Include only files matching wildcards during creation or extraction.
  - `--files-from <path>`: Read input file paths from a newline-separated manifest file.
  - `--strip-components <N>`: Strip `N` leading path elements from file paths on extraction (equivalent to `tar --strip-components`).
  - `-j`, `--junk-paths`, `--flatten`: Extract all files flat into the destination directory without creating subdirectories.

- **FR-003 [System & Metadata Presets]**:
  - `--exclude-vcs`: Automatically exclude `.git/`, `.svn/`, `.hg/`, `.gitignore`, `.gitattributes`.
  - `--no-mac-metadata`: Automatically omit `.DS_Store`, `__MACOSX/`, AppleDouble `._*` resource forks, and Spotlight metadata.
  - `--strip-quarantine`: Strip macOS `com.apple.quarantine` extended attributes upon extraction.
  - `--preserve-permissions`: Ensure exact POSIX permissions and mode bits are preserved across create and extract.

- **FR-004 [Process-Safe Credential Inputs]**:
  - TTY Non-Echo Input: Read password securely via `readpassphrase` / POSIX `termios` when encrypted archives are encountered without `-p`.
  - `--password-file <path>`: Read encryption/decryption key directly from a secured file path.
  - `TTZIP_PASSWORD` & `TTZIP_HEADER_PASSWORD`: Support environment variable fallback for CI/CD pipelines.
  - Sensitive Memory Eradication: All password buffers must be scrubbed using volatile memory wipe (`memset_s` / `secure_zero_memory`).

- **FR-005 [Granular Overwrite Policies]**:
  - `--overwrite <policy>`: Support policies `always` (default with `-y`), `never` (or `-n`), `newer` (freshen), `backup` (create `.bak`), `prompt` (interactive TTY).
  - Interactive Resolution: In interactive TTY mode, display file conflict dialog showing existing file size/mtime vs archive entry size/mtime with options `[y/n/A/N/b/d]`.

- **FR-006 [UNIX Paging & Terminal Ergonomics]**:
  - Auto-Pager: Pipe `list` and `tree` outputs through `$PAGER` (defaulting to `less -RFX`) when stdout is connected to a TTY and output exceeds terminal rows.
  - TTY Auto-Detection: Automatically disable ANSI color codes, spinners, and interactive prompts when stdout/stderr are redirected to non-TTY pipes or files.
  - Terminal Width Clamping: Truncate and ellipsis path strings intelligently to fit within current `stty size` terminal columns.

- **FR-007 [Universal Machine-Readable JSON Schemas]**:
  - `--json`: Emit formal JSON / NDJSON structures for all commands (`create`, `extract`, `list`, `tree`, `test`, `cat`, `hash`, `bench`).
  - Schema consistency: All JSON payloads must include `schema_version`, `status`, `exit_code`, and domain-specific telemetry.

- **FR-008 [Exit Code Taxonomy Standardization]**:
  - Standard POSIX Exit Codes:
    - `0`: Success
    - `1`: Warning / Minor error / No matching files
    - `2`: Fatal error / Corrupt archive
    - `3`: Checksum / Integrity validation failure
    - `4`: Authentication / Password error / Missing passphrase
    - `5`: Security threat / Zip Slip / Directory traversal blocked
    - `6`: File I/O / Disk space / Permission denied
    - `7`: Command line syntax / Unrecognized option error
    - `8`: Out of memory / Resource limit reached
    - `130`: Terminated by user (`SIGINT` / Ctrl+C)

- **FR-009 [Shell Completion Extensions]**:
  - Generate production auto-completions for Zsh (`_ttzip-cli`), Bash (`ttzip-cli.bash`), Fish (`ttzip-cli.fish`), and NuShell (`ttzip-cli.nu`).

---

### 3.2 Key Entities

- **CLIOptions**: Extended options structure containing pattern exclusions, inclusion globs, password sources, overwrite policies, component stripping, and formatting configurations.
- **ArchiveEntryMetadata**: Metadata model representing individual archive entries including path, compressed/uncompressed size, CRC32/SHA256 checksums, permissions, mtime, and encryption status.
- **CollisionPolicy**: Enum representing file overwrite behaviors (`always`, `never`, `newer`, `backup`, `prompt`).
- **TerminalRenderContext**: Context model tracking TTY status, window width/height, color capabilities, pager handle, and JSON streaming configuration.

---

## 4. Success Criteria *(mandatory)*

### 4.1 Measurable Outcomes

- **SC-001 (Command Parity Rate)**: 100% of standard archiving command verbs (`create`, `extract`, `list`, `test`, `cat`, `tree`, `delete`, `update`, `hash`) fully operational and tested.
- **SC-002 (Process Table Privacy)**: 0% plaintext password leakage in `ps aux` or environment process listings when using interactive prompt or `--password-file`.
- **SC-003 (Exclusion Precision)**: 100% exclusion fidelity for glob expressions, `--exclude-vcs`, and `--no-mac-metadata` across 16 supported formats.
- **SC-004 (Startup Latency)**: CLI cold launch latency (`ttzip-cli --version` / `--help`) remains under `8 ms` on Apple Silicon.
- **SC-005 (Schema Strictness)**: 100% of CLI `--json` outputs conform to strict JSON schemas without unvalidated bare payloads.
- **SC-006 (Zero Memory Leak & Bounds Safety)**: Zero memory safety violations or UAF issues under address sanitizer (`ASan`) across all CLI command flows.

---

## 5. Assumptions

1. **Target Operating System**: macOS 14.0+ (Sonoma, Sequoia) on Apple Silicon (ARM64) and Intel (x86_64).
2. **Swift & C Toolchain**: Swift 6.0 standard library and native POSIX/C11 static engines (`CTTZipBridge`).
3. **Distribution Format**: Standalone universal binary (`ttzip-cli`) and Homebrew formula (`ttzip.rb`).
4. **Pager Dependency**: If `less` or `$PAGER` is not installed or available on the host system, the CLI gracefully falls back to direct unpaged stdout.
