# Data Model: 026-zero-regression-final-mile

## 1. FastPathTarExtractionContext

TAR / TAR.ZST 单文件与根条目快速旁路上下文模型。

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `entryPathname` | String | 是 | 归档条目相对路径 |
| `isRootEntry` | Boolean | 是 | 是否无子目录前缀（`strchr(entryPathname, '/') == NULL`） |
| `fullDestPath` | String | 是 | 物理目标写入路径 |

## 2. SevenZipCacheAlignedSliceConfig

7Z 256KB Cache 对齐切片配置模型。

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `sliceSizeBytes` | Integer | 是 | 单切片大小（262,144 字节，即 256KB） |
| `alignmentBytes` | Integer | 是 | 物理对齐字节（64 字节缓存行 / 16KB 页） |
| `useArmNeonCrypto` | Boolean | 是 | 是否启用 ARMv8 8-Way 硬件向量加解密 |
