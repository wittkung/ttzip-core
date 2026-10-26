# R1 7z 流式解码排空修复设计与交接记录（Q1-ISSUE-01 / C5-02a）

## 1. 问题证据与根因分析

### 1.1 Q1 失败证据（`C5-02a`）
- **问题票据**：`/tmp/ttzip-q1-acceptance-20261017/reports/issue-tickets.md`（`Q1-ISSUE-01`）与 `/tmp/ttzip-q1-acceptance-20261017/reports/acceptance_results.json`。
- **样本特征**：`/tmp/ttzip-q1-acceptance-20261017/archives/c5_large.7z`
  - 压缩后总大小：`135,549` 字节（远小于 `CHUNK_SIZE = 4 MiB`）。
  - 归档内单文件 `large_16mb.bin` 未压缩大小：`14,417,920` 字节（约 `13.75 MiB`，跨越 4 个 `4 MiB` 块）。
  - 期望 SHA-256：`1ab15267018b8a2a2563fef2029d479dd0f6482210f50503598dbbec86195bfb`。
  - 外部 `7z t` 校验完全合法，而 TTZip CLI 与 Engine API 解压均报 `CorruptHeader`。

### 1.2 核心根因
1. **`payload.rs::decode_raw_payload_chunked` 把“压缩输入耗尽”等同于“解码完成”**：
   - LZMA2 分支使用 `while in_buf.pos < in_buf.size` 循环调用 `dstream.decompress_stream(&mut out_buf, &mut in_buf)`。当压缩输入仅 `135,549` 字节而解压输出达 `14,417,920` 字节时，第一次调用即消耗完全部输入（`in_buf.pos == in_buf.size`），但 `out_buf` 仅写满第一个 `4 MiB` 分块（`4,194,304` 字节），底层 `FL2_DStream` 返回 `remaining = 1` 表示内部字典/输出缓冲仍有待排空数据。因循环条件只看 `in_buf.pos < in_buf.size`，解码循环在输出 `4 MiB` 后提前退出，后续 `archive.rs` 因已写出字节数（`4 MiB`）小于期望长度（`14,417,920`）返回 `TTZipStatus::ErrCorruptHeader`。
   - LZMA1 分支同样使用 `while !payload_slice.is_empty()`，在输入耗尽但输出缓冲满 `4 MiB` 且未达 `is_end` 时提前退出。
2. **`stream.rs::stream_7z_folder_from_reader` 流式 Folder 读取循环同样未排空内部缓冲**：
   - 内层 LZMA2 循环使用 `while in_buf.pos < in_buf.size`，LZMA1 循环使用 `while !slice.is_empty() && !*finished`。当单次读取或最后一块 `read_buf` 被完全消费但解码器内部仍有超过 `4 MiB` 的待输出数据时，未能继续以空输入切片（或当前 `in_buf` 位置）泵出剩余数据直至 `remaining == 0` / `is_end == true` 或需要更多输入。
   - 此外，当外部 `7z` 生成 AES-256 + LZMA2 归档时，AES-CBC 16 字节对齐块末尾可能带有 `1..=15` 字节块填充；当 LZMA2 遇到结束标记 `CHUNK_FINAL` 返回 `remaining == 0` 时，流已完成，不应再对已完成流继续调用 `decompress_stream`（否则触发 `FL2_error_stage_wrong`）。
3. **外部 `7z` 多 Coder 顺序（`[LZMA2, AES]`）下 Folder 最终解压大小解析**：
   - 外部 `7z` 在加密归档中按 `[LZMA2, AES]` 排列 `folder.coders`，此时 `folder.unpack_sizes.last()` 对应 AES 输出（可能带对齐差异），真正的最终解压输出对应未被 `folder.bind_pairs` 绑定为输入的输出流索引。
4. **未对齐/超限/截断校验与半成品防护**：
   - 需要在每次向 `sink` 发射分块前检查 `total_decompressed + chunk.len() <= expected_unpack_size`（当 `expected_unpack_size > 0`），并在正常结束（非选择性提取 `TTZipStatus::Eof` 早退）时校验 `total_decompressed == expected_unpack_size` 及流结束标志，防止截断流、无进展死循环或输出超限泄露半成品。

---

## 2. 设计判断与实现方案

