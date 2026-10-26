# R2 统一浏览、单文件流与选中提取路由修复（设计与交接记录）

- **日期**：2026-10-17
- **责任组**：R2（Unified Browse, Single-File Stream & Selected-Entry Extraction Routing）
- **覆盖问题单**：
  - `Q1-ISSUE-02`：单卷加密 `.7z`（`c3_enc.7z`）与单卷 `.tar.br`（`c1_tarbr.tar.br`）选中提取失败。
  - `Q1-ISSUE-05`（Rust 读取侧）：单卷 `.tar.br` 浏览仅返回 1 条虚拟记录、单文件压缩流（`.gz`/`.bz2`/`.xz`/`.zst`/`.lz4`/`.br`/`.lz`/`.lrz`/`.sz`）无法浏览或会话解压、逻辑条目名（`"data"`）与落盘文件名（`<stem>`）不一致、以及 `archive_read_support_format_raw` 无条件启用导致的非归档假阳性。
  - `Q1-ISSUE-06`（读取侧）：真实外部 `DMG`、`AAR`、`XAR`、`WIM` 样本的识别、浏览、提取支持路径与错误分类收敛。

---

## 1. 读取路由五项核心契约（Read Routing Contracts）

1. **契约 1 — 单卷与分卷统一按实际格式选择原生解码路径**：
   - `SevenZip`（`.7z` / `.7z.001`）：无论单卷（`open_single_volume`）还是分卷（`MultiVolumeReader::open`），在 `inspect_archive`、`extract_archive_with_metrics`、`extract_single_entry_memory`、`extract_selected_entries` 四条入口统一路由至原生 `sevenz` 解码管线（`EncodedHeaderDecoder` + `extract_7z_to_sink_from_reader` / `extract_7z_single_entry_from_seekable` / `extract_sevenz_selected_from_reader`），不再把单卷 `.7z` 选中提取落入缺少 AES-256 解密能力的 `libarchive` 兜底分支。
   - `Brotli`（`.tar.br` / `.br`）与 `Snappy`（`.sz`）：无论单卷还是分卷，统一通过 Rust 原生 `brotli::Decompressor` 与 `snap::read::FrameDecoder` 解压，不再把单卷 `.tar.br` 直接塞入不支持 Brotli filter 的 `libarchive`。
2. **契约 2 — 浏览、整包提取、选中提取共享一致的逻辑条目身份**：
   - 对于纯单文件压缩流（`.gz`/`.bz2`/`.xz`/`.zst`/`.lz4`/`.br`/`.lz`/`.lrz`/`.sz`），`inspect_archive`、`extract_archive_with_metrics`、`extract_selected_entries`（含 `ExtractionSession`）与 `extract_single_entry_memory` 全部使用 `derive_single_stream_entry_name(archive_path)` 派生的 `<stem>`（例如 `single_payload.bin.gz` -> `"single_payload.bin"`）作为唯一逻辑条目路径。
   - 彻底消除此前 `libarchive` `raw` 模式下浏览返回 `"data"`、整包提取重写为 `<stem>`、而会话/选中提取因查找 `<stem>` 或写出 `"data"` 导致失败或文件名错乱的不一致问题。
3. **契约 3 — 纯压缩流与 TAR 压缩容器通过 512B 解压前瞻严格区分**：
   - `try_open_resolved_stream` 打开任意确认的压缩流（原生 `Brotli`/`Snappy`/`Lzfse` 或 `LibarchiveRawFilterReader` 驱动的 `Gzip`/`Bzip2`/`Xz`/`Zstd`/`Lz4`/`Lzip`/`Lrzip`）后，预读解压后的前 512 字节并检查 POSIX USTAR / GNU TAR 魔数（偏移 `257..262 == b"ustar"`）。
   - 若命中 TAR 头且非显式复合后缀冲突，则用 `PreReadStream` 无损拼回已读前缀并包装为 `ResolvedStream::TarContainer`，交由 `LibarchiveStreamReader::open_sequential` 执行内部 TAR 层级浏览、整包提取或选中提取（支持文件与目录前缀选中）。
   - 若前 512 字节非 TAR 头：对显式声明为 `.tar.<ext>` / `.tgz` 等复合扩展名的损坏文件直接返回 `TTZipStatus::ErrCorruptHeader`；对纯单文件流扩展名包装为 `ResolvedStream::SingleFileStream { entry_name, reader }`。
