# Phase 0 Research Report: Feature 071 — CLI UNIX Pipe Streaming, Shell Completion, BSD Man Page, and Local CI/CD Gate

**Feature Directory**: `specs/071-cli-pipe-streaming-completion-manpage`  
**Research Date**: 2026-08-17  
**Status**: Completed & Verified

---

## R001: UNIX Pipe Streaming & Standard I/O Architecture

### Decision
1. **Stdout Streaming for Creation (`-o -`)**:
   - Stream-friendly sequential formats (TAR, TAR.GZ, TAR.ZST, TAR.BZ2, TAR.XZ, TAR.LZ4, BROTLI, SNAPPY) stream directly to `STDOUT_FILENO` via `archive_write_open_fd(a, STDOUT_FILENO)` in `ttzip_tar_native.c` and direct unbuffered writes in `ttzip_tar_zstd_direct.c`.
   - Seekable formats (ZIP, 7Z, ISO, DMG) reject `-o -` fast with `CLIExitCode.usage` (64) explaining that random-access seeking is required.
2. **Stdin Streaming for Extraction (`-i -`, `-`)**:
   - Sequential formats extract directly from `STDIN_FILENO` via `archive_read_open_fd(a, STDIN_FILENO, 65536)`.
   - Non-seekable stdin inputs for ZIP/7Z automatically spool to an ephemeral, anonymous temporary file via `StreamPipeAdapter.readStdinToTemporaryFileIfNeeded()` and clean up in `defer`.
3. **Single-Entry Stdout Cat (`ttzip-cli cat` / `extract -O -`)**:
   - Uses `ttzip_stream_archive_entries_to_fd(..., STDOUT_FILENO)`.
   - Checks `isatty(STDOUT_FILENO)` and probes the first 4KB with `ttzip_is_buffer_binary()`. If binary and `--force` is absent, aborts with error to prevent terminal garbling.
4. **Signal & Pipe Safety (`SIGPIPE`)**:
   - Masks `SIGPIPE` with `signal(SIGPIPE, SIG_IGN)` during initialization.
   - Translates POSIX `EPIPE` writes to `TTZIP_ERR_BROKEN_PIPE` (-141) and exits cleanly with `CLIExitCode.sigpipe` (141 = 128 + 13).
5. **Log & Telemetry Isolation**:
   - When stdout is used for data bytes (`-o -`, `cat`), `TerminalRenderEngine` redirects all progress bars, spinners, and diagnostics to `stderr`.
   - If `isatty(STDERR_FILENO) == 0` or `-q` is provided, progress rendering is completely disabled.

### Rationale
- Pure $O(1)$ streaming memory footprint without unbounded RAM accumulation.
- Strict isolation of binary payload on `stdout` prevents data corruption in UNIX pipes.
- Clean `141` exit status under `SIGPIPE` adheres to standard POSIX conventions (`cat`, `tar`, `head`).

### Alternatives Considered
- **Unbounded Memory Spooling (`archive_read_open_memory`)**: Rejected because piping multi-gigabyte archives would exhaust system RAM.
- **Dumping entire 7z/Zip to stdout post-compression**: Rejected because it breaks pipeline streaming invariants and incurs double I/O.

### Source
- `Sources/CTTZipBridge/include/CTTZipStreamCoder.h:L21-84`
- `Sources/CTTZipBridge/CTTZipStreamCoder.c:L134-269`
- `Sources/CTTZipBridge/CTTZipBridge_Archive.c:L269-382`
- `Sources/CTTZipBridge/ttzip_tar_native.c:L148-230`
- `Sources/CTTZipBridge/ttzip_tar_zstd_direct.c:L351-425`
- `Sources/TTZipCore/CLI/TerminalRenderEngine.swift:L23-131`
- `Sources/TTZipCore/CLI/StreamPipeAdapter.swift:L9-45`

---

## R002: Dynamic Shell Auto-Completion Generator Architecture

### Decision
1. Implement a declarative `ShellCompletionGenerator` in `Sources/TTZipCore/CLI/ShellCompletionGenerator.swift` supporting `Zsh`, `Bash`, `Fish`, and `Nushell`.
2. Enrich `CLIOptionSpec` with `valueChoices: [String]?` (format names, overwrite policies, language codes) and `isFilePath: Bool`.
3. Generator programmatically iterates over `CLICommandSpec.allSpecs` and `CLICommandSpec.globalOptions`:
   - **Zsh**: Renders `#compdef ttzip-cli ttzip`, `_arguments -C`, subcommand switching with `_describe -t subcommands`, format choices `:(zip 7z tar.zst ...)`, and archive file patterns `_files -g "*.zip *.7z *.tar* ..."`.
   - **Bash**: Renders `_ttzip_cli_completions()`, subcommand branching via `compgen -W`, format flag value auto-complete, and file fallback `compgen -f`.
   - **Fish**: Renders `complete -c ttzip-cli -f`, subcommands via `__fish_use_subcommand`, options via `__fish_seen_subcommand_from`, and selective file completion `-F`.

