# Data Model: Deep Algorithmic Absorption of libdeflate

**Feature Directory**: `specs/054-libdeflate-deep-algorithmic-absorption`
**Created**: 2026-08-18
**Status**: Completed

---

## 1. 核心实体模型 (Entity Models)

### 1.1 `Adler32ChecksumResult`
表示 Adler-32 高性能硬件计算的结果与遥测元数据。

| 字段名 | 类型 | 必填 | 约束 | 语义说明 |
| :--- | :--- | :--- | :--- | :--- |
| `adler32` | `integer` | 是 | 32-bit 无符号整数 ($0 \dots 4294967295$) | 计算所得的最终 Adler-32 校验和 |
| `bytesProcessed` | `integer` | 是 | $\ge 0$ | 参与计算的总字节数 |
| `engineType` | `string` | 是 | 枚举: `arm_neon_dotprod`, `arm_neon_baseline`, `x86_avx2`, `scalar_fallback` | 实际触发的硬件加速计算分支 |
| `elapsedNanoseconds` | `integer` | 是 | $\ge 0$ | 物理单调时钟耗时（纳秒） |

---

### 1.2 `MatchfinderRebaseParams`
表示 16-bit 匹配查找器滑动窗口重置操作的参数与状态。

| 字段名 | 类型 | 必填 | 约束 | 语义说明 |
| :--- | :--- | :--- | :--- | :--- |
| `windowSize` | `integer` | 是 | `32768` 或 `65536` | 滑动窗口大小（字节） |
| `tableSizeEntries` | `integer` | 是 | $\ge 1024$，且为 16 的倍数 | 哈希表/链表中的 16-bit 条目总数 |
| `vectorInstruction` | `string` | 是 | 枚举: `arm_neon_vqaddq_s16`, `x86_avx2_paddsw`, `scalar_branchless` | 执行重置所使用的底层向量化指令 |
| `rebaseDurationMicros` | `number` | 是 | $\ge 0.0$ | 窗口重置耗时（微秒，断言 $\le 5.0\mu s$） |

---

### 1.3 `BranchlessBitbufState`
表示 64 位无分支位流解码器的瞬时寄存器状态。

| 字段名 | 类型 | 必填 | 约束 | 语义说明 |
| :--- | :--- | :--- | :--- | :--- |
| `bitbuf64` | `integer` | 是 | 64-bit 无符号整数 | 64 位机器字累加器内容 |
| `bitsLeft` | `integer` | 是 | $0 \dots 63$ | 累加器中当前可立即消费的有效位数 |
| `consumedBytes` | `integer` | 是 | $\ge 0$ | 已经从输入流中读取消费的字节数 |
| `refillCount` | `integer` | 是 | $\ge 0$ | 触发 `REFILL_BITS_BRANCHLESS()` 的总次数 |

---

## 2. 内存布局对齐契约 (Memory Alignment Contract)

- 所有匹配查找器结构体（`ttzip_hc_matchfinder_t`, `ttzip_bt_matchfinder_t`）必须统一声明 `__attribute__((aligned(32)))`。
- 哈希表数组大小必须对齐到 `1024` 字节，确保向量化循环 `size != 0` 递减时永不产生残差标量分支。