4. **契约 4 — `raw` 格式仅限受控压缩流 filter 使用，严禁接受任意未知或损坏文件**：
   - 从 `LibarchiveStreamReader::open_sequential` 与 `open_seekable` 中移除无条件的 `archive_read_support_format_raw(a)`。
   - 新增受控的 `LibarchiveStreamReader::open_raw_filter`，仅在 `classify_stream_codec` 确认文件具备合法压缩流魔数或受支持流扩展名时调用，并在读取虚拟条目及 EOF 时强制断言 `archive_filter_count(a) >= 2`（必须至少挂载 1 个真实解压 filter，拒绝 `ARCHIVE_FILTER_NONE` 原样透传）。
   - 任意未知二进制（如 `not_an_archive.bin`）或截断/伪造的 `.gz`..`.sz` 文件在浏览与提取时均稳定返回错误（`ErrCorruptHeader` / `ErrExtractionFailed`），绝不产生名为 `"data"` 或 `<stem>` 的假阳性文件。
5. **契约 5 — 真实格式四态明确区分（识别成功 / 可浏览 / 可提取 / 明确不支持 vs 损坏）**：
   - 明确不支持的容器或特性返回 `TTZipStatus::ErrUnsupportedFeature`（并在 `fallback_policy::should_abort_without_fallback` 中短路，不降级为 `ErrCorruptHeader`）；结构损坏的文件返回 `TTZipStatus::ErrCorruptHeader`。

---

## 2. 问题根因与修复详情

### 2.1 `Q1-ISSUE-02`：单卷加密 `.7z` 与单卷 `.tar.br` 选中提取失败
- **根因**：
  1. `extract_single.rs` 中 `extract_selected_entries` 与 `extract_single_entry_memory` 仅对 `MultiVolumeSet::detect(archive_path).is_some()` 的分卷归档调用了原生 `sevenz` 和 `.tar.br` 解码分支；单卷 `.7z` 与单卷 `.tar.br` 直接落入了末尾的 `LibarchiveStreamReader::open_file`，而内置 `libarchive` 既不支持 7z AES-256 解密，也不支持 Brotli filter。
- **修复**：
  1. 在 `container_bridge.rs` 中提取出统一的 `extract_7z_single_entry_from_seekable` 与 `extract_sevenz_selected_from_reader`，无论单卷 `File` 还是分卷 `MultiVolumeReader`，只要 `detect_format` 识别为 `DetectedFormat::SevenZip`，一律走原生 `EncodedHeaderDecoder` + `sevenz::decoder` 路径。
  2. 在 `extract_single.rs` 中通过 `try_open_resolved_stream` 统一处理单卷与分卷 `.tar.br` 及其他压缩 TAR 流，交由 `LibarchiveStreamReader::open_sequential` 执行流式选中提取与单条目内存提取。

### 2.2 `Q1-ISSUE-05`（读取侧）：`.tar.br` 浏览仅 1 条目 & 单文件压缩流全链路打通
- **根因**：
  1. `inspect.rs` 仅在 `MultiVolumeSet::detect` 分支处理了 Brotli，单卷 `c1_tarbr.tar.br` 落入 `LibarchiveStreamReader::open_file`，在旧代码的 `archive_read_support_format_raw` 下被当成未解压的 1 个 `"data"` 原始字节流。
  2. 纯单文件压缩流（`single_payload.bin.gz`..`.sz`）在 `inspect_archive`、`ExtractionSession`（`preflight` -> `stage` -> `commit`）和 `extract_single_entry_memory` 中没有统一的流式适配器。