### Rationale
- **Zero-Drift Guarantee**: All metadata lives strictly in `CLICommandSpec`; any newly added option or command is automatically available across all shell completion scripts.
- **Zero Latency**: Statically rendered scripts eliminate runtime process invocation per Tab press.

### Alternatives Considered
- **Static hand-crafted scripts in repo**: Rejected due to high risk of documentation and completion drift.
- **Runtime keystroke callbacks (`ttzip-cli __complete ...`)**: Rejected due to 10-30ms process spawning latency per keystroke in interactive shells.

### Source
- `Sources/TTZipCore/CLI/CLICommandSpec.swift:L1-544`
- `Sources/TTZipCore/CLI/CLIOptions.swift:L1-294`
- `Sources/TTZipCLI/CLICommandRouter.swift:L234-246`
- `Tests/TTZipTests/CLIPOSIXStandardTests.swift:L143-189`

---

## R003: BSD Man Page (mdoc) Generator Architecture

### Decision
1. Implement a dedicated `ManPageGenerator` in `Sources/TTZipCore/CLI/ManPageGenerator.swift` utilizing the BSD `mdoc(7)` troff macro grammar.
2. Structure the manual into 12 canonical sections in order:
   `.Dd`, `.Dt TTZIP-CLI 1`, `.Os macOS`, `.Sh NAME`, `.Sh SYNOPSIS`, `.Sh DESCRIPTION`, `.Sh COMMANDS`, `.Sh OPTIONS`, `.Sh SUPPORTED FORMATS`, `.Sh EXAMPLES`, `.Sh ENVIRONMENT`, `.Sh EXIT STATUS`, `.Sh STANDARDS`, `.Sh SEE ALSO`, `.Sh AUTHORS`.
3. Strict `mandoc -Tlint` compliance:
   - Single list blocks (`.Bl -tag -width 18n` ... `.El`).
   - Macro options syntax: `.It Fl s Ar SIZE , Fl -split Ns = Ns Ar SIZE`.
   - Literal period escaping `\&.` to prevent unintended troff macro triggering.
   - Formats section dynamically populated from `ArchiveFormatStandardRegistry.shared.allSpecs()`.
   - Exit status populated from `CLIExitCode.allCases`.

### Rationale
- BSD `mdoc` is the native manual page standard for macOS and BSD systems, producing superior typography on modern terminal pagers compared to legacy `man(7)`.
- Eliminates external document compilation tools (e.g. `pandoc` / `ronn`), keeping TTZip 100% self-contained.

### Alternatives Considered
- **Legacy `man(7)` macros (`.TH`, `.SH`)**: Rejected because legacy macros lack semantic structure and produce inferior formatting on macOS `less`/`mandoc`.
- **Pre-rendered static man page file**: Rejected to avoid maintenance drift when options change.

### Source
- `Sources/TTZipCore/CLI/CLICommandSpec.swift:L486-542`
- `Sources/TTZipCore/Standards/ArchiveFormatStandardSpec.swift:L240-320`
- `Sources/TTZipCore/CLI/CLIExitCode.swift:L1-40`
- BSD `mdoc(7)` & `mandoc(1)` specification.

---

## R004: Local CI/CD Automated Test Gate & Regression Pipeline

### Decision
1. Implement `scripts/run_local_ci_gate.sh` executing a 6-stage sequential test gate:
   - **Stage 1**: Build release binary (`swift build -c release --product ttzip-cli`).
   - **Stage 2**: Standards compliance validation (`ttzip-cli test --standard all`).
   - **Stage 3**: Differential oracle validation (`ttzip-cli test --differential all`).
   - **Stage 4**: Malformed stream fuzzing gate (`ttzip-cli test --fuzz`).
   - **Stage 5**: 13 hardware throughput floors (`swift test --filter XCTestPerformanceMeasureTests`).
   - **Stage 6**: Pipeline stream E2E test (roundtrip `stdin -> create -> extract -> stdout` SHA-256 hash identity).
2. Format output with high-contrast ANSI scorecard badges (`PASS`, `FAIL`, `SKIP`, `GATE`).
3. Return non-zero exit code on any gate failure.

### Rationale
- Provides a fast (< 60s), reproducible local verification gate before code check-in, preventing broken commits.
- Validates both internal Swift/C unit tests and external binary CLI pipe behavior under release optimization.

### Alternatives Considered
- **Relying solely on `swift test`**: Rejected because `swift test` cannot easily test end-to-end binary CLI subprocess piping, `stdin` redirection, and stdout binary stream hashes under release optimization.

### Source
- `scripts/run_all_tests.sh:L1-50`
- `Sources/TTZipCLI/TestCommand.swift:L1-467`
- `Tests/TTZipTests/XCTestPerformanceMeasureTests.swift:L1-60`
