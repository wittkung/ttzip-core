# Data Model: 145-pure-c-container-framing-and-cli-engine

## Entity: `ttzip_archive_config_t`
- `codec`: `ttzip_api_codec_t` (0 = Store, 1 = Deflate, 2 = Zstd, 3 = LZMA2, etc.)
- `level`: `int32_t` (compression level 1..19)
- `threads`: `uint32_t` (thread count, 0 = auto P/E topology)
- `password`: `const char *` (optional encryption password)
- `solid_block_size`: `size_t` (solid archive block size in bytes)

## Entity: `ttzip_archive_entry_info_t`
- `file_name`: `const char *` (UTF-8 relative path)
- `compressed_size`: `uint64_t`
- `uncompressed_size`: `uint64_t`
- `crc32`: `uint32_t`
- `is_directory`: `bool`
- `is_encrypted`: `bool`