- **修复**：
  1. 新增 `crates/ttzip-archive/src/archive/stream_adapter/single_stream.rs`，实现 `ConfirmedStreamCodec`、`LibarchiveRawFilterReader`、`PreReadStream`、`try_open_resolved_stream` 以及单文件流的 `inspect_single_file_stream`、`extract_single_file_stream`、`extract_selected_single_file_stream`、`extract_memory_single_file_stream`。
  2. 在 `inspect.rs`、`extract.rs`、`extract_single.rs` 中统一接入 `try_open_resolved_stream`，使 `.tar.br`（单卷与分卷）稳定列出内部全部 22 个 TAR 条目并支持选中提取，9 种纯单文件流（`.gz`/`.bz2`/`.xz`/`.zst`/`.lz4`/`.br`/`.lz`/`.lrz`/`.sz`）全链路通过 `inspect` -> `ExtractionSession::preflight/stage/commit` -> `extract_archive_with_metrics` -> `extract_single_entry_memory`。

### 2.3 `Q1-ISSUE-06`（读取侧）：真实 `DMG`、`AAR`、`XAR`、`WIM` 支持路径与独立工具对账
- **根因**：
  1. Bazel 静态构建的 `//third_party/libarchive` 未链接 `libxml2`，导致 `libarchive` 内置的 XAR 与 WIM reader 在固定包（以及不带 `libxml2` 的构建）中失效，且 0 条目合法 XAR（`ext_created.xar`）因 `entries == 0` 被 `inspect.rs` 误判为 `ErrCorruptHeader`。
  2. `libarchive` 无 Apple Archive（`.aar`：`pbze` 并行 LZFSE + `AA01`/`YAA1`）reader，也无 macOS UDIF DMG（尾部 512 字节 `koly` 块 + 分区表/HFS+/APFS）reader；旧代码因开启了 `format_raw`，将 `.dmg`/`.aar`/`.wim`/`.xar` 整个容器文件当作单文件 `"data"` 原样拷出，产生假阳性。
- **修复**：
  1. 在 `detect.rs` 中为 `detect_archive_sniff` 与 `detect_format` 补齐 `pbze`/`AA01`/`YAA1`/`AEA1`（`DetectedFormat::Aar`）与 UDIF 尾部 512B `koly` v4 签名（`DetectedFormat::Dmg`）识别。
  2. 在 `container_bridge.rs` 与 `xar_wim_bridge.rs` 中实现零外部 C 依赖的纯 Safe Rust 容器转码桥（转码为内存 POSIX USTAR 流供全链路 `inspect` / `extract` / `extract_selected` / `ExtractionSession` 复用）：
     - **`AAR`（Apple Archive）**：原生解析 `pbze` 并行 LZFSE 分块流与 `AA01`/`YAA1` 字段头（含 `PATP`、`TYP1`、`MOD2`、`MTM*`、`DAT*`、`LNK*`），完整支持浏览、整包提取与选中提取；对加密 `AEA1` 容器明确返回 `ErrUnsupportedFeature`。
     - **`XAR`（eXtensible ARchive）**：原生解析 `xar!` 28B 头部、zlib 压缩的 XML `<toc>`（含 `<name enctype="base64">` UTF-8 CJK/Emoji 文件名解码、嵌套 `<file>` 树、gzip/zlib/bzip2/none 堆区 extent 解码），同时在 Cargo 与无 `libxml2` 的 Bazel 构建下工作，并正确支持 0 条目合法 XAR（`ext_created.xar`）。
     - **`WIM`（Windows Imaging Format）**：原生解析未压缩 `MSWIM\0\0\0`（v1.13，含 offset table 中 `RESHDR_FLAG_METADATA` 元数据定位、根目录对齐跳过安全描述符及 UTF-16LE `DIRENTRY`/`STREAMENTRY` 树）；对 LZX/XPRESS/LZMS 压缩或分卷 WIM 明确返回 `ErrUnsupportedFeature`。
     - **`DMG`（Apple UDIF Disk Image）**：验证尾部 512B `koly` v4 签名后明确返回 `ErrUnsupportedFeature`（解析 UDZO blkx + HFS+/APFS 文件系统需额外文件系统层支持）；非 UDIF 损坏 `.dmg` 返回 `ErrCorruptHeader`。

