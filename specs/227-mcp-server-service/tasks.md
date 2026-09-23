# Tasks: TTZip Model Context Protocol (MCP) Server and Agent Manual Ecosystem

**Input**: Design documents from `products/ttzip/core/specs/227-mcp-server-service/`
**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/](contracts/)
**Organization**: Tasks are grouped by user story to enable independent implementation and testing.

---

## Format: `- [ ] [TaskID] [P?] [Story?] Description with file path`

- **[P]**: Parallelizable (touches independent files or isolated modules)
- **[Story]**: Associated User Story (`[US1]`, `[US2]`, `[US3]`)
- Every task specifies exact file paths and verifiable constraints.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Register MCP CLI subcommand, configure module structure, and establish core imports.

- [x] T001 Register `Mcp` subcommand variant with doc comments in `products/ttzip/core/rust/ttzip-tui/src/cli/args.rs`
- [x] T002 Create MCP handler module directory and declare public exports in `products/ttzip/core/rust/ttzip-tui/src/cli/handlers/mod.rs` and `products/ttzip/core/rust/ttzip-tui/src/cli/handlers/mcp/mod.rs`
- [x] T003 Wire `Commands::Mcp` routing in `products/ttzip/core/rust/ttzip-tui/src/main.rs` to call `handlers::mcp::execute_mcp`

---

## Phase 2: Foundational (Blocking Prerequisites - JSON-RPC 2.0 & Stdio Protocol Loop)

**Purpose**: Core protocol framing, JSON-RPC 2.0 serialization/deserialization, and stdio event loop.

- [x] T004 [P] Implement JSON-RPC 2.0 protocol structures (`JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcError`, `JsonRpcNotification`) and standard error constants (`PARSE_ERROR: -32700`, `INVALID_REQUEST: -32600`, `METHOD_NOT_FOUND: -32601`, `INVALID_PARAMS: -32602`, `INTERNAL_ERROR: -32603`) in `products/ttzip/core/rust/ttzip-tui/src/cli/handlers/mcp/protocol.rs`
- [x] T005 [P] Implement MCP initialization handshake (`initialize`, `notifications/initialized`, `ping`) returning server info (`ttzip-mcp`, version `1.0.0`, protocol `2024-11-05`) in `products/ttzip/core/rust/ttzip-tui/src/cli/handlers/mcp/protocol.rs`
- [x] T006 Implement line-delimited stdio server event loop with strict stdout/stderr isolation (all logs to stderr, stdout purely JSON-RPC) in `products/ttzip/core/rust/ttzip-tui/src/cli/handlers/mcp/mod.rs`

---

## Phase 3: User Story 1 - Zero-Scripting Direct Archive Operations (Priority: P1) 🎯 MVP

**Goal**: Enable AI agents to perform archive creation, extraction, and listing via native MCP tool calls without generating temporary scripts.

**Independent Test**: Execute `ttzip_create` on test directory, verify archive creation, then call `ttzip_list` and `ttzip_extract` into an isolated destination directory via stdio JSON-RPC without spawning shell commands.

- [x] T007 [P] [US1] Define JSON Schema tool definitions and input argument DTOs for `ttzip_list`, `ttzip_extract`, and `ttzip_create` in `products/ttzip/core/rust/ttzip-tui/src/cli/handlers/mcp/tools.rs`
- [x] T008 [US1] Implement path canonicalization and Zip Slip path traversal defense (`output_path.starts_with(&canonical_dest)`) for agent extractions in `products/ttzip/core/rust/ttzip-tui/src/cli/handlers/mcp/tools.rs`
- [x] T009 [US1] Implement execution handlers for `ttzip_list`, `ttzip_extract`, and `ttzip_create` connecting to existing `crate::cli::handlers` in `products/ttzip/core/rust/ttzip-tui/src/cli/handlers/mcp/tools.rs`
- [x] T010 [P] [US1] Add unit tests for `ttzip_list`, `ttzip_extract`, and `ttzip_create` JSON-RPC dispatch in `products/ttzip/core/rust/ttzip-tui/src/cli/handlers/mcp/tests.rs`

---

## Phase 4: User Story 2 - In-Memory Archive Inspection & Virtual Traversal (Priority: P2)

**Goal**: Allow AI agents to inspect archive metadata, view single files into context, and verify integrity with zero temporary disk writes.

