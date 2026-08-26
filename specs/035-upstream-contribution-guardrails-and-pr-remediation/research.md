# Technical Research Report: 上游开源贡献质量规范体系与 3 个 PR 严谨重构

> 对应工件：`specs/035-upstream-contribution-guardrails-and-pr-remediation/plan.md`  
> 调查执行者：`Libarchive Streaming C Safety Researcher` & `Libarchive Commit & Test Architecture Researcher`

---

## R001: 32-Bit / Multi-Arch Integer Truncation & Safe Clamping Patterns

### Decision
在将 `int64_t pack_stream_inbytes_remaining` 截断/赋给 `size_t to_read` 或 `bytes_in` 时，必须使用严格的 `(uint64_t)` 上限比较后向下强转模式：
```c
to_read = UBUFF_SIZE;
if ((uint64_t)zip->pack_stream_inbytes_remaining < (uint64_t)to_read)
    to_read = (size_t)zip->pack_stream_inbytes_remaining;
```

### Rationale
- 在 32 位平台（ARMv7、i386、MIPS32、RISC-V 32）上，`size_t` 仅为 32 位无符号整数（最大 4GB-1），而 `pack_stream_inbytes_remaining` 为 64 位有符号整数。
- 直接 `(size_t)pack_stream_inbytes_remaining` 在文件大于 4GB 时会发生高位截断，导致解密流读取长度突变为极小值甚至 0，引发死循环或数据损坏。
- 通过先设默认最大分块 `UBUFF_SIZE`（16KB），再在 `(uint64_t)` 空间内比较剩余大小，确保被强转的数值绝不超过 32 位表示范围，在 32 位与 64 位机器上均 100% 安全且零编译器警告。

### Alternatives Considered
- 使用标准宏 `(size_t)MIN(UBUFF_SIZE, zip->pack_stream_inbytes_remaining)`：**否决**。C 宏未显式处理 signed 64-bit 与 unsigned 32-bit 的类型提升，在部分严格编译模式（`-Wsign-compare`）下会触发编译警告或产生意外的符号扩展。

### Source
- `Vendor/libarchive-upstream/libarchive/archive_read_support_format_7zip.c:3634-3636`
- POSIX IEEE Std 1003.1 `stdint.h` / `sys/types.h`

---

## R002: Libarchive Streaming Read-Ahead & Consumption State Machine Invariants

### Decision
1. **Read Ahead 缓冲区请求**：统一使用 `__archive_read_ahead(a, 1, &bytes_avail)` 请求流式缓冲区，随后在得到实际可用长度 `bytes_avail`（断言 `buff_in != NULL && bytes_avail > 0`）后，再按 16 字节对齐计算 `aligned_in = ((size_t)bytes_avail / 16) * 16`。
2. **消费返回值捕获**：调用 `__archive_read_consume(a, aligned_in)` 必须严格断言返回值 `>= 0`：
```c
if (__archive_read_consume(a, aligned_in) < 0) {
    archive_set_error(&a->archive, ARCHIVE_ERRNO_FILE_FORMAT,
        "Failed to consume encrypted 7-Zip data");
    return (ARCHIVE_FATAL);
}
```

### Rationale
- `__archive_read_ahead(a, min, &avail)` 当请求的 `min`（如 16 字节）大于底层当前分块剩余可用字节时，解压层会被迫尝试通过内存拷贝拼接跨块数据。如果此时遇到 EOF 或损坏流，函数直接返回 `NULL`。
- 如果代码只检查 `bytes_avail <= 0`，当返回 `1 <= bytes_avail < 16` 时 `buff_in` 为 `NULL`，直接解引用会导致严重段错误。
- 传入 `min = 1` 是 libarchive 内部一贯的高性能与安全模式，既避免了强制内存拼接拷贝，又能确保只要流中有数据就一定返回非空有效指针。