### 2.1 四状态明确区分（不修改底层 `ttzip-codecs` FL2 封装或公共接口）
在 `stream.rs` 中实现并向 `payload.rs` 共享统一的流式排空引擎 `pump_lzma2_stream` 与 `pump_lzma1_stream`：
1. **流结束（Stream Finished）**：
   - LZMA2：`dstream.decompress_stream` 返回 `remaining == 0`；先发射本轮产出的数据块，随即返回 `Ok(true)`。
   - LZMA1：`decoder.decompress_stream` 返回 `is_end == true`；先发射本轮产出的数据块，随即返回 `Ok(true)`。
2. **输出排空中（Draining Output）**：
   - 即使 `in_buf.pos == in_buf.size`（或 `slice.is_empty()`），只要上一轮 `produced > 0` 且流尚未结束（LZMA2 `remaining > 0` / LZMA1 `!is_end`），继续循环调用 `decompress_stream` 排空解码器内部状态。
3. **需要更多输入（Need More Input）**：
   - 当 `in_buf.pos == in_buf.size`、本轮 `produced == 0` 且 `is_final_input == false` 时，返回 `Ok(false)`，通知外层 `stream_7z_folder_from_reader` 读取下一个压缩分块。
4. **截断或无进展错误（Truncated / No-Progress Error）**：
   - 当本轮 `consumed == 0 && produced == 0`（输入未耗尽但解码器无进展，或输入已耗尽且 `is_final_input == true` 但流仍未结束）时，立即返回 `Err(TTZipStatus::ErrCorruptHeader)`，杜绝无限循环。

### 2.2 统一覆盖两个解码入口与 Folder 大小解析
- `payload.rs`（`decode_raw_payload_chunked` / `decode_7z_folder_streaming` / `decode_7z_solid_streaming`）与 `stream.rs`（`stream_7z_folder_from_reader` / `extract_single_entry_bounded` / `extract_7z_single_entry_from_reader`）共用同一套 `pump_lzma2_stream`、`pump_lzma1_stream` 与 `emit_checked_chunk`。
- `payload.rs` 新增 `folder_final_unpack_size` 与 `resolve_folder_stream_size`，通过 `folder.bind_pairs` 定位最终未绑定输出流索引，在 `payload.rs`、`stream.rs`、`archive.rs`、`solid_stream.rs` 中统一解析 Folder 最终解压字节数。
- `archive.rs` 在最后一个待提取文件写满后若仍收到多余解压字节（`ensure_no_excess_stream_bytes`），立即丢弃 `cur_handle`（触发 `InFlightJailFile::Drop` 清理临时文件）并返回错误，不在目标目录留下半成品文件。
- 保持 `CHUNK_SIZE = 4 * 1024 * 1024`（4 MiB）与 `MAX_IN_MEMORY_DECOMPRESS_BYTES = 64 * 1024 * 1024`（64 MiB）不变，保持所有现有公开签名不变。

### 2.3 加密路径下“解码/流完整性错误”与“输出端（Sink）错误”严格区分
- **问题分析**：
  - 流式解码函数（`decode_7z_folder_streaming`、`stream_7z_folder_from_reader`、`pump_lzma2_stream`、`pump_lzma1_stream` 等）内部既调用底层解压器，也调用外部传入的 `sink(chunk)` 写入回调。
  - 底层 `Fl2DStream::decompress_stream` 与 `LzmaAloneDecoder::decompress_chunk` 在比特流解码失败时返回 `TTZipStatus::ErrExtractionFailed`，而 `decode_reader_codec_chunked` 原先也将解码读取错误映射为 `ErrExtractionFailed`；与此同时，目标文件写入失败时 `sink(chunk)` 同样返回 `ErrExtractionFailed`，路径安全拦截返回 `ErrSecurityViolation`。
  - 若 `map_folder_decompress_err` 在 `has_aes == true` 时简单把除少数控制流/内存状态外的所有错误一律转为 `ErrInvalidPassword`，会导致加密归档解压时的磁盘写入失败（`ErrExtractionFailed`）、安全策略拒绝（`ErrSecurityViolation`）、文件打开失败（`ErrOpenFailed`）等输出端故障被误报为密码错误。
