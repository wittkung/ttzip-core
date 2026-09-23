# Feature Specification: TTZip Model Context Protocol (MCP) Server and Agent Manual Ecosystem

**Feature Branch**: `227-mcp-server-service`

**Created**: 2026-09-18

**Status**: Draft

**Input**: User description: "ttzip需要基于我们的cli，包装和提供完整的mcp服务，用户的agent可以读取相关手册，基于用户自然语言命令直接完成相关服务，现在mac端的ai agent在需要压缩和解压时，总是需要额外编写各种临时脚本，很难用"

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Zero-Scripting Direct Archive Operations (Priority: P1)

An autonomous AI agent running on macOS receives a user request such as "Extract the dataset archive in ~/Downloads/corpus.7z and compress the resulting output into report.zip". Instead of generating fragile bash/python glue scripts that invoke system `/usr/bin/unzip` or missing utilities, the agent directly invokes `ttzip` MCP tools (`ttzip_extract` and `ttzip_create`) with typed JSON parameters, receiving structured operation results and error diagnostics.

**Why this priority**: Eliminates the primary operational bottleneck where agents fail or stall due to missing CLI dependencies, script execution sandboxes, shell escaping bugs, or syntax differences between archive formats.

**Independent Test**: Can be validated by executing a standard MCP tool call sequence (`ttzip_create` followed by `ttzip_extract`) against a synthetic multi-format corpus, verifying target filesystem state and structured JSON response without spawning any external shell or interpreter process.

**Acceptance Scenarios**:
1. **Given** an existing archive file at a specified path, **When** the agent invokes `ttzip_extract` with destination directory and options, **Then** the archive contents are safely extracted and a structured summary (file count, bytes decompressed, elapsed duration) is returned.
2. **Given** a directory of source files, **When** the agent invokes `ttzip_create` with target format and compression level, **Then** the output archive is created and verification checksums are returned.
3. **Given** an encrypted archive, **When** the agent attempts extraction without a password, **Then** the system returns a structured `PASSWORD_REQUIRED` error indicating encryption algorithm and required parameters.

---

### User Story 2 - In-Memory Archive Inspection & Virtual Traversal (Priority: P2)

An AI agent needs to find a specific log file or inspect a configuration file inside a 20GB archive without downloading or extracting the entire archive to the host drive. The agent accesses the archive via the `archive://` MCP resource scheme or the `ttzip_list` and `ttzip_cat` tools, traversing the virtual directory tree and streaming only the single required file into context.

**Why this priority**: Saves disk I/O, prevents storage exhaustion on developer machines, and provides instant file access for large-scale codebases and archives.

**Independent Test**: Can be validated by querying `ttzip_list` and `ttzip_cat` against a 1GB+ multi-file archive, confirming that memory residency remains under 64MB and disk usage is 0 bytes.

**Acceptance Scenarios**:
1. **Given** a multi-file archive, **When** the agent queries `ttzip_list` with a pattern filter, **Then** only matching virtual file records (path, uncompressed size, timestamps, CRC) are returned in JSON format.
2. **Given** a valid archive path and an internal file path, **When** the agent invokes `ttzip_cat`, **Then** the decompressed stream of that single file is returned directly to context without touching the host filesystem.

---

### User Story 3 - Agent Knowledge Grounding & Decision Prompts (Priority: P3)

An agent encountering complex archive operations (e.g. multi-volume splitting, damaged headers, format conversion between 7z and zip) reads built-in MCP resource manuals (`ttzip://manuals/...`) and activates structured MCP prompts (`ttzip_plan_archive_operation`). The agent obtains accurate parameter constraints, format capability matrices, and safety invariants (avoiding tar-bombs, handling path traversals) before taking action.

**Why this priority**: Prevents agent hallucination regarding unsupported compression flags, eliminates format incompatibility failures, and enforces defensive extraction habits.

**Independent Test**: Can be validated by inspecting MCP `resources/list` and `prompts/list` responses, verifying markdown documentation availability and successful prompt expansion into operational instructions.

**Acceptance Scenarios**:
1. **Given** an active MCP session, **When** the agent requests `resources/read` for `ttzip://manuals/format-guide`, **Then** the system returns structured markdown detailing supported codecs (ZIP, 7z, TAR, Snappy, Brotli), performance tradeoffs, and parameter limits.
2. **Given** an ambiguous user instruction to unpack an untrusted archive, **When** the agent invokes the `safe_unpack_prompt`, **Then** the prompt outputs a plan mandating directory isolation and collision avoidance.

---

### Edge Cases

