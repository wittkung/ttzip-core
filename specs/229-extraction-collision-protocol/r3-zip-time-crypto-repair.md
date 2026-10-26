# R3 设计与交接记录：ZIP 时间戳修复（Q1-ISSUE-03）与 ZipCrypto 兼容修复（C3-04）

## 1. 实际根因分析

### 1.1 Q1-ISSUE-03：ZIP 时间戳严重偏移（2023 -> 2070、2026 -> 2030）

1. **主因：流式写入与原位编辑路径将 `unix_to_dos_time` 返回元组 `(dos_time, dos_date)` 反向解构，且漏写 `0x5455` 扩展时间戳**
   - `products/ttzip/core/rust/crates/ttzip-archive/src/zip/writer/types.rs` 中的 `unix_to_dos_time(epoch_secs: u32) -> (u16, u16)` 返回顺序为 `(dos_time, dos_date)`。
   - `assemble.rs` 与 `store_stream.rs` 按正确顺序 `let (dos_time, dos_date) = unix_to_dos_time(...)` 解构并写入了 `0x5455` Extended Timestamp。
   - 但默认流式并行写入路径 `zip/writer/parallel_batch.rs`（第 315、501、516 行）、`zip/writer/streaming_sink.rs`（第 184 行）以及原位编辑路径 `archive/in_place_edit/zip.rs`（第 195、255 行）将其反向解构为 `let (dos_date, dos_time) = unix_to_dos_time(...)`，导致 LFH 与 CDFH 中 `dos_time` 与 `dos_date` 槽位互换；同时上述路径均未写入 `0x5455` Extended Timestamp。
   - 以 Q1 固定历史时间样本 `1700001234`（`2023-11-14 22:33:54 UTC`）为例，其 `dos_time`（`0xB43B`）被写入 `dos_date` 字段后，读取端解析年份为 `1980 + (0xB43B >> 9) = 1980 + 90 = 2070` 年；2026 年样本同理偏移至 2030 年。

2. **次因：DOS 本地日历时间转换与读取端字段优先级不统一**
   - `unix_to_dos_time` 原先采用手写 UTC 算术，未遵循 PKWARE APPNOTE MS-DOS 本地日历时间语义（`localtime_r`）。
   - `zip/parser.rs` 的 `parse_cdfh_entry` 原先对无 `0x5455` 的条目直接调用 UTC 版 `dos_to_unix_time`，且忽略了 `0x000a` NTFS 时间戳扩展字段；`zip_central_directory.rs` 亦未纳入 `0x000a` NTFS 时间戳回退。

### 1.2 C3-04：正确 ZipCrypto 密码仍失败（`ErrInvalidPassword`）

1. **样本核验结论：Q1 样本与密码完全合法**
   - 经 Python `zipfile`、`/usr/bin/unzip` 与 `7z` 独立核验，`/tmp/ttzip-q1-acceptance-20261017/samples/external_archives/ext_zip_zipcrypto.zip`（密码 `Secret2026!`）为合法的 Traditional PKWARE ZipCrypto 归档（含 `hello.txt`、`sub/`、`sub/nested.txt` 共 3 个条目；`hello.txt` 参数为 `flag=0x1`, `method=0 (Store)`, `csize=46`, `usize=34`, `crc32=0xb35bccb4`, `lfh_dos_time=0xa439`, `lfh_dos_date=0x5d51`，带 `0x000a` NTFS 扩展字段，明文为 `"Hello from external tool archive!\n"`）。

2. **根因 A：读取端将所有加密 ZIP 条目硬编码按 WinZip AES-256 处理**
   - `zip/reader.rs`（`extract_entry_bytes`、`extract_entry_to_sink_with_sheet`、`verify_entry_encryption_key`、`extract_all_with_sheet`）与 `zip/reader_stream.rs`（`decrypt_entry_payload`、`preflight_verify_encrypted_zip_entries`）对所有 `entry.is_encrypted == true` 的条目无条件要求 `compressed_size >= 28` 并调用 `winzip_aes256_decrypt_and_verify`。
   - 对 `compression_method != 99` 的传统 ZipCrypto 条目，AES 校验必然返回 `ErrInvalidPassword`；而 `fallback_policy::should_abort_without_fallback(TTZipStatus::ErrInvalidPassword)` 为 `true`，导致提取立即中止，既未调用 `ZipCryptoEngine`，也不会回退至 `libarchive`。