- **设计方案**：
  1. **在解码器调用点归一化比特流错误（`map_codec_bitstream_err`）**：
     - 仅在底层编解码器自身执行解压操作（`Fl2DStream::decompress_stream`、`LzmaAloneDecoder::decompress_chunk`、`deflate_decompress`、`bzip2_decompress` 以及 `decode_reader_codec_chunked` 中的内存解码读取）失败时，将编解码器返回的 `ErrExtractionFailed | ErrCorruptHeader` 归一化为流损坏状态 `TTZipStatus::ErrCorruptHeader`（保留 `ErrOutOfMemory`、`ErrUnsupportedFeature` 等资源/特性状态）。
     - `sink(chunk)` 的返回值直接向上传递，**绝不**经过 `map_codec_bitstream_err`。
  2. **收紧 `map_folder_decompress_err` 的映射范围**：
     - 当 `has_aes == true` 时，`map_folder_decompress_err` **仅**将 `TTZipStatus::ErrCorruptHeader | TTZipStatus::ErrInvalidPassword`（即密码错误导致解密出伪随机字节进而引发的比特流损坏、解压长度不匹配或 CRC 校验失败）映射为 `TTZipStatus::ErrInvalidPassword`。
     - 来自输出端、安全边界、取消控制和资源限制的错误（`ErrExtractionFailed`、`ErrSecurityViolation`、`ErrOpenFailed`、`ErrPathTooLong`、`Cancelled`、`Eof`、`ErrOutOfMemory`、`ErrSolidBudgetExceeded`、`ErrUnsupportedFeature` 等）一律原样保留，不因“归档已加密”而篡改错误语义。

---

## 3. 修改文件清单与 SHA-256

| 文件路径 | 变更类型 | SHA-256 |
|---|---|---|
| `products/ttzip/core/rust/crates/ttzip-archive/src/sevenz/decoder/payload.rs` | 修改 | `b4f2ae420644c74d7c37f1610d7625b696b335a2c01b8eb768caed2a987fd434` |
| `products/ttzip/core/rust/crates/ttzip-archive/src/sevenz/decoder/stream.rs` | 修改 | `baf370b68d2bc7b5aa5a30f45d21d5b871c2231d15eb8f6ce8970f2a8a57c6e3` |
| `products/ttzip/core/rust/crates/ttzip-archive/src/sevenz/decoder/archive.rs` | 修改 | `d5101b1319e48b201341118c9641e4eed994bb629ab360b20acc4db55e39cf46` |
| `products/ttzip/core/rust/crates/ttzip-archive/src/sevenz/decoder/solid_stream.rs` | 修改 | `33ba2244c1113a7d1fbf3edb3e96facac39030ae771faaa6fcb8c3d45f9cc468` |
| `products/ttzip/core/rust/ttzip-engine/tests/r1_sevenz_streaming_regression_tests.rs` | 新增 | `2f9771e3dacee7ea85bcbeab46b95404196754c59a0ab9f5791e919214071483` |

> **Bazel 自动发现说明**：`products/ttzip/core/rust/ttzip-engine/BUILD.bazel` 使用 `glob(["tests/*.rs"])` 自动注册集成测试目标，新增 `r1_sevenz_streaming_regression_tests.rs` 无需改动共享 `BUILD.bazel`。

---

## 4. 验证命令与原始结果

### 4.1 行数与代码质量门禁（`lint_loc_gate.py` & `lint_code_quality.py`）
命令：
```bash
python3 tools/ci/lint_loc_gate.py --dir products/ttzip/core/rust/crates/ttzip-archive/src/sevenz/decoder --min-files 1
python3 tools/ci/lint_code_quality.py --dir products/ttzip/core/rust/crates/ttzip-archive/src/sevenz/decoder
```
原始输出：
```text
Scanning target roots: products/ttzip/core/rust/crates/ttzip-archive/src/sevenz/decoder (Ceiling: 800 LOC)
Scanned 7 source files (3,209 total physical LOC).
✅ LOC GATE PASSED: All 7 source files strictly conform to the <= 800 LOC gate.

Thresholds: File LOC <= 800 | Fn LOC <= 80 | McCabe CC <= 15
Scanned 7 files (3,209 physical LOC). Analyzed 88 functions.
ℹ️  Historical / Whitelisted Baseline: 3 fn LOC & 3 CC entries bypassed.
✅ QUALITY GATE PASSED: All source files and architecture invariants strictly conform to standards.
```

### 4.2 源码专项单元测试与集成回归测试
> **说明**：验证期间其他并行组（R2/R4 等）正在主工作树中编辑各自负责的 `archive/stream_adapter/`、`archive/unified/`（均归 R2）与 `sevenz/header/stream.rs`（归 R4）。为严格遵守不触碰白名单外文件的规定，同时验证 R1 当前源码哈希对应的真实行为，在 `/tmp` 隔离副本中将其他组未完编译文件对齐至 `HEAD` 基线并挂载 R1 上述 5 个文件（完全相同的 SHA-256）执行 `cargo test`，测试完毕后已清理 `/tmp` 临时构建目录。当前状态为：**隔离源码专项通过，合并门禁与固定包复验待完成**。

