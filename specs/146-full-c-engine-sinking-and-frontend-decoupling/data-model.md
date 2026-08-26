# Data Model: 146-full-c-engine-sinking-and-frontend-decoupling

## Entity: `ttzip_file_kind_t`
- `TTZIP_KIND_UNKNOWN = 0`
- `TTZIP_KIND_IMAGE   = 1`
- `TTZIP_KIND_VIDEO   = 2`
- `TTZIP_KIND_AUDIO   = 3`
- `TTZIP_KIND_PDF     = 4`
- `TTZIP_KIND_ARCHIVE = 5`
- `TTZIP_KIND_TEXT    = 6`
- `TTZIP_KIND_CODE    = 7`
- `TTZIP_KIND_DOC     = 8`

## Entity: `ttzip_magic_info_t`
- `kind`: `ttzip_file_kind_t`
- `mime_type`: `const char *`
- `format_name`: `const char *`
- `is_archive`: `bool`

## Entity: `ttzip_archive_report_t`
- `entry_count`: `uint64_t`
- `total_uncompressed_bytes`: `uint64_t`
- `total_compressed_bytes`: `uint64_t`
- `corrupted_entries`: `uint32_t`
- `is_encrypted`: `bool`
- `format_name`: `const char *`