3. **根因 B：流式写入端 ZipCrypto 校验字节误用 `dos_date`**
   - 在 `zip/writer/parallel_batch.rs:501` 中，流式 Data Descriptor（Bit 3 = 1）模式构造 12 字节 ZipCrypto 加密头时写成了 `let (_, dos_time) = unix_to_dos_time(mtime_secs)`，实际把 `dos_date` 传给了 `compute_zipcrypto_check_byte(crc, dos_time, true)`，导致 `header[11]` 写入了 `(dos_date >> 8) as u8` 而非 `(lfh_dos_time >> 8) as u8`。

---

## 2. 设计与修复方案

### 2.1 统一时间模型与字段优先级（`zip/datetime.rs`）

- 复用并扩展 `DosDateTime`：
  - `DosDateTime::from_local_unix_epoch_secs(secs: i64) -> Self`：通过 `libc::localtime_r` 将 Unix epoch 秒转换为本地日历时间；`< 1980-01-01 00:00:00` 钳位至 `DosDateTime::MIN`，`> 2107-12-31 23:59:58` 钳位至 `DosDateTime::MAX`；奇数秒按 DOS 规范向下对齐到 2 秒粒度（`sec & !1`）。
  - `DosDateTime::to_local_unix_epoch_secs(&self) -> i64`：通过 `libc::mktime`（`tm_isdst = -1`）还原为本地 Unix epoch 秒。
  - `resolve_zip_mtime_secs(extra: &ZipExtraFields, dos_time: u16, dos_date: u16) -> i64`：严格执行三级优先级：
    1. `0x5455` Extended Timestamp（`extra.mod_time`，UTC，1 秒精度）；
    2. `0x000a` NTFS Extra Field（`extra.ntfs`，UTC，1 秒精度）；
    3. MS-DOS 本地时间（`dos_to_local_unix_epoch_secs(dos_time, dos_date)`，本地日历时间，2 秒精度）。
- 写入与原位编辑端统一：
  - `unix_to_dos_time(epoch_secs: u32) -> (u16, u16)` 委托给 `DosDateTime::from_local_unix_epoch_secs(epoch_secs as i64)`，统一返回 `(dos_time, dos_date)`。
  - 修复 `parallel_batch.rs`（目录 LFH、文件 LFH、ZipCrypto check byte）、`streaming_sink.rs`（CDFH）以及 `archive/in_place_edit/zip.rs`（`write_lfh`、`write_cdfh`）的元组解构顺序，并在 LFH 与 CDFH 中补齐 `0x5455` Extended Timestamp 扩展字段，确保 Store、串行、并行、流式、分卷、加密创建及原位增删改路径的 LFH/CDFH/`0x5455` 完全一致。

### 2.2 ZipCrypto 校验、AE-1/AE-2 CRC32 边界与错误语义隔离（`zip/parser.rs`、`zip/reader.rs` & `zip/reader_stream.rs`）

1. **按 `entry.compression_method` 与有效 `0x9901` Extra 区分加密体制与 AE 版本**
   - 在内部 Rust 结构 `ZipEntry`（`zip/parser.rs`，非 C-ABI / UniFFI 类型，全仓无跨范围字面量构造点）中新增保留 `pub aes_version: u16`。
   - 仅当 `0x9901` WinZip AES 扩展字段满足有效性契约：`extra.has_winzip_aes && extra.aes_vendor_id == WINZIP_AES_VENDOR_ID (0x4541) && matches!(extra.aes_version, WINZIP_AES_VERSION_AE1 (0x0001) | WINZIP_AES_VERSION_AE2 (0x0002)) && (1..=3).contains(&extra.aes_strength)` 时，才将 `aes_version`、`aes_strength` 与 `actual_method` 写入 `ZipEntry`（`zip_central_directory.rs` 同步采用相同有效性判定）。
   - `compression_method == 99`：要求 `is_valid_winzip_aes_entry(entry)` 为真（且若 LFH 亦携带 `0x9901` 扩展字段，其有效性及 `aes_version` / `aes_strength` / `actual_method` 必须与 CDFH 一致，否则返回 `TTZipStatus::ErrCorruptHeader`），并强制验证 2 字节 PVV 与 10 字节 HMAC-SHA1-80 认证标签。
   - `compression_method != 99`：Traditional PKWARE ZipCrypto（要求 `compressed_size >= 12`）。