命令：
```bash
cargo test --manifest-path products/ttzip/core/rust/Cargo.toml -p ttzip-archive --lib sevenz
cargo test --manifest-path products/ttzip/core/rust/Cargo.toml -p ttzip-engine \
  --test r1_sevenz_streaming_regression_tests \
  --test sevenz_compliance_tests \
  --test sevenz_solid_stream_tests
```
原始输出：
```text
running 24 tests
test sevenz::decoder::payload::tests::test_decode_raw_payload_chunked_bzip2_exceeds_memory_limit ... ok
test sevenz::decoder::payload::tests::test_decode_raw_payload_chunked_deflate_exceeds_memory_limit ... ok
test sevenz::decoder::payload::tests::test_decode_7z_folder_streaming_single_coder_bomb_exceeds_memory_limit ... ok
test sevenz::decoder::solid_stream::tests::test_solid_folder_index_empty_header ... ok
test sevenz::decoder::stream::tests::test_extract_single_entry_rejects_offset_addition_overflow ... ok
test sevenz::varint::tests::test_buffer_too_small_error ... ok
test sevenz::format::tests::test_7z_varint_roundtrip ... ok
test sevenz::decoder::solid_stream::tests::test_solid_folder_index_multi_stream_offsets_and_lookup ... ok
test sevenz::decoder::stream::tests::test_extract_single_entry_bounded_rejects_short_output_without_crc ... ok
test sevenz::varint::tests::test_nid_constants_values ... ok
test sevenz::decoder::solid_stream::tests::test_solid_folder_index_backslash_and_slash_normalization ... ok
test sevenz::tests::test_7z_unsupported_codec_routing ... ok
test sevenz::varint::tests::test_streaming_io_primitives_roundtrip ... ok
test sevenz::format::tests::test_7z_signature_header_serialize_and_parse ... ok
test sevenz::decoder::stream::tests::test_extract_7z_single_entry_from_reader_rejects_short_output_without_crc ... ok
test sevenz::varint::tests::test_varint_basic_roundtrips ... ok
test sevenz::varint::tests::test_truncated_buffer_errors ... ok
test sevenz::decoder::solid_stream::tests::test_solid_early_exit_cancellation ... ok
test sevenz::tests::test_7z_multi_folder_full_archive_extraction ... ok
test sevenz::decoder::archive::tests::test_7z_extract_all_cancellation_with_sheet ... ok
test sevenz::tests::test_7z_multi_folder_parsing_and_seek_index ... ok
test sevenz::decoder::payload::tests::test_decode_raw_payload_chunked_brotli_passthrough ... ok
test sevenz::tests::test_7z_solid_roundtrip_lzma2_and_store ... ok
test sevenz::encrypted_header::tests::test_fixture_encrypted_aes256_7z ... ok
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 264 filtered out; finished in 0.01s

     Running tests/r1_sevenz_streaming_regression_tests.rs
running 6 tests
test test_multi_file_solid_block_crossing_4mib_and_selective_early_exit ... ok
test test_encrypted_archive_sink_fault_injection_preserves_error_semantics ... ok
test test_q1_c5_02a_large_high_ratio_7z_all_entrypoints ... ok
test test_truncated_corrupted_overrun_and_cancellation_safety ... ok
test test_incompressible_multi_chunk_and_encrypted_archives ... ok
test test_chunk_boundary_sizes_lzma2_and_lzma1 ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.37s

     Running tests/sevenz_compliance_tests.rs
running 33 tests
test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

     Running tests/sevenz_solid_stream_tests.rs
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.56s
```

---

## 5. 交付四项总结

> **整体状态**：隔离源码专项通过，合并门禁与固定包复验待完成。

