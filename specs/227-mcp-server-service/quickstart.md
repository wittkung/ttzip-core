# Quickstart Validation Guide: TTZip Model Context Protocol (MCP) Server

**Feature**: `227-mcp-server-service`  
**Date**: 2026-09-18  
**Status**: Fully Implemented & E2E Verified (Production-Ready)

---

## 1. Overview & Invocation

The TTZip MCP server is embedded directly into the `ttzip` CLI binary. It communicates with AI agents over standard input and standard output (`stdio`) using JSON-RPC 2.0.

### Direct Subcommand
```bash
# Launch MCP server over stdio
ttzip mcp
```

Diagnostic logs, tracing messages, and runtime errors are emitted strictly to `stderr`, keeping `stdout` completely clean for JSON-RPC 2.0 frames.

---

## 2. Client Configuration

### A. Claude Desktop Configuration
Add the following snippet to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "ttzip": {
      "command": "ttzip",
      "args": ["mcp"]
    }
  }
}
```

### B. Antigravity / Cursor Configuration
Add to `.gemini/antigravity/mcp/ttzip.json` or `.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "ttzip": {
      "command": "ttzip",
      "args": ["mcp"]
    }
  }
}
```

---

## 3. End-to-End Verification Scenarios

### Scenario 1: Protocol Initialization & Handshake
Validate that `ttzip mcp` responds to the standard MCP initialization sequence.

**Input (stdin)**:
```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0.0"}}}
```

**Expected Output (stdout)**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": "2024-11-05",
    "capabilities": {
      "tools": { "listChanged": false },
      "resources": { "subscribe": false, "listChanged": false },
      "prompts": { "listChanged": false },
      "logging": {}
    },
    "serverInfo": {
      "name": "ttzip-mcp",
      "version": "1.0.0"
    }
  }
}
```

---

### Scenario 2: Tool Discovery (`tools/list`)
Verify that all 8 archive management tools are exposed with valid JSON Schema parameter declarations.

**Input (stdin)**:
```json
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
```

**Verification Check**:
- `result.tools` contains 8 entries: `ttzip_list`, `ttzip_extract`, `ttzip_create`, `ttzip_inspect`, `ttzip_cat`, `ttzip_verify`, `ttzip_convert`, `ttzip_repair`.
- Each tool defines a non-empty `description` and a valid `inputSchema` object.

---

### Scenario 3: Archive Listing & Filtering (`tools/call`)
Verify listing of archive contents without writing files to disk.

**Input (stdin)**:
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "ttzip_list",
    "arguments": {
      "archive_path": "/tmp/test_corpus.zip",
      "include": ["*.txt"]
    }
  }
}
```

**Verification Check**:
- `result.isError` is `false`.
- `result.content[0].type` is `"text"`.
- Text payload deserializes into JSON containing `entries`, `total_entries`, and byte counters matching the filter.

---

### Scenario 4: Direct In-Memory Inspection (`ttzip_cat`)
Verify single file retrieval into context with 0 temporary disk writes.

**Input (stdin)**:
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "tools/call",
  "params": {
    "name": "ttzip_cat",
    "arguments": {
      "archive_path": "/tmp/test_corpus.zip",
      "entry_path": "nested/config.json"
    }
  }
}
```

**Verification Check**:
- Text content matches the uncompressed file bytes.
- Host temporary directory (`/tmp` or `$TMPDIR`) has 0 newly created extraction directories.

---

### Scenario 5: Agent Manual Grounding (`resources/read`)
Verify agent retrieval of built-in technical documentation.

**Input (stdin)**:
```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "resources/read",
  "params": {
    "uri": "ttzip://manuals/format-matrix"
  }
}
```

**Verification Check**:
- `result.contents[0].mimeType` is `"text/markdown"`.
- Text content contains technical comparison tables for ZIP, 7z, TAR, Snappy, and Brotli.

---

### Scenario 6: Zip Slip Path Traversal Immunity
Verify that malicious relative paths inside an archive cannot write files outside the target sandbox directory.

**Input (stdin)**:
```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "method": "tools/call",
  "params": {
    "name": "ttzip_extract",
    "arguments": {
      "archive_path": "/tmp/malicious_zipslip.zip",
      "destination": "/tmp/sandbox"
    }
  }
}
```

**Verification Check**:
- Tool call rejects or sanitizes escaping paths.
- No files are created in `/tmp` outside `/tmp/sandbox/`.

---

## 4. Automated Smoke Test Script

A verification script using stdio piping:

```bash
printf '%s\n%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"smoke","version":"1"}}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
| cargo run -p ttzip-tui -- mcp \
| jq -c '.result | keys'
```
