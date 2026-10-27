# 自有 7z 合规语料替换契约

> 以下保留 MM42 离线制作阶段的来源与断言记录。当前切片只固化自有语料、生成配方与真实合规测试；生产解码修复由独立 MM45 提交提供。此前 live 快照的 34 项合规验收不替代本提交与指定前置组合的干净候选验收。reader 型选中提取等产品路径不由本套 mapped 合规测试证明完成。

状态：18类自有archive全部完成离线制作与核验（含LZ4）；正式 Rust/Bazel 验收仍由主代理串行执行。旧测试不能再因缺样本被记为 passed。正式测试只消费仓内固定字节，不执行 CLI 或回退私人/vendor 目录。

| 原类 / 原断言 | 自有输入对应 | 必须验证的方法 |
|---|---|---|
| 01/02 一个/两个空文件 | 同名空文件与空流 flags | 无 payload coder；不伪称 LZMA |
| 03/04 单/双文件明确明文 | 保留原文本，固定文件名/数量 | LZMA |
| 05/06 solid / non-solid，3条目，3447/2654 bytes | 自有确定性二进制，保留数量/大小，增加完整字节比较与 folder 数差异 | LZMA2；solid 1 folder / non-solid 2 folders |
| 07 Copy 明文及4种损坏 | 保留原明文，Copy archive 作为CRC/magic/truncation负对照 | Copy |
| 08 Delta 明文 | 保留 aaaabbbbcccc | Delta + LZMA2 |
| 09 Delta/BCJ2 DAG，9000/7000/16000 bytes | 自有 branch/pattern 字节，保留大小并比较全内容 | Delta + BCJ2 + LZMA2 多输入图 |
| 10 6条目、462336-byte MZ payload/path lookup | 自有合成 branch bytes，不复制7za程序；改用明确的synthetic路径 | BCJ2 + LZMA2 |
| 11/12 ARM64/x86，357888/367104 bytes，MZ前缀 | 自有带分支指令模式的合成数据，不声称真实PE可执行 | ARM64 / BCJ + LZMA2 |
| 13 PPMd，11356-byte文本 | 自有文本，保留长度且比较全内容；不复制第三方许可证 | PPMd |
| 14 BZip2，两条明确文本 | 保留world/bar明文，按路径索引而非依赖外部排序 | BZip2 |
| 15 LZ4，11556-byte文本 | 自有文本，保留长度并比较全内容；不复制许可证 | LZ4真实coder（0x04f71104），缺工具硬失败 |
| 16 Brotli，11556-byte文本 | 自有文本，保留长度且比较全内容；不复制许可证 | BROTLI真实coder（0x04f71102），缺工具硬失败 |
| 17 AES data，129421-byte内容与CRC | 自有确定性内容，保留长度与CRC验证，更新CRC到新明文；另比全内容 | LZMA2 + 7zAES，明文header |
| 18 AES header+data，两文件2247/1417 bytes与CRC | 自有内容，数量/大小/CRC/全内容/文件名正对照 | 7zAES header + data |
| 密码负向 | 同份加密fixture先正确密码正对照，再错误/缺密码精确ErrInvalidPassword | 不接受任意解析错误冒充密码拒绝 |

制作工具是本机已安装7zz 26.02与7z p7zip17.05：标准方法（LZMA/LZMA2/PPMD/BZip2/BCJ/BCJ2/Delta/ARM64/7zAES）用7zz，LZ4与BROTLI用p7zip（其codec表含4F71104 LZ4、4F71102 BROTLI）。仅处理生成器自己在临时目录写出的明文。生成命令、版本、真实coderID/chain、所有archive/明文hash记录manifest。未构建产品、未下载、未读取用户文件。

非加密archive需要重制字节一致；加密archive的盐/IV由CLI随机生成，仓内字节一旦冻结以manifest为准，生成器check只验证固定ciphertext、正确密码、严格明文及负向，不将重新随机加密宣称字节可复现。

## LZ4 缺口：根因与闭环

上一轮把p7zip17.05生成的LZ4归档判为“工具不可用”，证据是它自己的 `7z t` 返回 E_FAIL。根因在上游实现，不在本仓代码：

- p7zip 的 7z LZ4 coder 走 zstdmt 的 LZ4MT 封装。上游 `C/zstdmt/lz4-mt_compress.c` 只在 `ctx->threads > 1` 时写 12 字节容器头（magic `0x184D2A50`、version 4、frame size），随后才是 LZ4 frame；单线程时这 12 字节不写，缓冲区里残留垃圾字节，输出流因此不是合法 LZ4MT 流。
- 单线程还把 `out.size` 留成 `LZ4F_compressFrameBound()` 的上界，于是无论输入多大都写出约 1 MiB 的零填充负载；这正是旧证据里“1 MiB 归档 + E_FAIL”的来源。
- 修复方式不是换文件名或换参数盲试，而是按上游条件用多线程编码：`-mmt=2`（排在基础参数 `-mmt=1` 之后，后者被覆盖）。此时头部正确写出，归档 281 字节，p7zip 自身 `7z t` 通过。

因此 LZ4 输入不再阻断：`lz4.7z` 由 p7zip 以 `-m0=LZ4 -mmt=2` 生成，明文 11556 字节自有文本。

独立核验（不止工具自证）：