2. **PKWARE APPNOTE 校验字节规则**
   - 读取 LFH 偏移处的 `lfh_flag` 与 `lfh_dos_time`，计算 `bit3 = ((entry.flag | lfh_flag) & 0x0008) != 0`；
   - `bit3 == false` 时校验 `decrypted_header[11] == (entry.crc32 >> 24) as u8`；
   - `bit3 == true` 时校验 `decrypted_header[11] == (lfh_dos_time >> 8) as u8`。

3. **CRC32 完整性校验边界（`should_verify_entry_crc32`）：严格按有效 `aes_version` 区分 AE-1 与 AE-2**
   - **绝不凭 `compression_method == 99 && entry.crc32 == 0` 推断 AE-2**：
     `should_verify_entry_crc32(entry)` 仅在 `is_valid_winzip_aes_entry(entry) && entry.aes_version == WINZIP_AES_VERSION_AE2` 时返回 `false`（AE-2 规范将 header `crc32` 置 0 并抑制 CRC32 校验，完整性由 2 字节 PVV + 10 字节 HMAC-SHA1-80 认证标签保障）。
   - **WinZip AE-1（`entry.aes_version == WINZIP_AES_VERSION_AE1`）始终强制执行 CRC32 校验**：
     即使 AE-1 条目在 header 中声明 `entry.crc32 == 0`，也必须计算解压后明文的 `computed_crc` 并校验 `computed_crc == entry.crc32`：
     - 当 AE-1 明文真实 CRC32 非零、加密负载与 HMAC-SHA1-80 认证均合法，但 header 错误声明 `crc32 == 0`（或任意错误 CRC）时，内存与流式入口均拒绝并返回 `TTZipStatus::ErrCorruptHeader`；
     - 当 AE-1 明文真实 CRC32 恰好为 `0`（或声明了匹配的真实非零 CRC32）且认证通过时，校验通过。
   - **Traditional ZipCrypto 条目与未加密条目**：无论 `entry.crc32` 是否为 `0x00000000`，始终强制校验 `computed_crc == entry.crc32`（未加密 CRC 不匹配返回 `ErrCorruptHeader`，ZipCrypto CRC 不匹配返回 `ErrInvalidPassword`）。

4. **错误语义严格隔离（`map_zipcrypto_decode_err`）**
   - ZipCrypto 单字节校验存在 $1/256$ 假阳性概率；当错误密码偶发通过首字节校验导致后续解压流损坏或 CRC32 不匹配时，仅将 `TTZipStatus::ErrCorruptHeader` 映射为 `TTZipStatus::ErrInvalidPassword`。
   - **严格保留安全限制、取消与 I/O 错误语义**：
     - 解压输出超过声明 `uncompressed_size` 配额（Zip Bomb / 越界膨胀）返回的 `TTZipStatus::ErrSecurityViolation` 原样保留，绝不映射为 `ErrInvalidPassword`；
     - 异步取消 `TTZipStatus::Cancelled`、写入/I/O 错误 `TTZipStatus::ErrExtractionFailed`、内存超限 `TTZipStatus::ErrOutOfMemory` 全部原样透传。

---

## 3. 修改文件清单与源码 SHA-256 哈希

以下为 R3 独占修改范围内的 12 个代码与测试文件的最终精确 SHA-256 哈希：

