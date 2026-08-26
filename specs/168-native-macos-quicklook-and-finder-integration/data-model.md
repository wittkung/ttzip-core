# Data Model: macOS Quick Look 与 Finder 拖拽交互模型 (Feature 168)

## 1. Quick Look 预览请求模型 (`QuickLookPreviewRequest`)

Represents an interactive space-bar or click preview query for a selected archive entry or disk file.

| Field Name | Type | Nullable | Constraints / Description |
| :--- | :--- | :---: | :--- |
| `request_id` | `UUID` | No | Unique preview request identifier |
| `source_type` | `enum` | No | `DISK_FILE`, `ARCHIVE_VIRTUAL_ENTRY` |
| `archive_path` | `string` | Yes | Path to parent archive if source is virtual entry |
| `entry_path` | `string` | No | Virtual path inside archive or absolute disk path |
| `staged_url` | `URL` | Yes | Ephemeral sandboxed file URL staged for Quick Look |
| `is_password_protected` | `boolean` | No | True if encryption key is required |
| `staging_status` | `enum` | No | `IDLE`, `EXTRACTING`, `STAGED_READY`, `DISMISSED`, `FAILED` |

---

## 2. Finder 拖拽承诺模型 (`FinderDragPromisePayload`)

Represents a deferred file promise registered with macOS Pasteboard for Finder drag-and-drop.

| Field Name | Type | Nullable | Constraints / Description |
| :--- | :--- | :---: | :--- |
| `promise_id` | `UUID` | No | Unique drag promise identifier |
| `suggested_file_name` | `string` | No | Suggested destination filename (e.g. `document.pdf`) |
| `uniform_type_identifier`| `string` | No | Standard macOS UTI (e.g. `com.adobe.pdf`, `public.data`) |
| `archive_url` | `URL` | No | File URL of source container |
| `virtual_entry_path` | `string` | No | Relative path inside source container |
| `uncompressed_byte_size`| `integer` | No | Expected byte size of extracted file |
| `write_destination_url` | `URL` | Yes | Destination directory provided by Finder upon drop |
