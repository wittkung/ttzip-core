# Data Model: 029-full-matrix-leapfrog-and-zero-regression

## 1. DeferredCentralizedCleanupContext

场景级延迟集中清理上下文模型。

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `scenarioPrefix` | String | 是 | 场景唯一隔离前缀 |
| `allocatedTempPaths` | Array<String> | 是 | 场景中创建的所有临时路径列表 |
| `isCleanupDeferred` | Boolean | 是 | 清理是否已延后至场景结束 |

## 2. WimDirectStreamContext

WIM 纯 C 原生流式解压上下文模型。

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `bufferCapacity` | Integer | 是 | 读缓冲区大小（默认 8MB） |
| `isMagicMatched` | Boolean | 是 | WIM 标识是否已识别 |
| `isDirectIOActive` | Boolean | 是 | 是否直通 Direct I/O 落盘 |