| 文件路径 | SHA-256 |
| :--- | :--- |
| `products/ttzip/core/rust/crates/ttzip-archive/src/zip/datetime.rs` | `5f982525786369479a86d0771e25e180c9a739e6a057e8110d0bd00cf44797f1` |
| `products/ttzip/core/rust/crates/ttzip-archive/src/zip/mod.rs` | `de79972c7eaa95e40b61623df8239ea8e7dffc27a2bb1c253736e83302a28e41` |
| `products/ttzip/core/rust/crates/ttzip-archive/src/zip/parser.rs` | `94a11133759a814cf3f92b1817ec2416a4dfac0c15f038aec1af46973d33533f` |
| `products/ttzip/core/rust/crates/ttzip-archive/src/zip/reader.rs` | `b4169e351f404aee5a7acbbce9e1b48eaeb77e1adaef9a0bd38aac1935af0854` |
| `products/ttzip/core/rust/crates/ttzip-archive/src/zip/reader_stream.rs` | `8d9ab6ca6b3c06feb7facd853b4e52be3bb2a7e0b72a4d1527d09c4bbb5ad99b` |
| `products/ttzip/core/rust/crates/ttzip-archive/src/zip/writer/parallel_batch.rs` | `f132672c1487c48b037a24d36004508b0b1bf819edd595e8be265b2422f444e7` |
| `products/ttzip/core/rust/crates/ttzip-archive/src/zip/writer/streaming_sink.rs` | `7bce2e5b98073ffdaeef9c72580c9c7ac3902a3ab687555b4f19ad712b9aa992` |
| `products/ttzip/core/rust/crates/ttzip-archive/src/zip/writer/types.rs` | `86896cc728403a0b541bbf45cb1b5d1925ed912241e6678ad7c837dc7de3ddab` |
| `products/ttzip/core/rust/crates/ttzip-archive/src/archive/unified/zip_central_directory.rs` | `063a9d2855ff899fff44c5c745c3a5bf8f3688f0f11260c6e6fd00a6920a472f` |
| `products/ttzip/core/rust/crates/ttzip-archive/src/archive/in_place_edit/zip.rs` | `023583bfec1203cab5681491ba7e09cecfdb3e89d093634518ba572c33829c50` |
| `products/ttzip/core/rust/ttzip-engine/tests/r3_zip_timestamp_crypto_regression_tests.rs` | `c95d228201cce26db4175af3739489b7fcb2f0b11a00b4b576fea8a6cdda99de` |
| `products/ttzip/core/rust/ttzip-engine/tests/r3_zip_crc_and_aes_version_regression_tests.rs` | `f996f9acb8676d9983865156081c05eb33ee2726abe36e44eb949eb3a5c07268` |

---

## 4. 独立专项回归测试（按职责拆分为两个测试套件）

为遵守单文件 `<= 800` 行、单函数 `<= 80` 行、圈复杂度 `<= 15` 门禁且不压缩代码排版，R3 专项回归按职责拆分为两个独立测试文件（均由 `products/ttzip/core/rust/ttzip-engine/BUILD.bazel` 的 `glob(["tests/*.rs"])` 与 Cargo 自动发现，无需修改 `BUILD.bazel`）：

### 4.1 `r3_zip_timestamp_crypto_regression_tests.rs`（721 行，最大函数 66 行，最大圈复杂度 5，共 7 项测试）
1. `test_q1_issue_03_all_creation_paths_header_and_roundtrip_fidelity`
   - 覆盖 Q1 两个固定历史时间戳（`hist_2023_nov.txt = 1700001234`、`hist_2023_mar.sh = 1680000000`）在 **Store 并行流**、**单线程流式 Deflate**、**多线程并行流式 Deflate**、**ZipCrypto 加密流**、**WinZip AES-256 加密流** 全部路径下的 LFH/CDFH `dos_time`、`dos_date`、`0x5455` 扩展时间戳一致性及解压后文件 `mtime`、内容、权限无回归。
2. `test_in_memory_parallel_and_split_volume_timestamp_and_crypto_fidelity`
   - 覆盖内存并行批处理（`compress_items_parallel` + `assemble_zip_archive`）与分卷流式创建（`create_zip_streaming_parallel_split`）下的历史时间戳、LFH/CDFH 一致性、ZipCrypto 正确密码提取与错误密码拒绝。
3. `test_calendar_boundaries_leap_years_and_priority_resolution`
   - 覆盖闰日（`2000-02-29`、`2024-02-29`）、跨年边界、DOS 范围边界（`1980-01-01 00:00:00` .. `2107-12-31 23:59:58`）、2 秒偶对齐规则，以及 `0x5455` > `0x000a` NTFS > DOS 本地时间三级优先级。
4. `test_timezone_matrix_utc_vs_pure_dos_contract`
   - 跨 `UTC0`、`Asia/Shanghai`、`America/New_York` 三个时区验证：含 `0x5455` 时严格按 1 秒精度 UTC epoch 解析；纯 DOS 时间按本地日历时间及 2 秒精度契约解析。