### 5.1 已完成
1. 修复 `sevenz/decoder/payload.rs` 与 `sevenz/decoder/stream.rs` 中 LZMA2 / LZMA1 流式解码在压缩输入提前耗尽（`< 4 MiB`）但解压输出跨越多个 `4 MiB` 分块时提前终止的缺陷。
2. 明确区分“输入消耗”、“输出排空”、“流结束（`remaining == 0` / `is_end`）”与“需要更多输入”，并处理外部 `7z` AES-CBC 块对齐填充下的流结束判定与 `[LZMA2, AES]` Coder 顺序下的最终解压大小解析。
3. 在 `payload.rs` 与 `stream.rs` 中通过 `map_codec_bitstream_err` 与收紧的 `map_folder_decompress_err` 严格区分加密归档的“解码/流完整性错误”（映射为 `ErrInvalidPassword`）与“输出端（Sink）/安全/取消/资源限制错误”（保留 `ErrExtractionFailed`、`ErrSecurityViolation`、`Cancelled`、`ErrOutOfMemory`、`ErrSolidBudgetExceeded` 等原始状态码）。
4. 在 `payload.rs`、`stream.rs`、`archive.rs` 中补齐期望解压大小上限检查、正常完成时的完整长度一致性校验以及多余尾部字节拦截，确保截断、损坏、输出超限与中途取消均返回确定错误且不在磁盘留下半成品文件。
5. 新增独立回归测试 `products/ttzip/core/rust/ttzip-engine/tests/r1_sevenz_streaming_regression_tests.rs`（含加密归档 Sink 故障注入与错误密码对照测试）并更新本交接文档。

### 5.2 已验证
1. **Q1 `C5-02a` 原始失败样本（复制到独立临时目录）**：`c5_large.7z`（压缩 `135,549` 字节，解压 `14,417,920` 字节）通过 `SevenZArchive::extract_all_with_sheet`（1 线程与 4 线程）、`extract_7z_from_reader`、`extract_single_entry_bounded`、`extract_7z_single_entry_from_reader` 与 `SolidEarlyExitExtractor` 全入口验证，输出 SHA-256 均为 `1ab15267018b8a2a2563fef2029d479dd0f6482210f50503598dbbec86195bfb`。
2. **分块边界覆盖**：使用外部 `/opt/homebrew/bin/7z` 生成 LZMA2 与 LZMA1 样本，覆盖 `4 MiB - 1`、`4 MiB`、`4 MiB + 1`、`8 MiB`、`12 MiB + 37` 五种输出边界，全部解压哈希一致。
3. **多场景覆盖**：外部 `7z` 生成的高压缩率、不可压缩（压缩后 `> 4 MiB`）、多文件 Solid Block（总大小 `> 9 MiB` 全量解压 + 中间文件早退提取）、AES-256 加密（`-p` + `-mhe=on`）及 TTZip 自生加密样本全部验证通过。
4. **加密归档 Sink 故障注入与错误语义**：在 AES 加密归档流式解压（含首个分块与跨 `4 MiB` 后续分块）中注入 `ErrExtractionFailed`、`ErrSecurityViolation`、`Cancelled`、`ErrOutOfMemory`、`ErrSolidBudgetExceeded` 以及路径穿越安全拦截，均原样返回对应状态码；同一加密归档使用错误密码解压仍准确返回 `ErrInvalidPassword` 且不留残留文件。
5. **异常与取消安全**：截断流、损坏负载、声明大小小于实际解压输出（Output Overrun）、预取消及首个 `4 MiB` 分块写出后的中途回调取消均返回预期错误码，且目标目录无残留文件。
6. **门禁指标**：`sevenz/decoder` 单文件行数 `<= 800`、修改函数行数 `<= 80`、圈复杂度 `<= 15`，无新增编译告警或 Clippy 告警。

### 5.3 尚未验证
1. 尚未在 R2、R3、R4 全部停笔后的合并工作树上运行全仓 `ttaction run` 或 `bazel test -c opt //...`（按指令由 H 统一集成执行）。
2. 尚未对重新打包签名的 `TTZip.app` / `ttzip-cli` 固定产物包执行 Q1 端到端验收脚本复验（按指令由 H 打包、Q1 复验）。

### 5.4 依赖其他组
1. **无需修改底层 FL2 封装或公共接口**：本次修复完全在 `sevenz/decoder/` 内部通过 `Fl2DStream::decompress_stream` 的返回状态（`remaining` / `in_buf.pos` / `out_buf.pos`）完成，不依赖 `ttzip-codecs` 或公共类型变更。
2. **共享工作树编译汇合依赖 R2 / R3 / R4 完成各自当前编辑**：当前工作树中 `archive/stream_adapter/` 与 `archive/unified/`（均归 R2）、以及 `sevenz/header/stream.rs`（归 R4）存在并行编辑中的未完改动，待四组全部停止修改后由 H 统一运行全量编译与门禁。