| 格式与 Q1 样本 | 外部独立 CLI 验证命令与结果 | TTZip 识别 (`detect`) | TTZip 浏览 (`inspect_archive`) | TTZip 整包与选中提取 (`ExtractionSession` / `extract_*`) | 状态说明 |
|---|---|---|---|---|---|
| **`AAR`** (`ext_created.aar`, 319B) | `/usr/bin/aa list -i ext_created.aar` -> `RC=0` (`hello.txt`, `sub`, `sub/nested.txt`) | `DetectedFormat::Aar` | `Ok(3)` (`hello.txt`, `sub/`, `sub/nested.txt`) | 整包与选中提取全部 `Ok`，内容与 `small_ext_src` 100% 匹配 | 纯 Rust `pbze` (LZFSE) + `AA01`/`YAA1` 原生支持 |
| **`XAR`** (`ext_created.xar`, 209B) | `/usr/bin/xar -tf ext_created.xar` -> `RC=0` (0 条目合法空归档) | `DetectedFormat::Xar` | `Ok(0)` | `Ok(0)`（不报错、不泄漏文件） | 纯 Rust `xar!` + zlib XML `<toc>` 原生支持（不依赖 `libxml2`） |
| **`XAR`** (`c1_xar.xar`, 26,432B) | `/usr/bin/xar -tf c1_xar.xar` -> `RC=0` (22 条目，含 Base64 CJK/Emoji 路径) | `DetectedFormat::Xar` | `Ok(22)` | 整包、选中与内存提取全部 `Ok`，7/7 文件内容与 SHA-256 匹配 | 支持 `<name enctype="base64">` 与嵌套目录树 |
| **`WIM`** (`ext_created.wim`, 1,713B) | `/opt/homebrew/bin/wimlib-imagex dir ext_created.wim` -> `RC=0` (`/`, `/hello.txt`, `/sub`, `/sub/nested.txt`) | `DetectedFormat::Wim` | `Ok(3)` (`hello.txt`, `sub/`, `sub/nested.txt`) | 整包与选中提取全部 `Ok`，内容与 `small_ext_src` 100% 匹配；压缩 WIM 返回 `ErrUnsupportedFeature` | 纯 Rust 未压缩 `MSWIM` 原生支持；LZX/XPRESS 压缩 WIM 明确报 `ErrUnsupportedFeature` |
| **`DMG`** (`ext_created.dmg`, 19,956B) | `/usr/bin/hdiutil imageinfo -format ext_created.dmg` -> `RC=0` (`UDZO`) | `DetectedFormat::Dmg` | `Err(ErrUnsupportedFeature)` | `Err(ErrUnsupportedFeature)`（损坏 `.dmg` 返回 `ErrCorruptHeader`） | 识别 UDIF `koly` trailer 并明确返回不支持，不再伪装成 raw `"data"` 或报损坏 |

---

## 3. R2 修改与新增文件清单及 SHA-256 哈希

所有修改严格限制在 R2 独占白名单内，且全部通过 `python3 tools/ci/lint_code_quality.py` 与 `python3 tools/ci/lint_loc_gate.py`（单文件 $\le 800$ 行、单函数 $\le 80$ 行、圈复杂度 $\le 15$，0 编译告警）：

