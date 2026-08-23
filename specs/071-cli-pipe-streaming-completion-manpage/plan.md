# Implementation Plan: Feature 071 — CLI UNIX Pipe Streaming, Shell Auto-Completion, BSD Man Page, and Local CI/CD Gate

**Feature Branch**: `071-cli-pipe-streaming-completion-manpage`  
**Created**: 2026-08-17  
**Status**: Ready for Tasks  
**Spec Document**: [`spec.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/071-cli-pipe-streaming-completion-manpage/spec.md)

---

## 1. Technical Context & Baseline

### Architecture Boundaries
- **In-Process C Bridge (`CTTZipBridge`)**: POSIX descriptor streaming (`archive_write_open_fd`, `archive_read_open_fd`), signal handling (`SIGPIPE` -> `SIG_IGN`), and binary buffer probing (`ttzip_is_buffer_binary`).
- **Core Engine (`TTZipCore`)**: Declarative shell completion generator (`ShellCompletionGenerator`), BSD mdoc troff generator (`ManPageGenerator`), stream pipeline manager (`StreamPipeAdapter`), and exit code mappings (`CLIExitCode.sigpipe = 141`).
- **CLI Subcommand Router (`TTZipCLI`)**: Wire `-o -`, `-i -`, `-O -`, `completion [zsh|bash|fish]`, `man`, and `test` commands.
- **Local CI Automation (`scripts/`)**: `scripts/run_local_ci_gate.sh` driving the 6-stage regression gate.

---

## 2. Constitution & Invariant Check

| Principle / Invariant | Compliance Status | Rationale / Enforcement |
| :--- | :--- | :--- |
| **Hot-Path Zero-Cost Abstraction** | PASS | Sequential pipe streaming operates directly via 64KB POSIX descriptor chunks without heap accumulation. |
| **100% In-Process C Static Bindings** | PASS | Zero subprocess spawning for core archiving and extraction pipelines. |
| **Hard Performance Floors** | PASS | Local CI gate enforces all 13 compression/decompression floors before passing. |
| **Stream-First & Bounds-First** | PASS | $O(1)$ stream memory footprint; zero unbounded `Data(count:)` buffers on streaming paths. |

---

## 3. Phase 0: Research Catalog

- [x] **R001** [SUBAGENT:research] 《UNIX Pipe Streaming & Standard I/O Architecture》: `archive_write_open_fd(a, STDOUT_FILENO)`, `archive_read_open_fd(a, STDIN_FILENO)`, `isatty` binary protection, `SIGPIPE` mask + status 141.
- [x] **R002** [SUBAGENT:research] 《Dynamic Shell Auto-Completion Generation Architecture》: Declarative `ShellCompletionGenerator` for Zsh, Bash, Fish derived from `CLICommandSpec`.
- [x] **R003** [SUBAGENT:research] 《BSD Man Page (mdoc) Generator Architecture》: 12-section BSD `mdoc(7)` troff generator with zero `mandoc -Tlint` warnings.
- [x] **R004** [SUBAGENT:research] 《Local CI/CD Automated Test Gate & Regression Pipeline》: 6-stage sequential test gate script with ANSI summary scorecard.

---

## 4. Phase 1: Design Artifacts & Contracts

- **Data Model**: [`data-model.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/071-cli-pipe-streaming-completion-manpage/data-model.md)
- **Contracts**:
  - `contracts/stream_pipeline_config.json`
  - `contracts/shell_completion_request.json`
  - `contracts/local_ci_gate_report.json`
- **Quickstart**: [`quickstart.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/071-cli-pipe-streaming-completion-manpage/quickstart.md)

---

## 5. Component Modification & Creation Plan

### 5.1 C Bridge (`Sources/CTTZipBridge/`)
- `CTTZipDiagnostics.c` & `CTTZipDiagnostics.h`: Add `ttzip_install_signal_handlers()` installing `signal(SIGPIPE, SIG_IGN)`.
- `ttzip_tar_native.c`: Support `output_path == "-"` via `archive_write_open_fd(a, STDOUT_FILENO)`.
- `ttzip_tar_zstd_direct.c`: Support `output_path == "-"` with stdout writes.
- `CTTZipBridge_GzParallel.c`: Protect `ctx->fd == STDOUT_FILENO` against closure.

### 5.2 Core CLI Models & Generators (`Sources/TTZipCore/CLI/`)
- `CLIExitCode.swift`: Add `public static let sigpipe: Int32 = 141`.
- `CLICommandSpec.swift`: Add `valueChoices` and `isFilePath` properties to `CLIOptionSpec`.
- `ShellCompletionGenerator.swift` [NEW]: Dynamic Zsh, Bash, Fish, Nushell script renderer.
- `ManPageGenerator.swift` [NEW]: BSD `mdoc(7)` troff manual page generator.
- `StreamPipeAdapter.swift`: Enhance stdout streaming detection, stderr progress redirection, and stdin spooling.
- `TerminalRenderEngine.swift`: Add dynamic progress stream routing to `stderr` during stdout streaming.

### 5.3 CLI Subcommand Handlers (`Sources/TTZipCLI/`)
- `CLICommandRouter.swift`:
  - Route `-o -` / `--output -` in `archive` to stdout stream.
  - Route `-i -` / `--input -` in `extract` to stdin stream.
  - Route `completion` subcommand to `ShellCompletionGenerator`.
  - Route `man` subcommand to `ManPageGenerator`.

### 5.4 Test Suites & CI Scripts (`Tests/TTZipTests/` & `scripts/`)
- `PipeStreamingTests.swift` [NEW]: Unit & E2E tests for stdout stream create, stdin stream extract, `SIGPIPE` exit 141, and TTY binary suppression.
- `ShellCompletionTests.swift` [NEW]: Unit tests asserting all subcommands and format choices are generated across Zsh/Bash/Fish.
- `ManPageGeneratorTests.swift` [NEW]: Unit tests asserting 12 sections and valid mdoc syntax.
- `scripts/run_local_ci_gate.sh` [NEW]: Executable 6-stage regression gate runner.
