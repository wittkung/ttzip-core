# Data Model: 147-full-122-files-c-migration-and-swift-slimming

## Entity: `ttzip_split_config_t`
- `max_volume_bytes`: `uint64_t` (e.g. 100MB, 2GB per split part)
- `naming_pattern`: `ttzip_split_naming_t` (0 = PKZIP .z01, 1 = 7z .001)
- `target_base_path`: `const char *`

## Entity: `ttzip_inplace_mutation_t`
- `archive_path`: `const char *`
- `action`: `ttzip_inplace_action_t` (0 = APPEND, 1 = DELETE, 2 = RENAME)
- `target_entry_name`: `const char *`
- `new_file_src_path`: `const char *`

## Entity: `ttzip_security_context_t`
- `password`: `const char *`
- `recovery_record_percentage`: `double` (e.g. 3.0 = 3% parity data)
- `is_dse_scrubbed`: `bool`