5. `test_c3_04_external_tools_infozip_cross_verification`
   - 独立外部工具双向交叉验证：`/usr/bin/zip`（`-P Secret2026!` 含 bit 3 Data Descriptor）创建 -> TTZip 提取并校验历史 `mtime` 与错误密码拒绝；TTZip 创建 ZipCrypto 归档 -> `/usr/bin/unzip` 提取并校验历史 `mtime`。
6. `test_in_place_edit_zip_preserves_timestamps_and_content`
   - 验证 `in_place_edit_zip` 执行原位删除、替换与新增后，未修改条目与新写入条目的 LFH/CDFH `dos_time`、`dos_date`、`0x5455` 扩展时间戳及解压后内容、`mtime` 完全准确。
7. `test_sha256_permissions_symlinks_and_cancellation_non_regression`
   - 验证字节内容完整性、只读权限（`0o444`）、符号链接、创建/解压取消保护（`TTZipStatus::Cancelled`）及流式 Reader 提取无回归。

### 4.2 `r3_zip_crc_and_aes_version_regression_tests.rs`（650 行，最大函数 76 行，最大圈复杂度 3，共 6 项测试）
1. `test_c3_04_q1_embedded_fixture_roundtrip_and_password_rejection`
   - 内嵌 Q1 C3-04 的 507 字节原始样本 `Q1_EXT_ZIP_ZIPCRYPTO_BYTES`，不依赖 `/tmp` 外部文件，独立验证正确密码 `Secret2026!` 提取成功、错误密码与空密码返回 `TTZipStatus::ErrInvalidPassword`。
2. `test_c3_04_zipcrypto_data_descriptor_roundtrip_and_wrong_password`
   - 覆盖含 bit 3 Data Descriptor（`flag = 0x0009`，校验字节取 `lfh_dos_time >> 8`）的 ZipCrypto Deflate 样本在内存提取（`extract_entry_bytes`、`extract_entry_to_sink`）与流式提取（`extract_zip_entry_to_sink_from_reader`）下的正确密码成功与错误密码拒绝。
3. `test_zero_crc32_non_empty_zipcrypto_and_unencrypted_verification`
   - 使用确定性 4 字节非空明文（IEEE CRC32 精确为 `0x00000000`），覆盖未加密与传统 ZipCrypto（Store / Deflate）在内存与流式入口下的 `crc32 == 0` 完整校验，以及篡改密文/错误密码时的拒绝行为。
4. `test_winzip_aes_ae1_enforces_crc32_including_zero_crc_boundary`
   - 覆盖合法 WinZip **AE-1**（`aes_version == 1`）在内存与流式入口下的完整校验：
     - 声明匹配的非零真实 CRC32 时提取成功，错误密码返回 `ErrInvalidPassword`；
     - 明文真实 CRC32 为 `0` 且声明 `crc32 == 0` 时提取成功；
     - **关键对照测试**：加密负载与 PVV + HMAC-SHA1-80 认证均完全有效，但 AE-1 头部将非零真实 CRC32 错误声明为 `0` 时，内存提取（`extract_entry_bytes`、`extract_entry_to_sink`）与流式提取（`extract_zip_entry_to_sink_from_reader`）均强制执行 CRC32 校验并返回 `TTZipStatus::ErrCorruptHeader`。
5. `test_winzip_aes_ae2_and_invalid_aes_extra_contracts`
   - 覆盖合法 WinZip **AE-2**（`aes_version == 2`，`crc32 == 0`）在内存与流式入口下按 AE-2 契约跳过冗余 CRC32、同时严格保留 PVV 与 HMAC-SHA1-80 认证校验（篡改 1 bit HMAC tag 或错误密码均返回 `ErrInvalidPassword`）；
   - 验证非法 `aes_version = 3`、非法 `vendor_id = 0x5a5a` 或 LFH/CDFH `0x9901` 版本不一致（LFH 为 AE-1、CDFH 为 AE-2）时，内存与流式入口均拒绝并返回 `TTZipStatus::ErrCorruptHeader`，绝不凭 `method == 99 && crc32 == 0` 盲目放行。