### Alternatives Considered
- 强制每次传入 `min = 16` 并单独处理 `buff_in == NULL`：**否决**。强制要求 16 字节连续会导致跨 I/O 分块时触发不必要的内部 buffer reallocation 与 memcpy 拷贝，降低流式吞吐。

### Source
- `Vendor/libarchive-upstream/libarchive/archive_read_support_format_7zip.c:3616-3622`
- `Vendor/libarchive-upstream/libarchive/archive_read.c` (`__archive_read_ahead` 内部实现)

---

## R003: Atomic Commit Splitting Strategy for PR #3388

### Decision
将 PR #3388 的全量变更严格拆分为 3 个逻辑独立、各自可独立编译通过的原子 Commit：
1. **Commit 1: `[infra] cryptor: add AES-256-CBC symmetric encryption interface`**
   - 包含：`libarchive/archive_cryptor.c`、`libarchive/archive_cryptor_private.h`。
   - 作用：提供纯粹的跨平台加密/解密驱动中枢（CommonCrypto、OpenSSL、CNG、mbedTLS）。
2. **Commit 2: `[feat] 7zip: add AES-256-SHA-256 stream decryption pipeline`**
   - 包含：`libarchive/archive_read_support_format_7zip.c`。
   - 作用：挂载 AES 解密上下文至 7z 解包流，处理 Encoded Header 与分块解密。
3. **Commit 3: `[test] test: add regression test suite for 7zip AES decryption`**
   - 包含：`libarchive/test/test_read_format_7zip_encryption_*.c`、`Makefile.am`、`libarchive/test/CMakeLists.txt`。
   - 作用：注册 3 组加密回归测试。

### Rationale
- 满足 @stoeckmann 提出的核心要求，同时严格符合 Git Bisect 可二分查找准则。若后续在某个特殊编译环境出现构建问题，二分定位可精确识别是底层 Cryptor 宏定义问题还是 7z 业务逻辑问题。

### Alternatives Considered
- 维持单个 Monolithic Commit：**否决**。被 Reviewer 明确标记 Changes Requested。

### Source
- Upstream Review Feedback: `https://github.com/libarchive/libarchive/pull/3388#pullrequestreview-4944838528`

---

## R004: Libarchive Test Oracle Alignment & Public-API Integration Test Patterns

### Decision
1. **测试预言机对齐**：彻底移除 `test_archive_crc32.c` 中所有预计算的硬编码常量，统一调用 `test_utils.h` 中的 `bitcrc32(c, data, len)` 进行位级黄金校验。
2. **测试模式双层覆盖**：
   - 第一层：通过 `test_archive_crc32.c` 直接针对 `archive_crc32.h` 的 5 组边界输入（空输入、V.42 校验、分段增量、非对齐指针、大块 8 路循环展开）进行单元隔离验证。
   - 第二层：通过现有的 `test_write_format_zip_file.c`、`test_read_format_7zip.c` 等 public API 集成测试，验证实际格式打包与解包时的校验闭环。

### Rationale
- `bitcrc32()` 是 libarchive 自带的、绝对保证正确的纯位运算实现，不受任何硬件指令、编译器优化或外部库版本影响，是项目认可的唯一测试权威。
- 结合单元层面的硬件指令测试与公共 API 的 Zip 格式测试，既保证了 ARMv8 ACLE 硬件快速路径的绝对正确，又满足了 kientzle 对公共 API 测试体系的要求。

### Alternatives Considered
- 仅保留 Zip 公共 API 测试，彻底删除 `test_archive_crc32.c`：**否决**。Zip 测试无法精准覆盖 ACLE 的 8 路循环展开首尾非对齐字节边界，保留独立的 CRC32 单元测试能够提供更完备的白盒覆盖率。

### Source
- `Vendor/libarchive-upstream/test_utils/test_utils.h:35-36`
- `Vendor/libarchive-upstream/libarchive/test/test_write_format_zip_file.c:125-126`
- `Vendor/libarchive-upstream/libarchive/test/test_archive_crc32.c`
