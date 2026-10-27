# Feature Specification: Global 64-bit SWAR Acceleration & Benchmarking

## 1. Background & User Motivation
在 Feature 056 中，我们成功将 64-bit SWAR（SIMD-Within-A-Register）算法应用于 LZMA2 匹配查找器，将底层字符串比对吞吐从 2.5 GB/s 提升至 4.9 GB/s（+91.8%）。用户要求将这一微架构优化思想系统性地迁移推广到代码库中的其他高频热路径（字符集探测、容器魔数识别、全局匹配器），并建立精确的优化前后性能基准测试体系，确保每一处改动均为可量化的正向优化。

---

## 2. Target Scope & Optimization Opportunities

### Area 1: 字符集与编码快速探测 (`ttzip_detect_encoding_fast`)
* **目标文件**：[`Sources/CTTZipBridge/CTTZipUtils.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipUtils.c)
* **现状**：逐字节扫描 ASCII 字符（`bytes[i] <= 0x7F`）。在绝大多数包含英文字符/路径名的归档中，逐字节循环造成大量的标量跳转与分支开销。
* **优化方案**：引入 64-bit SWAR 掩码扫描 `(v & 0x8080808080808080ULL) == 0`，单周期批量跳过 8 字节 ASCII，大幅加速归档条目列表的字符集探测。

### Area 2: 归档容器头部魔数识别 (`ttzip_detect_format_from_header`)
* **目标文件**：[`Sources/CTTZipBridge/ttzip_native_archive.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/ttzip_native_archive.c)
* **现状**：调用动态库函数 `memcmp` 比较 4~6 字节魔数（如 7z `7z\xbc\xaf\x27\x1c`、TAR `ustar`、XZ `7zXZ`）。
* **优化方案**：利用 32-bit / 64-bit 未对齐整数直接比对，消除函数调用开销与寄存器保存恢复开销。

### Area 3: 全局共享 Match Finder (`CTTZipNEONMatchFinder.h`)
* **目标文件**：[`Sources/CTTZipBridge/include/CTTZipNEONMatchFinder.h`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/CTTZipNEONMatchFinder.h)
* **现状**：共享头文件中的 `ttzip_neon_match_len` 统一升级为 64-bit SWAR 实现，为所有下游组件提供 4.9 GB/s 的前缀比对能力。

---

## 3. Functional Requirements

* **REQ-1 (SWAR ASCII Mass Scanner)**：在 `ttzip_detect_encoding_fast` 中实现 64 位 SWAR ASCII 快速扫描，对连续 ASCII 字符进行 8 字节步进，遇高位字节平滑回退到多字节 UTF-8 / GB18030 状态机。
* **REQ-2 (Zero-Call Header Sniffing)**：在 `ttzip_detect_format_from_header` 中将短字符串 `memcmp` 重构为固定宽度整数标量比对。
* **REQ-3 (Benchmark Corpus & Quantified Proof)**：编写专属基准测试，量化优化前后的吞吐提升，证明优化为 100% 正向且零倒退。
* **REQ-4 (Zero Memory Safety Invariant)**：所有 64-bit 读取必须受严格的边界检查保护，防止末尾越界与 ASan 报错。

---

## 4. Success Criteria

1. **功能正确性**：编码探测测试、归档识别测试 100% 绿灯通过。
2. **微基准正向加速**：
   * 字符集 ASCII 探测吞吐提升 $\ge +200\%$。
   * 头部魔数探测延迟下降 $\ge +50\%$。
3. **全格式回归与门禁**：全量 525+ 单元测试 100% 绿灯，`XCTestPerformanceMeasureTests` 零性能倒退。

---

## 5. Clarifications

### Q1: 64-bit SWAR 在 ASCII 探测中的大端/小端安全性如何？
- **决议**：SWAR 表达式 `(word & 0x8080808080808080ULL) == 0` 是逐字节最高有效位（MSB）掩码与操作，其位掩码在每个字节内部完全对称，在大端与小端系统上均为 100% 等价有效，零平台歧义。

### Q2: 头部魔数直接读取是否会遇到未对齐访问崩溃（Bus Error）？
- **决议**：使用 `memcpy(&val, buffer, size)` 或编译期未对齐访问宏。现代编译器在 ARM64 和 x86_64 上将其内联编译为单条硬件未对齐加载指令，既保证绝对的 C 标准严格别名合规，又享有单周期硬件指令速度。

