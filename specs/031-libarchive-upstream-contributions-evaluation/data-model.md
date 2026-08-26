# Data Model: 031-libarchive-upstream-contributions-evaluation

本数据模型定义了 TTZip 开源贡献管理、技术评估项、Upstream PR 规格及性能门禁实体的严格字段结构。

---

## 1. UpstreamContributionProposal (上游贡献提议实体)

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `id` | `String` | 是 | 提案唯一标识符 (例如 `"C01-7z-write-aes256"`) |
| `title` | `String` | 是 | 提案英文标准标题 |
| `tier` | `String` | 是 | 优先级梯队 (`"Tier1"`, `"Tier2"`, `"Tier3"`) |
| `targetFiles` | `Array<String>` | 是 | Upstream libarchive 目标文件路径列表 |
| `upstreamStatus` | `String` | 是 | 状态 (`"Proposed"`, `"InDevelopment"`, `"Submitted"`, `"Merged"`, `"InternalOnly"`) |
| `prNumber` | `Integer` | 否 | GitHub PR 编号 (若已提交) |
| `speedupFactor` | `Double` | 是 | 相对原生 libarchive 的预期或实测吞吐提升倍数 |
| `hasScalarFallback` | `Boolean` | 是 | 是否包含 100% 兼容的 C99 标量回退分支 |
| `breakingChanges` | `Boolean` | 是 | 是否包含破坏性 API/ABI 变更 (必须为 `false`) |

---

## 2. CryptoWriterConfig (7z AES-256 写入配置实体)

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `passphrase` | `String` | 是 | 加密密码明文字符串 |
| `numCyclesPower` | `Integer` | 是 | SHA-256 迭代轮次指数 (默认 19，范围 10..24) |
| `saltBytes` | `Integer` | 是 | 随机 Salt 字节数 (固定 16) |
| `ivBytes` | `Integer` | 是 | 随机 IV 字节数 (固定 16) |
| `headerEncryption` | `Boolean` | 是 | 是否对目录元数据启用 `kEncodedHeader` 加密 |

---

## 3. PreallocationDescriptor (磁盘预分配描述实体)

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `filePath` | `String` | 是 | 目标物理文件路径 |
| `fileDescriptor` | `Integer` | 是 | 打开的有效文件描述符 (fd >= 0) |
| `expectedSize` | `Integer` | 是 | 归档中声明的文件精确字节数 |
| `platformApi` | `String` | 是 | 使用的系统级预分配 API (`"Darwin_F_PREALLOCATE"`, `"POSIX_posix_fallocate"`, `"Fallback_ftruncate"`) |
| `isContiguous` | `Boolean` | 是 | 是否申请连续磁盘块 |
