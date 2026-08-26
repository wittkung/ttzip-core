# Feature Specification: CLI UNIX Pipe Streaming, Shell Auto-Completion, BSD Man Page, and Local CI/CD Test Gate

**Feature Branch**: `071-cli-pipe-streaming-completion-manpage`  
**Created**: 2026-08-17  
**Status**: Draft  
**Input**: Comprehensive CLI modernization covering UNIX Pipe & Standard I/O streaming (`stdin`/`stdout`/`-O`/`-c`/`-`), dynamic Shell completion generation (Zsh/Bash/Fish), BSD mdoc Man Page generation (`man1/ttzip-cli.1`), and local CI/CD automated test gate harness (`/speckit-specify`).

---

## 1. Executive Summary & Problem Statement

Building upon the standards compliance matrix and differential testing infrastructure of Feature 070, Feature 071 delivers the remaining core capabilities required for `ttzip-cli` to operate as a first-class citizen in professional POSIX and macOS developer workflows:

1. **UNIX Pipe & Standard I/O Streaming**: Enabling zero-disk-footprint piping (`cat input.tar | ttzip-cli create -f zst -o - > archive.tar.zst` and `cat archive.zip | ttzip-cli extract -i - -d dest/`), single-entry stdout extraction (`ttzip-cli extract -O - <entry> <archive>`), and safe `SIGPIPE` handling.
2. **Shell Auto-Completion System**: Dynamic generation of compliant Zsh, Bash, and Fish tab-completion scripts directly synchronized with `CLICommandSpec` to guarantee zero maintenance drift.
3. **BSD Man Page Manual Generation**: Native BSD mdoc troff generator outputting `ttzip-cli(1)` and `ttzip(1)` man pages complete with synopsis, flags, format matrix, and POSIX examples.
4. **Local CI/CD Automated Test Gate**: A robust local regression test harness (`scripts/run_local_ci_gate.sh`) integrating build verification, standards checks, differential oracle validation, stream mutation fuzzing, performance throughput floors, and pipe streaming roundtrips into a unified 1-command gate.

---

## 2. Clarifications

### Session 2026-08-17
- Q: How will progress bars and interactive terminal output behave when stdout is redirected to a pipe or file? → A: Automatic TTY detection (`isatty(STDOUT_FILENO)`) will redirect progress meters and status badges to `stderr` or suppress them in streaming mode, ensuring binary stdout streams are 100% clean and uncorrupted.
- Q: How will streaming extraction handle archive formats that require seeking (e.g. ZIP Central Directory at EOF)? → A: For stream-friendly formats (TAR, TAR.GZ, TAR.ZST, GZ, ZST, LZ4), direct single-pass streaming is used. For non-seekable stdin with ZIP/7Z formats that mandate central header reading, an in-memory or ephemeral page-aligned temporary buffer spooler is used transparently without leaking resources.
- Q: How will shell completion scripts stay synchronized with newly added CLI flags? → A: Completion scripts are dynamically derived at runtime from `CLICommandSpec.allSpecs`, reflecting options, abbreviations, value hints, and format choices programmatically.
- Q: What constitutes a passing local CI/CD gate? → A: 100% success across: (1) Release build compilation, (2) 16-format standards compliance tests, (3) differential oracle tests against system `tar`/`unzip`/`7zz`, (4) malformed stream fuzzing suites, (5) 13 performance throughput floors, and (6) stdout/stdin pipe round-trip integrity assertions.

---

## 3. User Scenarios & Personas

### Persona: DevOps & CI/CD Engineer (Alex)
- **Goal**: Seamlessly compress and decompress artifacts in build pipelines and shell scripts using standard UNIX pipes without writing intermediate multi-gigabyte files to disk.
- **Workflow**: Runs `tar -cf - build/ | ttzip-cli create -f zst -o - | curl -T - https://artifacts.internal/build.tar.zst` and verifies that progress logs do not corrupt the uploaded binary payload.

### Persona: Command Line Power User (Elena)
- **Goal**: Fast, frictionless command exploration with responsive tab completion and instant terminal man page access.
- **Workflow**: Installs Zsh completions via `eval "$(ttzip-cli completion zsh)"`, presses `Tab` after `ttzip-cli create -f ` to see all 16 formats, and reads `ttzip-cli man | mandoc -a` for quick option lookups.

### Persona: Core Engine Maintainer (Witt)
- **Goal**: Guarantee zero functional or performance regression before pushing code or releasing tags.
- **Workflow**: Runs `./scripts/run_local_ci_gate.sh` to execute the full test battery (build, standards, differential oracles, fuzzing, performance floors, pipe tests) in under 60 seconds with clear ANSI badges and non-zero exit code on failure.

