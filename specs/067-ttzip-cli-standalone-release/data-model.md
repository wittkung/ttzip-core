# Data Model: CLI Release Manifest & Output Contracts

## Entity: CLIReleaseManifest
Defines release artifact metadata for `ttzip-cli`.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `name` | String | Yes | Binary identifier (`ttzip-cli`) |
| `version` | String | Yes | SemVer string (`1.0.0`) |
| `target_triple` | String | Yes | macOS deployment target (`universal-apple-macosx14.0`) |
| `architectures` | Array of String | Yes | Architectures included (`arm64`, `x86_64`) |
| `sha256_checksum` | String | Yes | Computed SHA-256 hexadecimal hash of release tarball |
| `tarball_filename`| String | Yes | Tarball name (`ttzip-cli-v1.0.0-macos-universal.tar.gz`) |

---

## Entity: CLIInspectJSONOutput
Defines structured `--json` schema output for `ttzip-cli inspect`.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `archive_path` | String | Yes | Absolute path to archive |
| `format` | String | Yes | Format enum name (e.g. `zip`, `7z`, `tar.zst`) |
| `total_entries` | Integer | Yes | Count of entries inside archive |
| `uncompressed_size` | Integer | Yes | Total bytes uncompressed |
| `compressed_size` | Integer | Yes | Total archive file size on disk |
| `is_encrypted` | Boolean | Yes | Flag indicating encryption presence |
