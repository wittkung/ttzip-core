# Phase 1 Data Model: TTZip Model Context Protocol (MCP) Entities

**Feature**: `227-mcp-server-service`  
**Date**: 2026-09-18  
**Status**: Completed

---

## 1. JSON-RPC 2.0 Transport Layer Entities

```
+-------------------------------------------------------------+
|                      JsonRpcMessage                         |
|  + jsonrpc: "2.0"                                           |
+-------------------------------------------------------------+
               |                               |
               v                               v
+-----------------------------+ +-----------------------------+
|       JsonRpcRequest        | |       JsonRpcResponse       |
|  + id: RequestId            | |  + id: RequestId            |
|  + method: String           | |  + result: Option<Value>    |
|  + params: Option<Value>    | |  + error: Option<RpcError>  |
+-----------------------------+ +-----------------------------+
               |
               v
+-----------------------------+
|     JsonRpcNotification     |
|  + method: String           |
|  + params: Option<Value>    |
+-----------------------------+
```

### Protocol Core Schemas
- **RequestId**: `Number | String | null`
- **JsonRpcError**:
  - `code: i32` (Standard JSON-RPC error codes: `-32700` Parse error, `-32600` Invalid Request, `-32601` Method not found, `-32602` Invalid params, `-32603` Internal error)
  - `message: String`
  - `data: Option<Value>`

---

## 2. Model Context Protocol Core Entities

### Server Capabilities
```json
{
  "tools": { "listChanged": false },
  "resources": { "subscribe": false, "listChanged": false },
  "prompts": { "listChanged": false },
  "logging": {}
}
```

### Tool Definition (`McpTool`)
- `name`: `String` (Unique tool identifier, e.g. `ttzip_extract`)
- `description`: `String` (Human and LLM-readable description of tool action)
- `inputSchema`: `JsonObject` (Valid JSON Schema Draft 7/2020-12 defining parameters)

### Tool Execution Result (`McpToolResult`)
- `content`: `Array<McpContent>`
  - `type`: `"text" | "image" | "resource"`
  - `text`: `String` (Formatted JSON string or human-readable status)
- `isError`: `bool` (True if operation failed, false otherwise)

### Resource Definition (`McpResource`)
- `uri`: `String` (URI identifier, e.g. `ttzip://manuals/format-matrix` or `archive:///tmp/test.zip`)
- `name`: `String` (Display title)
- `description`: `Option<String>` (Description of resource contents)
- `mimeType`: `Option<String>` (MIME type, e.g. `text/markdown`, `application/json`)

### Prompt Definition (`McpPrompt`)
- `name`: `String` (Prompt identifier, e.g. `ttzip_plan_extraction`)
- `description`: `Option<String>` (Prompt purpose)
- `arguments`: `Array<PromptArgument>`
  - `name`: `String`
  - `description`: `Option<String>`
  - `required`: `bool`

---

## 3. TTZip Domain Tool Input & Output Contracts

### 1. `ttzip_list`
**Input Parameters**:
- `archive_path`: `String` (Absolute path to archive file, required)
- `password`: `Option<String>` (Decryption password)
- `include`: `Option<Vec<String>>` (Glob patterns to include)
- `exclude`: `Option<Vec<String>>` (Glob patterns to exclude)
- `page`: `Option<u32>` (1-indexed page number for pagination, default: 1)
- `page_size`: `Option<u32>` (Max entries per page, default: 200, max: 1000)

**Output Payload**:
- `archive_path`: `String`
- `format`: `String` (`zip`, `sevenz`, `tar`, `snappy`, `brotli`)
- `total_entries`: `usize`
- `total_uncompressed_bytes`: `u64`
- `total_compressed_bytes`: `u64`
- `entries`: `Vec<VirtualEntryRecord>`
  - `name`: `String`
  - `relative_path`: `String`
  - `uncompressed_size`: `u64`
  - `compressed_size`: `u64`
  - `is_directory`: `bool`
  - `crc32`: `Option<String>`
  - `modified`: `Option<String>`

---