| 文件路径 | 行数 | SHA-256 | 说明 |
|---|---:|---|---|
| `products/ttzip/core/rust/crates/ttzip-archive/src/archive/unified/inspect.rs` | 631 | `de4016758a3f35a9777c99b85c9f9c383158dd2b467d2b04dedd05ea037f4c9b` | 统一单卷/分卷特殊容器、TAR 压缩流与纯单文件压缩流浏览路由 |
| `products/ttzip/core/rust/crates/ttzip-archive/src/archive/unified/extract.rs` | 775 | `31925ae3c18db71e0cddf68fc5d2a5394aa6248ccbb624469420207449ef1c6e` | 统一单卷/分卷特殊容器、TAR 压缩流与纯单文件压缩流整包提取路由 |
| `products/ttzip/core/rust/crates/ttzip-archive/src/archive/unified/extract_single.rs` | 784 | `79b0adf29f7ce792b9186cd293ab9a9e5463df3adeed7a5cf3c1ba82a22341ab` | 统一单卷/分卷 7z、ZIP、特殊容器、TAR 流与单文件流的选中提取及单条目内存提取 |
| `products/ttzip/core/rust/crates/ttzip-archive/src/archive/unified/detect.rs` | 251 | `8495ba52fbdadd86aa8cf4da4253b5ef9f514dcf8d6cba98645b51fecfcd450c` | 补齐 `pbze`/`AA01`/`YAA1`/`AEA1` (`Aar`)、UDIF `koly` (`Dmg`) 及分卷后缀剥离识别 |
| `products/ttzip/core/rust/crates/ttzip-archive/src/archive/unified/fallback_policy.rs` | 87 | `310e81eaceb5a7947a11106a90396a56524d62a977c66efe38cef6fdbd6e6c31` | 将 `ErrUnsupportedFeature` 纳入短路策略，防止被 fallback 覆盖为 `ErrCorruptHeader` |
| `products/ttzip/core/rust/crates/ttzip-archive/src/archive/stream_adapter/mod.rs` | 59 | `d0612019481ef1f702d623b4e257fa99e46545bae7f84ec9a5362b7055ee7014` | 导出 `container_bridge`、`single_stream`、`xar_wim_bridge` |
| `products/ttzip/core/rust/crates/ttzip-archive/src/archive/stream_adapter/read.rs` | 352 | `ef3220cc2037b37110c0efae8144e57be71ac9c852d61f24e90010c103eeb621` | 移除无条件 `format_raw`；新增受控 `open_raw_filter` 与 `verify_underlying_eof` |
| `products/ttzip/core/rust/crates/ttzip-archive/src/archive/stream_adapter/single_stream.rs` | 668 | `aa84cd883e999f84af1024e9d5d1b809b5d4af9e404cc4498998967a8f20099a` | 512B 前瞻区分 `TarContainer` 与 `SingleFileStream`，实现 9 种纯压缩流全链路读取 |
| `products/ttzip/core/rust/crates/ttzip-archive/src/archive/stream_adapter/container_bridge.rs` | 600 | `e528658e2603c53a714c50d4b5e8ba6f64349777a272f4f224d053ad8624d7f8` | 实现 `AAR` (`pbze`+`AA01`/`YAA1`) 转码、`DMG` 识别拦截与通用 7z 选中提取辅助函数 |
| `products/ttzip/core/rust/crates/ttzip-archive/src/archive/stream_adapter/xar_wim_bridge.rs` | 470 | `0c5ff30824e0e0d9f1ef000d03a4756dfef3d0d36ae47baa5e18534e67754efc` | 实现纯 Rust `XAR`（含 Base64 UTF-8 路径与 0 条目支持）与未压缩 `WIM` USTAR 转码器 |
| `products/ttzip/core/rust/ttzip-engine/tests/r2_archive_read_routing_regression_tests.rs` | 670 | `48b133260d64a43c9921ad8d49137ff47622899728455a9160e76c7a4613feea` | R2 独立专项回归测试套件（6 组测试覆盖 Q1 全部相关样本） |

---

## 4. 状态与跨组交接说明

