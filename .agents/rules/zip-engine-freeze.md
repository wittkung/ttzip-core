# 🔒 ZIP 核心引擎代码冻结保护规则 (ZIP Engine Freeze Rule)

## 一、 冻结指令与适用文件范围

以下 ZIP 格式核心打包、解压与 SIMD 加解密引擎代码已被标定为 **【完全冻结状态 (FROZEN)】**。任何 AI Agent、代码助手或自动重构流程 **严禁修改** 任何一行逻辑或结构，除非用户在 Prompt 中显式包含强制解锁命令 `FORCE UNFREEZE ZIP`：

### 冻结文件清单 (Frozen Target Files)

1. **ZIP 核心算法与并行处理层**：
   - `Sources/TTZipCore/Zip/ZipParallelExtractor.swift`
   - `Sources/TTZipCore/Zip/ZipParallelWriter.swift`
   - `Sources/TTZipCore/Zip/ZipCryptoEngine.swift`
   - `Sources/TTZipCore/Zip/ZipBlockParallelCompressor.swift`
   - `Sources/TTZipCore/Zip/ZipBlockParallelDecompressor.swift`
   - `Sources/TTZipCore/Zip/ZipCentralDirectoryReader.swift`
   - `Sources/TTZipCore/Zip/ZipStoreStreamWriter.swift`

2. **C / ARM NEON SIMD 桥接与加解密层**：
   - `Sources/CTTZipBridge/CTTZipBridge_Crypto.c`
   - `Sources/CTTZipBridge/include/CTTZipBridge_Crypto.h`
   - `Sources/CTTZipBridge/CTTZipExtract.c`

---

## 二、 AI 执行纪律

1. **读操作允许，写操作拦截**：可以读取上述文件分析逻辑，但严禁生成任何对上述文件的编辑（`replace_file_content` / `multi_replace_file_content` / `write_to_file`）。
2. **重构隔离**：若后续需要开发其他压缩格式（如 7Z、RAR、XZ）或上层 UI 功能，必须在各自独立的文件中编写，禁止修改已冻结的 ZIP 核心引擎代码。
3. **测试保护**：任何修改若触发 `swift test` 中 ZIP 相关的性能与哈希指纹回归测试失败，必须立刻 revert。