6. `test_zipcrypto_preserves_security_cancellation_and_io_errors`
   - 验证 ZipCrypto 解压输出超出声明大小配额时原样返回 `TTZipStatus::ErrSecurityViolation`，取消时返回 `TTZipStatus::Cancelled`，Sink 写入 I/O 失败时返回 `TTZipStatus::ErrExtractionFailed`，绝不误报为 `ErrInvalidPassword`。

---

## 5. 验证记录与独立工具版本

### 5.1 当前工作区源码直接验证（无任何编译豁免、无源码拼接）

在当前工作区 `$HOME/Documents/dev/metastudyline/source` 直接运行以下命令（未设置任何 `RUSTFLAGS` 豁免，未借用其他基线文件）：

1. **R3 两个独立专项回归测试套件**：
   - 命令：
     ```bash
     cargo test --manifest-path products/ttzip/core/rust/Cargo.toml --test r3_zip_timestamp_crypto_regression_tests --test r3_zip_crc_and_aes_version_regression_tests
     ```
   - 原始结果：
     ```text
     Running tests/r3_zip_crc_and_aes_version_regression_tests.rs
     running 6 tests
     test test_zero_crc32_non_empty_zipcrypto_and_unencrypted_verification ... ok
     test test_c3_04_zipcrypto_data_descriptor_roundtrip_and_wrong_password ... ok
     test test_zipcrypto_preserves_security_cancellation_and_io_errors ... ok
     test test_c3_04_q1_embedded_fixture_roundtrip_and_password_rejection ... ok
     test test_winzip_aes_ae2_and_invalid_aes_extra_contracts ... ok
     test test_winzip_aes_ae1_enforces_crc32_including_zero_crc_boundary ... ok

     test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running tests/r3_zip_timestamp_crypto_regression_tests.rs
     running 7 tests
     test test_calendar_boundaries_leap_years_and_priority_resolution ... ok
     test test_in_place_edit_zip_preserves_timestamps_and_content ... ok
     test test_in_memory_parallel_and_split_volume_timestamp_and_crypto_fidelity ... ok
     test test_sha256_permissions_symlinks_and_cancellation_non_regression ... ok
     test test_c3_04_external_tools_infozip_cross_verification ... ok
     test test_timezone_matrix_utc_vs_pure_dos_contract ... ok
     test test_q1_issue_03_all_creation_paths_header_and_roundtrip_fidelity ... ok

     test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     ```

2. **`ttzip-archive` 库编译与单元测试状态说明**：
   - `ttzip-archive` 生产库目标（`lib`）已在无豁免下编译通过并链接至上述 13 项集成测试；
   - 执行 `cargo test --manifest-path products/ttzip/core/rust/Cargo.toml -p ttzip-archive --lib -- zip` 时，因其他组正在修改的范围外测试文件 `crates/ttzip-archive/src/fs/tests_safe_extract.rs:294:32` 存在未收口字段（`missing field creation_mode_mask in initializer of StagedMetadataPlan`），按指令不拼接源码、不越界修改，留待 H 集成后统一复测。

3. **代码质量与行数门禁核查**：
   - 使用 `tools/ci/lint_code_quality.py` 对 R3 全部 12 个文件扫描：所有文件行数均 `<= 800` 行，所有新增/修改函数行数均 `<= 80` 行（`parse_cdfh_entry` 75 行，`reader_stream.rs` 最大函数 71 行，两个测试文件最大函数分别为 66 行与 76 行），圈复杂度均 `<= 15`，无新增 `#[allow(...)]`。
   - `ZipEntry` 为 `ttzip-archive` 内部 Rust 结构体（非 `#[repr(C)]`、未暴露给 C-ABI 或 UniFFI），且在 `zip/parser.rs` 之外无任何结构体字面量构造点，无需跨范围交接补丁。

### 5.2 独立工具版本与交叉验证结果

- **Info-ZIP Zip**：`Zip 3.0 (July 5th 2008)`（`/usr/bin/zip`）—— 创建含历史时间戳与 `-P` ZipCrypto 归档 -> TTZip 提取：时间戳、内容完全一致（PASS）。
- **Info-ZIP UnZip**：`UnZip 6.00 of 20 April 2009`（`/usr/bin/unzip`）—— 提取 TTZip 创建的未加密与 ZipCrypto 归档：`unzip -t -P <pw>` 与解压内容、时间戳完全一致（PASS）。
- **7-Zip**：`7-Zip (z) 26.00 (arm64)`（`/opt/homebrew/bin/7z`）—— 双向交叉验证 ZipCrypto（含 `-mem=ZipCrypto`）与历史时间戳归档：全部通过（PASS）。