### 已完成
1. 单卷与分卷加密/非加密 `.7z`（`c1_7z_normal.7z`、`c3_enc.7z`、`ext_7z_mhe_on.7z`、`ext_7z_mhe_off.7z`）的浏览、整包提取、选中提取（`extract_selected_entries` 与 `ExtractionSession`）、单条目内存提取（`extract_single_entry_memory`）及错误/缺失密码零泄漏拒绝。
2. 单卷与分卷 `.tar.br`（`c1_tarbr.tar.br`）内部 22 个 TAR 条目的层级浏览、文件与子目录选中提取、单条目内存提取。
3. 9 种纯单文件压缩流（`single_payload.bin.gz`/`.bz2`/`.xz`/`.zst`/`.lz4`/`.br`/`.lz`/`.lrz`/`.sz`）的浏览、整包提取、`ExtractionSession`（全量与选中条目）及单条目内存提取，逻辑条目名与落盘文件名统一为 `"single_payload.bin"`（43,008B）。
4. 移除无条件 `archive_read_support_format_raw`，消除任意未知文件或容器被当作 `"data"` 原样拷出的假阳性。
5. 真实外部 `AAR`（`ext_created.aar`）、`XAR`（`ext_created.xar` 与 `c1_xar.xar`）、`WIM`（`ext_created.wim`）的纯 Rust 原生读取、浏览与选中提取支持，以及 `DMG`（`ext_created.dmg`）的 `ErrUnsupportedFeature` 明确分类。

### 已验证
1. **R2 专项回归测试（源码测试）**：
   - 命令：`cargo test --manifest-path products/ttzip/core/rust/Cargo.toml -p ttzip-engine --test r2_archive_read_routing_regression_tests -- --nocapture`
   - 结果：`6 passed; 0 failed`（0 告警）。
2. **代码质量与行数门禁**：
   - 命令：`python3 tools/ci/lint_code_quality.py` 与 `python3 tools/ci/lint_loc_gate.py`
   - 结果：R2 全部文件 0 项文件行数（$\le 800$）、函数行数（$\le 80$）与圈复杂度（$\le 15$）违规。

### 尚未验证
1. **Bazel 全量与固定候选包复验**：按并行执行约束，R2 未运行全仓 `ttaction run`、未重建旧验收包，待四组全部停笔后由 **H 组** 统一集成打包并交 **Q1** 复验。
2. **`products/ttzip/core/rust/ttzip-engine/BUILD.bazel` 测试发现说明**：`ttzip-engine/BUILD.bazel` 使用 `glob(["tests/*_tests.rs"])` 自动发现集成测试，因此 `r2_archive_read_routing_regression_tests.rs` 无需修改 `BUILD.bazel` 即可被 Bazel 自动纳管（注：该测试读取 `/tmp/ttzip-q1-acceptance-20261017` 下的复制样本，若在 Bazel 沙箱内运行需由 H 决定是否挂载或仅在本地 Cargo 专项验证运行）。

### 依赖其他组
1. **依赖 R4（创建契约与 Swift 映射）**：
   - `products/ttzip/core/rust/crates/ttzip-archive/src/archive/unified/tests_lifecycle.rs:312` 中旧测试对目录输入调用 `create_archive(..., Snappy)` 因 R4 收紧单流创建契约（单文件流拒绝多文件/目录输入）而报错，该测试与 `create.rs` / `format_config.rs` 归属 R4 协调更新。
   - `C2-11`（`ext_created.ar`）：实测 `/tmp/ttzip-q1-acceptance-20261017/samples/external_archives/ext_created.ar`（96B）在生成时仅写入了 BSD `ar` 空符号表头 `"__.SYMDEF SORTED"`（无 `hello.txt` 成员字节），属样本生成脚本或 R4 `Ar` 只读能力边界范畴。
2. **依赖 R1（7z 大文件流式解码）**：
   - `c5_large.7z`（14.4MB Fast LZMA2）的解码修复归属 R1（`sevenz/decoder/`），R2 的 `inspect.rs` / `extract.rs` / `extract_single.rs` 已统一复用 R1 的 `sevenz::decoder` 入口。
