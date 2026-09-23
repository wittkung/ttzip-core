# Phase 0 Technical Research: TTZip MCP Server Architecture

**Feature**: `227-mcp-server-service`  
**Date**: 2026-09-18  
**Status**: Completed

---

## 1. Transport Protocol Selection

### Decision
Implement the Model Context Protocol (MCP) using standard input/output (`stdio`) transport under the `ttzip mcp` CLI subcommand.

### Rationale
- **Zero Configuration**: AI agent orchestrators (Claude Desktop, Antigravity, Cursor, Zed) natively launch local MCP servers as subprocesses communicating over standard streams.
- **Security & Sandboxing**: Avoids opening local network ports, eliminating firewall consent dialogs, port collisions, and unauthorized local socket access on macOS.
- **Instant Lifecycle Binding**: The MCP server lifecycle is coupled to the agent parent process; termination of the agent automatically cleans up the server subprocess without orphan daemons.

### Alternatives Considered
- **HTTP / Server-Sent Events (SSE)**: Requires managing port allocation, HTTP headers, authentication tokens, and daemon management. Rejected as unnecessarily complex for local CLI tooling.
- **Unix Domain Sockets (UDS)**: Requires socket file path management and cleanup on abnormal exits. Not uniformly supported across all standard desktop MCP clients.

---

## 2. Server Implementation Language & Runtime Strategy

### Decision
Embed the MCP server directly within the existing `ttzip-tui` Rust binary, sharing existing CLI argument parsing (`clap`) and internal business logic handlers (`crate::cli::handlers`).

### Rationale
- **Zero Runtime Dependencies**: No Node.js, Python, or external package managers required on the user's macOS host.
- **Microsecond Cold Start**: A native compiled Rust binary starts in under 5ms, avoiding the 200-500ms startup delay typical of Node.js/Python runtimes.
- **Zero-Copy Memory-Mapped Engine Integration**: Directly leverages `ttzip_engine::archive::source::mmap::MmapSource` and unified format parsers for ultra-high-throughput inspection.
- **Shared Code Contracts**: Directly reuses existing data structures (`VfsNodeContractDto`, `VfsTreeContractDto`, `ArchiveDescriptor`).

### Alternatives Considered
- **Standalone Node.js / TypeScript Wrapper**: Wrapping the CLI with `@modelcontextprotocol/sdk`. Rejected due to node runtime dependency, npm version drift, and subprocess serialization overhead.
- **Standalone Python FastMCP Wrapper**: Rejected due to Python environment fragmentation (`pyenv`, `conda`, system Python) and performance overhead.

---

## 3. Toolset & Schema Contract Design

### Decision
Expose 8 atomic, typed MCP tools with rigid JSON Schema contracts:
1. `ttzip_list`: Query archive contents with glob pattern filtering (`include`, `exclude`), pagination, and directory tree formatting.
2. `ttzip_extract`: Safe extraction to target directory with thread control, password decryption, and dry-run preview.
3. `ttzip_create`: Create archives with format selection (`zip`, `7z`, `tar`, `brotli`, `snappy`), compression level (0-12), volume chunking, and encryption.
4. `ttzip_inspect`: Comprehensive header inspection, compression ratio analysis, and format detection.
5. `ttzip_cat`: Direct in-memory extraction of a single entry stream to context without writing to disk.
6. `ttzip_verify`: Deep CRC/checksum and decompression integrity verification.
7. `ttzip_convert`: Transcode archives between container formats and compression levels.
8. `ttzip_repair`: Non-destructive salvage and reconstruction of corrupted ZIP/TAR headers.

### Rationale
Matches the complete headless CLI capabilities already implemented in `ttzip-tui/src/cli/handlers/`, converting CLI flags into self-documenting JSON Schema properties.

---

## 4. MCP Resources & Agent Manual Ecosystem

### Decision
Provide two distinct MCP resource schemes:
1. `ttzip://manuals/{topic}`: Embedded markdown documentation providing authoritative knowledge grounding for AI agents:
   - `ttzip://manuals/overview`: General architecture, capability boundaries, and CLI command parity.
   - `ttzip://manuals/format-matrix`: Technical specifications, compression ratios, and speed tradeoffs for ZIP, 7z, TAR, Snappy, and Brotli.
   - `ttzip://manuals/safe-extraction`: Best practices for sandboxed extraction, avoiding directory pollution (tar-bombs), and handling symlinks.
   - `ttzip://manuals/troubleshooting`: Diagnostics for damaged archives, wrong passwords, and multi-volume sets.
2. `archive://{path}`: Dynamic virtual directory resource allowing agents to read the virtual filesystem of any archive on disk as a readable text/markdown representation.

### Rationale
Fulfills the core user requirement: agents can read documentation directly via MCP resources before issuing commands, preventing hallucination of invalid flags or formats.

---

## 5. Security & Defensive Invariants (Zip Slip & TOCTOU Immunity)

### Decision
Enforce strict path confinement inside the MCP extraction handler:
- Canonicalize the destination directory path before extraction.
- For each entry, compute the intended output path and verify that `output_path.starts_with(&canonical_dest)`.
- Explicitly reject path segments containing `..` or absolute path prefixes.
- Follow TTZip Constitution Level 0 (Invariant-First & Bounds-First): safe symlink handling and delay fixup for directory permissions.

### Rationale
Agents frequently process untrusted archives from web downloads. Strict sandboxing ensures zero host filesystem compromise.

---

## 6. Stdio Stream Hygiene

### Decision
Enforce absolute separation between stdout and stderr:
- **stdout**: Dedicated exclusively to newline-delimited JSON-RPC 2.0 messages.
- **stderr**: Used for all diagnostic logging, warnings, and internal debug tracing.
- Any direct `print!` / `println!` calls inside business handlers are redirected or guarded when operating in MCP mode.

### Rationale
Extraneous characters written to stdout will invalidate JSON-RPC framing, crashing the MCP client parser.