---

## 6. 最终交付四项清单

### 6.1 已完成
1. 全面审核并修复 `parallel_batch.rs`、`streaming_sink.rs` 及原位编辑 `archive/in_place_edit/zip.rs` 中 `(dos_time, dos_date)` 反序解构问题，并在所有创建及原位编辑路径补齐 `0x5455` Extended Timestamp。
2. 统一 `datetime.rs` 的 `DosDateTime` 本地日历时间（`localtime_r` / `mktime`）转换与 `0x5455` > `0x000a` > DOS 本地时间三级解析优先级，同步应用到 `parser.rs`、`reader.rs` 与 `zip_central_directory.rs`。
3. 修复 `reader.rs` 与 `reader_stream.rs` 将所有加密条目硬编码为 WinZip AES-256 的问题，支持 `compression_method != 99` 的 Traditional ZipCrypto 内存与流式提取，并修复 `parallel_batch.rs` 流式 Data Descriptor 下 ZipCrypto check byte 误用 `dos_date` 的问题。
4. 在 `ZipEntry` 中保留经 `is_valid_winzip_aes_extra` 校验的 `aes_version`，并在 `reader.rs` 与 `reader_stream.rs` 中校验 LFH/CDFH 的 `0x9901` WinZip AES extra 一致性；`should_verify_entry_crc32` 严格区分 **AE-1**（`aes_version == 1`，即使声明 `crc32 == 0` 也强制执行 CRC32 校验）与 **AE-2**（`aes_version == 2`，按 AE-2 契约跳过 CRC32 并保留 PVV + HMAC-SHA1-80 认证校验），不再仅凭 `method == 99 && crc32 == 0` 推断 AE-2。
5. 严格隔离 `map_zipcrypto_decode_err` 错误语义：仅将解密后流损坏/CRC 不匹配的 `ErrCorruptHeader` 映射为 `ErrInvalidPassword`，完整保留 `ErrSecurityViolation`、`Cancelled`、`ErrExtractionFailed` 与 `ErrOutOfMemory`。
6. 按职责将 R3 回归测试拆分为 `r3_zip_timestamp_crypto_regression_tests.rs`（721 行）与 `r3_zip_crc_and_aes_version_regression_tests.rs`（650 行），补充合法 AE-1 / AE-2、非法 `0x9901` extra 以及“有效加密负载与认证、但 AE-1 声明 CRC 错误为零”的内存与流式对照测试，同时保留既有未加密 / ZipCrypto 零 CRC 测试。

### 6.2 已验证
- 当前工作区无豁免直接执行 `cargo test --manifest-path products/ttzip/core/rust/Cargo.toml --test r3_zip_timestamp_crypto_regression_tests --test r3_zip_crc_and_aes_version_regression_tests`：两个测试套件共 13 项集成测试全部通过（`6 passed` + `7 passed`，`0 failed`）。
- 与独立外部工具（`Zip 3.0`、`UnZip 6.00`、`7-Zip 26.00`）双向交叉验证通过。

### 6.3 尚未验证
- `cargo test --manifest-path products/ttzip/core/rust/Cargo.toml -p ttzip-archive --lib -- zip`：当前因其他组范围外文件 `crates/ttzip-archive/src/fs/tests_safe_extract.rs:294` 尚未收口，待 H 集成后复测。
- 全仓门禁 `ttaction run` 与 `bazel test -c opt //...`（按指令不在 R3 单组内自行运行）。
- 新候选 `.app` 包上的正确密码、时间戳和原位编辑端到端复验（交 Q1 在新包上复验，当前不关闭对应问题）。

### 6.4 依赖其他组
- 依赖其他组完成 `crates/ttzip-archive/src/fs/tests_safe_extract.rs` 的 `StagedMetadataPlan` 结构体初始化收口，并由 **H** 在各组停笔后统一执行全仓 `ttaction run` 与打包。
- 依赖 **Q1** 在新候选包上复验正确密码、时间戳和原位编辑。


