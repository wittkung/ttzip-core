# Implementation Plan: TTZip Model Context Protocol (MCP) Server and Agent Manual Ecosystem

**Branch**: `227-mcp-server-service` | **Date**: 2026-09-18 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `products/ttzip/core/specs/227-mcp-server-service/spec.md`

---

## Summary

Implement a native Model Context Protocol (MCP) server embedded in the TTZip CLI binary under the subcommand `ttzip mcp`. The server operates over standard input/output (`stdio`) using JSON-RPC 2.0, exposing 8 typed archive management tools (`ttzip_list`, `ttzip_extract`, `ttzip_create`, `ttzip_inspect`, `ttzip_cat`, `ttzip_verify`, `ttzip_convert`, `ttzip_repair`), 2 URI schemes for dynamic virtual archive inspection (`archive://{path}`) and static manuals (`ttzip://manuals/{topic}`), and 2 structured prompt templates. This removes the need for AI agents to generate fragile ad-hoc Bash/Python scripts for archive operations on macOS.

---

## Technical Context

- **Language/Version**: Rust 2021 (Rust 1.75+), Swift 6.0 (for app integration)
- **Primary Dependencies**:
  - `ttzip-engine` (internal microkernel: safe format decoders, encoders, memory-mapped I/O, SIMD CRC)
  - `clap 4.5` (CLI parsing and subcommand dispatch)
  - `serde 1.0` & `serde_json 1.0` (JSON-RPC 2.0 protocol serialization and schema validation)
- **Storage**: In-memory streaming and direct filesystem I/O; zero temporary database or cache files required
- **Testing**: `cargo test -p ttzip-tui`, unit tests for JSON-RPC dispatch, integration smoke tests with piped stdio
- **Target Platform**: macOS 14.0+ (Apple Silicon ARM64 NEON prioritized, Intel x86_64 compatible)
- **Project Type**: Standalone Systems CLI Utility with Embedded Protocol Server
- **Performance Goals**:
  - MCP initialization handshake < 10ms
  - Archive listing and file inspection with 0 temporary disk writes
  - Peak memory footprint $\le 64\text{MB}$ during stdio protocol serving
- **Constraints**:
  - Strict stdio hygiene: stdout strictly reserved for valid JSON-RPC 2.0 frames; all diagnostics redirected to stderr
  - Full path containment: zero directory traversal (Zip Slip immunity) on extraction
  - Single-file code limit $\le 800$ LOC (target $\le 350$ LOC) per architectural invariant
- **Scale/Scope**: 8 MCP tools, 5 static manual resources, 1 dynamic virtual filesystem URI scheme, 2 prompt templates

---

## Constitution Check

*GATE: Evaluation against TTZip Constitution Level 0 & Systemic Invariants*

| Constitutional Invariant | Status | Verification Mechanism |
| :--- | :--- | :--- |
| **I. Stream-First (流式第一性)** | **PASS** | `ttzip_cat` and `ttzip_list` utilize `MmapSource` without full-heap buffering; buffer memory bounded to $\le 64\text{MB}$. |
| **II. Invariant-First (纵深防御)** | **PASS** | Path canonicalization and sandbox prefix enforcement prevent Zip Slip path traversal before any disk writes occur. |
| **III. Bounds-First (确定性确界)** | **PASS** | Tool parameters constrained via JSON Schema (limits on thread count, buffer size, page limits); sensitive passwords zeroized upon completion. |
| **IV. Oracle-First (真实预言机)** | **PASS** | Protocol frames verified against standard JSON-RPC 2.0 oracle tests; archive operations cross-validated against native engine unit suites. |
| **Zero-Warning Gate** | **PASS** | Crate compiles under `cargo clippy -- -D warnings` with zero warnings. |
| **800 LOC Limit** | **PASS** | MCP module decomposed into modular components: `mcp/protocol.rs`, `mcp/dispatcher.rs`, `mcp/tools.rs`, `mcp/resources.rs`, `mcp/prompts.rs`. |

---

## Project Structure

### Documentation & Specifications (this feature)

```text
products/ttzip/core/specs/227-mcp-server-service/
├── spec.md                       # Feature specification (specify output)
├── checklists/
│   └── requirements.md           # Spec quality checklist (specify output)
├── research.md                   # Phase 0 technical research (plan output)
├── data-model.md                 # Phase 1 data model (plan output)
├── contracts/                    # Phase 1 interface contracts (plan output)
│   ├── mcp_tools_schema.json     # JSON Schema for all 8 MCP tools
│   └── mcp_protocol_contract.json# JSON-RPC 2.0 protocol contract
├── quickstart.md                 # Phase 1 validation scenarios (plan output)
├── plan.md                       # This implementation plan
└── tasks.md                      # Phase 2 task checklist (tasks output)
```

### Source Code Modifications

```text
products/ttzip/core/rust/ttzip-tui/
├── Cargo.toml                    # Verify serde/serde_json feature flags
└── src/
    ├── cli/
    │   ├── args.rs               # Add `Mcp` variant to `Commands` enum in Clap
    │   ├── handlers/
    │   │   ├── mod.rs            # Export `mcp` module
    │   │   └── mcp/              # New modular MCP subsystem
    │   │       ├── mod.rs        # Subcommand entrypoint & stdio loop
    │   │       ├── protocol.rs   # JSON-RPC 2.0 request/response/error types
    │   │       ├── tools.rs      # Tool registrations and handlers dispatch
    │   │       ├── resources.rs  # Manuals and archive:// URI provider
    │   │       ├── prompts.rs    # Extraction & compression guidance prompts
    │   │       └── tests.rs      # Unit & protocol serialization tests
    └── main.rs                   # Route `Commands::Mcp` to `handlers::mcp::execute_mcp`
```

---

## Implementation Details

### Module Responsibilities

1. **`args.rs` Update**:
   Add `Commands::Mcp` variant with optional `--log-level` flag to configure diagnostic stderr verbosity.
2. **`mcp/protocol.rs`**:
   Implements typed JSON-RPC 2.0 envelopes (`JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcError`) conforming to `contracts/mcp_protocol_contract.json`.
3. **`mcp/tools.rs`**:
   Exposes the 8 tool schemas defined in `contracts/mcp_tools_schema.json`. Dispatches calls to existing functions in `crate::cli::handlers::{list, extract, create, info, cat, check, convert, repair}`.
4. **`mcp/resources.rs`**:
   Provides static embedded markdown strings for `ttzip://manuals/*` and parses `archive://{path}` to return formatted virtual filesystem trees.
5. **`mcp/prompts.rs`**:
   Implements `ttzip_plan_extraction` and `ttzip_plan_compression` prompt expansions.
6. **`mcp/mod.rs`**:
   Runs the main stdio event loop: reads line-delimited JSON-RPC from `stdin.lock()`, processes request via dispatcher, writes serialized response followed by newline to `stdout.lock()`, and flushes.

---

## Verification Plan

### Automated Tests
1. **Unit Tests**:
   ```bash
   cargo test -p ttzip-tui --lib cli::handlers::mcp
   ```
2. **Integration & Smoke Pipeline**:
   ```bash
   # Run automated pipe verification
   printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}' \
   | cargo run -p ttzip-tui -- mcp
   ```
3. **Quality & Architecture Gates**:
   ```bash
   cargo clippy -p ttzip-tui -- -D warnings
   products/ttzip/core/scripts/lint_loc_gate.sh
   ```