- p7zip `7z t` 通过，`7z x -so` 解出的明文与自有明文逐字节相同。
- 从7z pack流里取出 12 字节 LZ4MT 头，校验 magic/version/frame size，再取出 LZ4 frame。
- `generate.py` 内置的独立 Python LZ4 frame/block 解码器解出同一明文（不复用 Rust 解码器）。
- 参考实现 `lz4` CLI 独立解码同一 frame，字节一致（CLI 另会校验 frame content checksum）。
- manifest 记录 `payload_verification`：`container=lz4mt-skippable`、`version=4`、`frame_bytes=159`、`decoded_bytes=11556`、两个独立解码器名称。

已知边界：LZ4MT 每个容器最多 1 MiB 输入（上游 `ctx->inputsize = 1024 * 1024`），本 fixture 11556 字节远小于该上限，因此 payload 只有一个容器；Rust 侧 `decode_lz4_raw_chunked` 只剥离一层 12 字节头，后续容器靠 LZ4 frame decoder 的 skippable-frame 分支处理。仓内 payload.rs 的 LZ4 解码函数已按逐字摘录隔离验证；引擎 crate 的完整链路（SevenZReader → payload.rs 派发 → 7z 流 CRC 校验）仍待主代理串行验证。

## 本轮接收状态

已闭环离线输入：01–18，共18份archive。`generate.py`（无参数）验证归档/明文CRC与SHA、实际folder coderID与bind graph、CLI完整解压；16份非加密archive在新临时目录重制后字节相同，2份AES archive保留固定ciphertext并做正确/错误密码验证。ARM64模式按4-byte对齐写入；Delta→BCJ2明文由branch-rich数据的逐字节积分构造，避免把两个filter只登记为名字。

LZ4已闭环并入manifest；旧 `blocked.json` 缺口登记随之上线移除，`test_fixture_15_lz4_input_blocker` 改为 `test_fixture_15_lz4`：除保留原来的条目数/长度/前缀断言外，新增“data 层 coder 集合恰为 {0x04F71104}”、明文 CRC32 与 SHA-256 断言，防止改名冒充codec。

Rust定位器使用编译期仓内include_bytes和manifest hash；未注册或缺失fixture硬失败，没有vendor/个人路径/CLI运行依赖。除原逐项断言外，新增统一默认测试对18份archive的全部文件名、大小、CRC、SHA与data coderID核对，solid/non-solid另断言1/2 folders。AES每个负向测试先对同一输入做正确密码全明文正对照，再精确要求ErrInvalidPassword（open或extract边界）；未容许corrupt/unsupported任意错误通过。

引擎侧待修事实（本切片不改生产代码）：7z快路径（`decoder/stream.rs` 的 `init_folder_stream_decoder`）只取folder中第一个非AES coder且不施加branch/delta滤镜，只有该coder不是LZMA/LZMA2/Copy时才返回 `ErrUnsupportedFeature` 并回退到 `payload.rs` 的DAG路径。因此Delta/BCJ/ARM64/BCJ2这5类多coder语料在Rust侧会取到滤镜前字节或直接解码失败——这是引擎缺陷，不是语料问题；本切片断言一律不放宽，证据与最小修复建议见 `/tmp/ttzip-sprint-hermetic-audit/sevenz-fastpath-filter-finding.md`，由主代理在生产解码路径上串行修复。

原4种损坏检查仍保留，next-header CRC反例补算start-header CRC，确保单独验证next-header而非先被start-header拒绝。离线7zz对四种变体均非零退出；Rust结果尚未运行。

共享BUILD无需新补丁：现有engine BUILD的TEST_DATA_FILES包含tests/fixtures/**，同时用于compile_data与data；COMMON_TEST_DEPS已有serde_json与sha2。源文件新增包含路径全部在该glob内。

验证：`python3 products/ttzip/core/rust/ttzip-engine/tests/fixtures/sevenz/generate.py`（无参数）通过并输出18份archive已核验；四种损坏archive CLI拒绝通过；独立审计脚本（不复用生成器helper，从归档与CLI重新推导事实）256项检查全部通过。另用 `rustc` 单独编译仓内源码（不经cargo、不占workspace target、不碰Bazel output base）：逐字摘录 `ttzip-archive` 的 `decode_lz4_raw_chunked` / `decode_reader_codec_chunked` / `emit_checked_chunk`，并以 `#[path]` 直接引入仓内 `ttzip-codecs` 的 LZ4 frame decoder 与 `ttzip-crypto` 的 xxhash32，对新fixture解出与自有明文逐字节一致的11556字节；两个反例控制确认其非恒绿——压缩块内翻转1字节与旧的单线程p7zip产物都被仓内错误契约判为 ErrCorruptHeader（后者正是缺少LZ4MT头）。引擎crate、Bazel目标与全量Rust suite均未构建或运行，正式suite需要主代理串行执行；其他18类若触发真实解码缺陷，也必须据证修复，不能放宽方法/明文断言。

图证据范围：manifest 的 `coder_graph_scope=main_streams` 表示Python从未压缩的7z主header直接解析到的data coders与bind pairs；AES-header项只能静态解析外层加密header coder，明确标成`encoded_header`。该项的data method由生成参数、CLI解密列表与完整解压支持，最终data coderID仍需Rust在正确密码解密后由默认manifest测试核对。没有把outer-header图冒充已独立观察到的data图。
