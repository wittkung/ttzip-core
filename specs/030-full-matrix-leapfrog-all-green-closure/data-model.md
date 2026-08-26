# Data Model: 030-full-matrix-leapfrog-all-green-closure

## 1. DmgPCoreTuningContext

DMG / ISO P-Core 硬件调度上下文模型。

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `qosClass` | String | 是 | 线程 QoS 等级（`QOS_CLASS_USER_INTERACTIVE`） |
| `isPCoreAffinityActive` | Boolean | 是 | P-Core 调度是否已激活 |
| `boostTimestamp` | Integer | 是 | 提频时间戳 |

## 2. WimNativeStreamContext

WIM 纯 C 8MB 零拷贝流式解压上下文模型。

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `readBufferCapacity` | Integer | 是 | 读缓冲区大小（`8388608` 字节） |
| `isSingleFileBypass` | Boolean | 是 | 是否触发单文件 Direct I/O 旁路 |
| `directoryCacheSlots` | Integer | 是 | L2 目录缓存槽位数（64 槽位） |