---

## 4. Functional Requirements

### 4.1 UNIX Pipe & Standard I/O Streaming (US1)
- **FR-001**: `ttzip-cli create` MUST accept `-o -` or `--output -` to stream the resulting compressed archive directly to `stdout`.
- **FR-002**: `ttzip-cli extract` MUST accept `-i -`, `--input -`, or `-` as the archive path argument to consume input from `stdin`.
- **FR-003**: `ttzip-cli extract` MUST support `-O -`, `--to-stdout`, or `-c` to extract specified entry contents directly to `stdout` without creating files on disk.
- **FR-004**: `ttzip-cli inspect` MUST accept `-i -`, `--input -`, or `-` to read and display archive entry metadata from `stdin`.
- **FR-005**: All diagnostic logs, progress bars, and banners MUST automatically redirect to `stderr` whenever `stdout` is not a TTY (`!isatty(STDOUT_FILENO)`) or when writing an archive to `stdout`.
- **FR-006**: CLI MUST safely catch `SIGPIPE` and exit with status 141 (128 + 13) without crashing or emitting stack traces when downstream pipe consumers terminate early.

### 4.2 Shell Auto-Completion Generation System (US2)
- **FR-007**: `ttzip-cli completion zsh` MUST generate a fully valid `_ttzip-cli` Zsh completion script with descriptions, flag groups, format completions, and file path completions.
- **FR-008**: `ttzip-cli completion bash` MUST generate a POSIX-compliant Bash completion script registering `complete -F _ttzip_cli_completions ttzip-cli ttzip`.
- **FR-009**: `ttzip-cli completion fish` MUST generate a compliant Fish shell completion script registering `complete -c ttzip-cli ...` and `complete -c ttzip ...`.
- **FR-010**: All completion generators MUST derive their arguments, descriptions, and value choices dynamically from `CLICommandSpec.allSpecs` to eliminate drift.

### 4.3 BSD Man Page Generation & Documentation (US3)
- **FR-011**: `ttzip-cli man` MUST generate standard BSD mdoc troff formatted manual source for `ttzip-cli(1)`.
- **FR-012**: Generated man page MUST include sections: `.Dd`, `.Dt`, `.Os`, `.Sh NAME`, `.Sh SYNOPSIS`, `.Sh DESCRIPTION`, `.Sh COMMANDS`, `.Sh OPTIONS`, `.Sh SUPPORTED FORMATS`, `.Sh EXAMPLES`, `.Sh EXIT STATUS`, `.Sh STANDARDS`, and `.Sh AUTHORS`.
- **FR-013**: Generated man page MUST be 100% compliant with `mandoc -Tlint` without syntax warnings or errors.

### 4.4 Local CI/CD Test Gate & Regression Harness (US4)
- **FR-014**: Provide `./scripts/run_local_ci_gate.sh` executing 6 gate stages sequentially:
  1. Build validation (`swift build -c release`).
  2. Standards compliance validation (`ttzip-cli test --standard all`).
  3. Differential oracle validation (`ttzip-cli test --differential all`).
  4. Malformed stream fuzzing validation (`ttzip-cli test --fuzz`).
  5. Performance floor validation (`swift test --filter XCTestPerformanceMeasureTests`).
  6. Pipeline streaming E2E validation (verifying `stdin | ttzip-cli create | ttzip-cli extract | stdout` bit-exact hash parity).
- **FR-015**: Local CI gate MUST output ANSI summary status cards and return non-zero exit code on any failure.

---

## 5. Success Criteria & Non-Functional Requirements

### 5.1 Correctness & Robustness
- **SC-001**: 100% bit-exact SHA-256 payload parity when compressing data through stdout pipe and decompressing through stdin pipe.
- **SC-002**: Zero memory leaks or dangling file descriptors during continuous streaming operations.
- **SC-003**: Clean termination under `SIGPIPE` within 5ms.

### 5.2 Usability & Developer Experience
- **SC-004**: Tab completion response time < 5ms in Zsh, Bash, and Fish.
- **SC-005**: Man page renders cleanly via `man`, `mandoc`, and `groff`.
- **SC-006**: `./scripts/run_local_ci_gate.sh` completes execution of all 6 gates in under 60 seconds.

---

## 6. Out of Scope

- Remote GitHub Actions cloud billing or webhook management (local gate execution only per requirement).
- Interactive GUI shell wrappers inside AppKit (CLI and terminal workflows only).