### 2. `ttzip_extract`
**Input Parameters**:
- `archive_path`: `String` (Absolute path to archive file, required)
- `destination`: `Option<String>` (Target output directory, default: current directory)
- `password`: `Option<String>` (Decryption password)
- `threads`: `Option<u32>` (Parallel extraction worker threads, default: 4)
- `dry_run`: `Option<bool>` (Simulate extraction without writing files, default: false)
- `include`: `Option<Vec<String>>` (Glob patterns to include)
- `exclude`: `Option<Vec<String>>` (Glob patterns to exclude)

**Output Payload**:
- `status`: `"completed" | "simulated"`
- `destination`: `String`
- `files_extracted`: `usize`
- `directories_created`: `usize`
- `bytes_extracted`: `u64`
- `elapsed_millis`: `u64`

---

### 3. `ttzip_create`
**Input Parameters**:
- `archive_path`: `String` (Target archive output path, required)
- `source_paths`: `Vec<String>` (List of files or directories to compress, required)
- `format`: `Option<String>` (`zip`, `7z`, `tar`, `brotli`, `snappy`; default: auto from extension)
- `level`: `Option<u8>` (Compression level 0-12, default: 6)
- `password`: `Option<String>` (Encryption password)
- `threads`: `Option<u32>` (Parallel worker threads, default: 4)
- `volume_size`: `Option<String>` (Split volume chunk size, e.g. "10M", "100MB")
- `dry_run`: `Option<bool>` (Simulate creation without disk write, default: false)

**Output Payload**:
- `status`: `"created" | "simulated"`
- `archive_path`: `String`
- `format`: `String`
- `compressed_size`: `u64`
- `uncompressed_size`: `u64`
- `compression_ratio`: `f64`
- `volume_count`: `usize`
- `elapsed_millis`: `u64`

---

### 4. `ttzip_inspect`
**Input Parameters**:
- `archive_path`: `String` (Absolute path to archive file, required)
- `deep_scan`: `Option<bool>` (Verify entry checksums during inspection, default: false)

**Output Payload**:
- `archive_path`: `String`
- `format`: `String`
- `file_size`: `u64`
- `entry_count`: `usize`
- `directory_count`: `usize`
- `total_uncompressed_bytes`: `u64`
- `compression_ratio`: `f64`
- `is_encrypted`: `bool`
- `is_multi_volume`: `bool`
- `comment`: `Option<String>`

---

### 5. `ttzip_cat`
**Input Parameters**:
- `archive_path`: `String` (Absolute path to archive file, required)
- `entry_path`: `String` (Relative path of entry inside archive, required)
- `password`: `Option<String>` (Decryption password)
- `max_bytes`: `Option<usize>` (Maximum bytes to return, default: 1MB, max: 10MB)

**Output Payload**:
- `archive_path`: `String`
- `entry_path`: `String`
- `size_bytes`: `usize`
- `truncated`: `bool`
- `content`: `String` (UTF-8 text or Base64 encoded payload)

---

### 6. `ttzip_verify`
**Input Parameters**:
- `archive_path`: `String` (Absolute path to archive file, required)
- `password`: `Option<String>` (Decryption password)
- `deep`: `Option<bool>` (Perform full payload decompression CRC check, default: true)

**Output Payload**:
- `valid`: `bool`
- `archive_path`: `String`
- `format`: `String`
- `verified_entries`: `usize`
- `corrupted_entries`: `usize`
- `errors`: `Vec<String>`

---

### 7. `ttzip_convert`
**Input Parameters**:
- `source_archive`: `String` (Source archive path, required)
- `destination_archive`: `String` (Target archive path, required)
- `target_format`: `Option<String>` (`zip`, `7z`, `tar`, `brotli`, `snappy`)
- `level`: `Option<u8>` (Compression level 0-12, default: 6)

**Output Payload**:
- `status`: `"converted"`
- `source_archive`: `String`
- `destination_archive`: `String`
- `entries_transferred`: `usize`
- `original_size`: `u64`
- `new_size`: `u64`
- `ratio_delta`: `f64`

---

### 8. `ttzip_repair`
**Input Parameters**:
- `damaged_archive`: `String` (Path to damaged archive, required)
- `output_path`: `String` (Path for reconstructed archive, required)
- `format_override`: `Option<String>` (`zip`, `tar`)

**Output Payload**:
- `success`: `bool`
- `damaged_archive`: `String`
- `repaired_archive`: `String`
- `salvaged_entries`: `usize`
- `lost_entries`: `usize`
- `repair_log`: `Vec<String>`
