# Data Model: 028-final-two-regressions-zero-closure

## 1. SingleFileExtractionContext

单文件快速写盘上下文模型。

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `isSingleRootFile` | Boolean | 是 | 是否为根目录单文件（`strchr(path, '/') == NULL`） |
| `fileDescriptor` | Integer | 是 | 目标文件打开的文件描述符（`open(..., O_WRONLY | O_CREAT | O_TRUNC, 0644)`） |
| `totalWrittenBytes` | Int64 | 是 | 累计写入字节数 |

## 2. DmgExtractionRouteContext

DMG 解压分发与临时目录上下文模型。

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `format` | String | 是 | 归档格式类型（`dmg`, `iso` 等） |
| `hasPassword` | Boolean | 是 | 是否包含密码 |
| `isDirectNativeRoute` | Boolean | 是 | 是否直通纯 C 原生解压通道 |
| `isTempDirAllocated` | Boolean | 是 | 临时目录是否已按需分配 |
