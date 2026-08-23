# Phase 1: Data Model & Type Entities

**Feature**: `161-seven-modules-c-migration`  
**Date**: 2026-08-20  

---

## 1. C Bridge Data Structures

### 1.1 `ttzip_reed_solomon` Entities
```c
// Galois Field lookup tables
extern const uint8_t ttzip_rs_exp_table[512];
extern const uint8_t ttzip_rs_log_table[256];
```

### 1.2 `ttzip_path_filter_opts_t`
| Field | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `exclude_patterns` | `const char* const*` | No | Array of glob exclude patterns |
| `exclude_count` | `size_t` | Yes | Count of exclude patterns |
| `include_patterns` | `const char* const*` | No | Array of glob include patterns |
| `include_count` | `size_t` | Yes | Count of include patterns |
| `exclude_vcs` | `bool` | Yes | Flag to ignore .git, .svn, etc. |
| `no_mac_metadata` | `bool` | Yes | Flag to ignore .DS_Store, __MACOSX, etc. |
| `case_sensitive` | `bool` | Yes | Flag for case sensitivity |

### 1.3 `ttzip_zip_extra_fields_t`
| Field | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `has_extended_timestamp` | `bool` | Yes | True if 0x5455 tag present |
| `timestamp_flags` | `uint8_t` | Yes | Bitmask (0x1 mod, 0x2 acc, 0x4 create) |
| `mod_time` | `uint32_t` | Yes | Unix epoch modification time |
| `acc_time` | `uint32_t` | Yes | Unix epoch access time |
| `create_time` | `uint32_t` | Yes | Unix epoch creation time |
| `unicode_path` | `const char*` | No | Pointer to UTF-8 path string |
| `unicode_path_len` | `size_t` | Yes | Length of Unicode path |
| `unicode_path_crc_valid` | `bool` | Yes | True if standard name CRC matches |
| `has_posix_permissions` | `bool` | Yes | True if 0x7875 tag present |
| `uid` | `uint32_t` | Yes | POSIX User ID |
| `gid` | `uint32_t` | Yes | POSIX Group ID |
| `has_zip64` | `bool` | Yes | True if 0x0001 tag present |
| `uncompressed_size` | `uint64_t` | Yes | 64-bit uncompressed size |
| `compressed_size` | `uint64_t` | Yes | 64-bit compressed size |
| `relative_offset` | `uint64_t` | Yes | 64-bit local header offset |
| `disk_number` | `uint32_t` | Yes | 32-bit disk start number |
| `has_winzip_aes` | `bool` | Yes | True if 0x9901 tag present |
| `aes_version` | `uint16_t` | Yes | AES AE version |
| `aes_vendor_id` | `uint16_t` | Yes | AES vendor ID (0x4541) |
| `aes_strength` | `uint8_t` | Yes | Strength (1: 128, 2: 192, 3: 256) |
| `aes_actual_method` | `uint16_t` | Yes | Underlying compression method |

### 1.4 `ttzip_crypto_probe_ctx_t`
| Field | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `format` | `ttzip_crypto_format_t` | Yes | Enum: Traditional / WinZip AES / 7z AES |
| `salt` | `uint8_t[16]` | Yes | Salt bytes for PBKDF2 |
| `salt_len` | `size_t` | Yes | Salt length (8..16) |
| `pvv` | `uint8_t[2]` | Yes | Password Verification Value |
| `zip_crc_msb` | `uint16_t` | Yes | ZipCrypto MSB check byte |
| `num_cycles_power` | `uint32_t` | Yes | 7z KDF power-of-2 iterations |
| `aes_iv` | `uint8_t[16]` | Yes | 7z AES IV |
| `probe_ciphertext` | `const uint8_t*` | No | Encrypted probe stream chunk |
| `probe_ciphertext_len` | `size_t` | Yes | Length of probe chunk |
| `expected_probe_crc32` | `uint32_t` | Yes | Expected CRC32 of probe block |

### 1.5 `ttzip_search_index_t`
| Field | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `buffer` | `uint8_t*` | Yes | Contiguous flat path/name buffer |
| `buffer_size` | `size_t` | Yes | Occupied bytes in buffer |
| `buffer_capacity` | `size_t` | Yes | Total buffer allocation capacity |
| `descriptors` | `ttzip_search_entry_desc_t*` | Yes | Flat array of entry descriptors |
| `entry_count` | `size_t` | Yes | Count of indexed entries |
| `entry_capacity` | `size_t` | Yes | Descriptor capacity |

### 1.6 `ttzip_tensor_intersect_block_t`
| Field | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `chunk_idx` | `int64_t` | Yes | Global linear chunk index |
| `block_idx_in_chunk` | `int32_t` | Yes | Block index inside chunk |
| `chunk_coords` | `int64_t[8]` | Yes | Multi-dim chunk grid coordinates |
| `block_coords` | `int64_t[8]` | Yes | Multi-dim block grid coordinates |
| `global_start_coords` | `int64_t[8]` | Yes | Global element coordinates |