**Independent Test**: Call `ttzip_inspect`, `ttzip_cat`, and read `archive://{path}` resource against multi-file archive, verifying returned file contents and confirming 0 temporary disk writes on the host system.

- [x] T011 [P] [US2] Implement tool schema and handler for `ttzip_inspect` and `ttzip_verify` in `products/ttzip/core/rust/ttzip-tui/src/cli/handlers/mcp/tools.rs`
- [x] T012 [P] [US2] Implement tool schema and handler for `ttzip_cat` with streaming decompression and `max_bytes` boundary limit (default 1MB, max 10MB) in `products/ttzip/core/rust/ttzip-tui/src/cli/handlers/mcp/tools.rs`
- [x] T013 [US2] Implement dynamic `archive://{path}` virtual directory URI provider returning markdown/JSON directory tree in `products/ttzip/core/rust/ttzip-tui/src/cli/handlers/mcp/resources.rs`
- [x] T014 [P] [US2] Add unit tests for `ttzip_inspect`, `ttzip_cat`, and `archive://` resource resolution in `products/ttzip/core/rust/ttzip-tui/src/cli/handlers/mcp/tests.rs`

---

## Phase 5: User Story 3 - Agent Knowledge Grounding & Decision Prompts (Priority: P3)

**Goal**: Provide AI agents with embedded technical manuals, transcoding/repair tools, and structured prompts for defensive extraction and format routing.

**Independent Test**: Query `resources/list` and `resources/read` for `ttzip://manuals/format-matrix`, and execute `prompts/get` for `ttzip_plan_extraction`, verifying markdown guides and reasoning scaffolds.

- [x] T015 [P] [US3] Implement tool schemas and handlers for `ttzip_convert` and `ttzip_repair` in `products/ttzip/core/rust/ttzip-tui/src/cli/handlers/mcp/tools.rs`
- [x] T016 [P] [US3] Embed static markdown manuals under `ttzip://manuals/{topic}` (`overview`, `format-matrix`, `safe-extraction`, `troubleshooting`) in `products/ttzip/core/rust/ttzip-tui/src/cli/handlers/mcp/resources.rs`
- [x] T017 [P] [US3] Implement prompt templates `ttzip_plan_extraction` and `ttzip_plan_compression` in `products/ttzip/core/rust/ttzip-tui/src/cli/handlers/mcp/prompts.rs`
- [x] T018 [P] [US3] Add unit tests for `ttzip_convert`, `ttzip_repair`, resources, and prompts in `products/ttzip/core/rust/ttzip-tui/src/cli/handlers/mcp/tests.rs`

---

## Phase 6: Polish & Quality Gates

**Purpose**: End-to-end integration tests, clippy validation, and architectural compliance.

- [x] T019 Add integration smoke test verifying stdio JSON-RPC handshake and multi-tool pipeline in `products/ttzip/core/rust/ttzip-tui/tests/mcp_smoke_test.rs`
- [x] T020 Run `cargo test -p ttzip-tui` and ensure 100% test pass rate across all unit and integration targets
- [x] T021 Run `cargo clippy -p ttzip-tui -- -D warnings` and confirm 0 errors 0 warnings
- [x] T022 Execute `products/ttzip/core/scripts/lint_loc_gate.sh` and ensure all files satisfy $\le 800$ LOC invariant

---

## Dependencies & Execution Order

```
[Phase 1: Setup (T001-T003)]
            │
            ▼
[Phase 2: Foundational JSON-RPC (T004-T006)]
            │
            ├────────────────────────────────────────┐
            ▼                                        ▼
[Phase 3: User Story 1 (T007-T010)]        [Phase 4: User Story 2 (T011-T014)]
  (MVP: list, extract, create)               (inspect, cat, verify, archive://)
            │                                        │
            └────────────────────┬───────────────────┘
                                 ▼
                    [Phase 5: User Story 3 (T015-T018)]
                      (convert, repair, manuals, prompts)
                                 │
                                 ▼
                    [Phase 6: Polish & Gates (T019-T022)]
```

---

## Parallel Execution Opportunities

- **T004 & T005**: Protocol definitions and initialization handlers can be developed in parallel with CLI argument parsing (T001).
- **T007, T011, T015**: Tool schema declarations in `tools.rs` can be drafted concurrently across stories.
- **T016 & T017**: Static documentation resources in `resources.rs` and prompt templates in `prompts.rs` can be implemented in parallel with tool handlers.
