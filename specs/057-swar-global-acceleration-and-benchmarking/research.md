# Phase 0 Research: Global SWAR Acceleration & Pattern Matching

## Research Item R001: 64-bit SWAR ASCII Validation Optimization
* **Decision**: 在 `ttzip_detect_encoding_fast` 中采用 64-bit `(v & 0x8080808080808080ULL) == 0` 批量扫描。
* **Rationale**:
  1. 纯 ASCII 字符的最高位（bit 7）必为 0。
  2. 64-bit 整数包含 8 个字节，与掩码 `0x8080808080808080ULL` 执行按位与只需 1 个 CPU 周期。若结果为 0，说明连续 8 字节全为有效 ASCII，游标直接 `i += 8`。
  3. 比逐字节循环节省 87.5% 的分支预测判断。
* **Alternatives Considered**:
  * *方案 B (128-bit NEON `vmaxvq_u8`)*：每次加载 16 字节并检查最大值是否 `< 128`。否决理由：在归档文件名这种通常短于 256 字节的字符串上，NEON 向量指令的设置和跨域开销超过其收益；64-bit GPR 标量 SWAR 延迟更低。
* **Source**:
  * UTF-8 向量化验证规范与 simdjson / utf8proc 微架构白皮书。

---

## Research Item R002: Format Header Sniffing Scalar Optimization
* **Decision**: 在 `ttzip_detect_format_from_header` 中使用 32-bit / 64-bit 定宽整数常量比对代替 `memcmp`。
* **Rationale**:
  1. 4~6 字节的 `memcmp` 涉及标准库函数调用开销、参数入栈与循环展开分支。
  2. 采用 `memcpy(&u32, ptr, 4)` 或直接 32 位整数比对（如 `0xafbc7a37` 表示 `"7z\xbc\xaf"`）会被编译器直接优化为单条 `ldr w` + `cmp w` 指令，执行耗时降至 1~2 个时钟周期。
* **Alternatives Considered**:
  * *方案 B (保持标准库 `memcmp`)*：否决理由：在频繁遍历归档条目或流式探测时存在无谓的函数跳转开销。
* **Source**:
  * POSIX C 标准库未对齐内存访问与 Clang `-O3` 汇编内联规则。