- **Path Traversal Attacks (Zip Slip)**: Archive entries containing relative components (`../`) or absolute paths (`/etc/passwd`) must be blocked or sanitized to prevent writes outside the target sandbox.
- **Corrupted or Truncated Archives**: Damaged streams must trigger structured `CORRUPTED_ARCHIVE` diagnostics with salvageable entry counts rather than crashing or terminating the MCP connection.
- **Character Encoding Variance**: Archive filenames encoded in legacy code pages (CP437, GBK, Shift-JIS) must be automatically normalized to valid UTF-8 strings.
- **Multi-Volume Archives**: Spanned archives (`.7z.001`, `.z01`) must indicate required sibling volume segments if any segment is missing from the working directory.
- **Concurrent Tool Calls**: The MCP server must handle multiple concurrent tool execution requests safely without state corruption or file write collisions.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide an MCP server interface executable via standard CLI command `ttzip mcp` operating over standard input/output (stdio).
- **FR-002**: System MUST implement the Model Context Protocol specification conforming to protocol version `2024-11-05` (or later), supporting `tools`, `resources`, and `prompts` capabilities.
- **FR-003**: System MUST expose atomic MCP tools covering core archive workflows:
  - `ttzip_list`: Query archive table of contents with optional glob filtering and pagination.
  - `ttzip_extract`: Decompress archive entries to a target destination with thread count, password, and pattern filtering.
  - `ttzip_create`: Create archives from paths with format selection (zip, 7z, tar, brotli, snappy), compression levels (0-12), and encryption.
  - `ttzip_inspect`: Retrieve metadata, format container type, encryption status, and compression statistics.
  - `ttzip_cat`: Read content of a single archive entry directly without disk extraction.
  - `ttzip_verify`: Perform integrity checks and CRC/payload verification passes.
  - `ttzip_convert`: Convert existing archives from one format to another with target level specifications.
  - `ttzip_repair`: Attempt structural salvage and recovery on damaged archives.
- **FR-004**: System MUST expose MCP resources under URI schemes:
  - `ttzip://manuals/{topic}`: Static markdown reference manuals detailing tool capabilities, format matrices, security rules, and parameter specifications.
  - `archive://{path}`: Dynamic virtual directory traversal mapping archive internal hierarchies.
- **FR-005**: System MUST expose MCP prompt templates (`ttzip_plan_extraction`, `ttzip_plan_compression`, `ttzip_diagnose_archive`) that provide ready-to-use reasoning scaffolds for AI agents.
- **FR-006**: System MUST return all tool outputs as structured JSON payloads adhering to explicit JSON Schemas, including explicit success booleans, status codes, and execution statistics.
- **FR-007**: System MUST enforce absolute path confinement during extraction, rejecting or stripping path traversal elements (`..`, absolute root paths) to protect host integrity.
- **FR-008**: System MUST emit diagnostic logs exclusively over standard error (stderr) or MCP log notifications, maintaining strict protocol purity on standard output (stdout).

---

### Key Entities

- **ArchiveDescriptor**: Represents an archive file's macroscopic metadata (path, container format, uncompressed size, compressed size, compression ratio, entry count, encryption method, volume count).
- **VirtualEntry**: Represents a single file or directory inside an archive (entry path, uncompressed bytes, compressed bytes, crc32/crc64 checksum, unix permissions, modification timestamp, is_directory flag).
- **ToolExecutionResult**: Standardized envelope for tool execution responses (success boolean, error code enum, error message, execution duration in milliseconds, payload data).
- **AgentManualResource**: Canonical documentation entity containing topic identifier, title, category, and markdown documentation body.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of standard compression, extraction, and listing operations can be executed by an AI agent through native MCP tool calls without generating or running temporary Bash/Python scripts.
- **SC-002**: In-memory inspection (`ttzip_cat` and `ttzip_list`) operations consume 0 bytes of temporary disk storage on the host machine.
- **SC-003**: 100% of malicious archive path traversal attempts (Zip Slip) are rejected prior to filesystem modification.
- **SC-004**: Tool argument validation errors remain below 1% across standard agent invocations due to rigid JSON schema definitions and descriptive tool annotations.
- **SC-005**: Protocol communication maintains 0 protocol framing errors over standard input/output (stdout/stdin).

---

## Assumptions

- The host environment provides a working installation of the `ttzip` CLI binary.
- The consuming AI agent client implements the standard MCP client specification over stdio.
- The underlying TTZip Rust microkernel and engine provide the high-performance decompressors, compressors, and format parsers.
- Network-based MCP transports (SSE / WebSocket) are outside the scope of this CLI-centric baseline; local stdio is the primary transport.
