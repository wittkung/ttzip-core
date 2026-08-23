# Data Model: 7z Solid 解压与 ARM64 密码加速模型 (Feature 167)

## 1. 7z 硬件加速密码会话模型 (`TTZip7zCryptoSession`)

Represents an initialized ARM64 hardware accelerated AES-256-CBC decrypt/encrypt session.

| Field Name | Type | Nullable | Constraints / Description |
| :--- | :--- | :---: | :--- |
| `expanded_enc_keys` | `array of 15 uint32x4` | No | 15 forward round keys for encryption |
| `expanded_dec_keys` | `array of 15 uint32x4` | No | 15 equivalent inverse round keys for decryption |
| `iv` | `uint32x4 (16 bytes)` | No | Initial Vector for CBC mode |
| `is_hardware_accelerated` | `boolean` | No | True if ARM64 ACLE Crypto is active |
| `num_cycles_power` | `integer` | No | Log2 of KDF cycles ($N \in [0, 24]$) |

---

## 2. 7z Solid 单条目流式提取请求模型 (`TTZip7zSolidEntryRequest`)

Represents a selective entry extraction query against a Solid 7z compressed stream.

| Field Name | Type | Nullable | Constraints / Description |
| :--- | :--- | :---: | :--- |
| `solid_stream_index` | `integer` | No | Target entry index in solid block ($K \ge 0$) |
| `pre_entry_skip_bytes` | `integer` | No | Offset in uncompressed solid stream to target file start |
| `target_entry_size` | `integer` | No | Uncompressed byte length of target file |
| `expected_crc32` | `integer` | No | Expected 32-bit CRC checksum |
| `out_buffer` | `pointer / buffer` | Yes | Caller-provided destination buffer (or NULL for file writing) |
| `out_fd` | `integer` | Yes | Target file descriptor (or -1 for memory extraction) |
