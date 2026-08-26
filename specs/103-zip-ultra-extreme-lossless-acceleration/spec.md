# Feature Specification: ZIP Tier 6/7 (Ultra Zopfli & Extreme Peak) 零损失加速

## 1. 业务背景与问题定义 (Problem Statement)
TTZip 在纯 ZIP 格式下实现了覆盖全部 8 大黄金档位的完整帕累托前沿：
- Tier 6 (Ultra Zopfli) 与 Tier 7 (Extreme Peak) 在 18 核心并发分块下分别实现了 ~3.0 MB/s 与 ~0.28 MB/s 的极限压缩，压缩率打破了 `advzip -4` 的行业极限纪录（体积达 2.95 MB）；
- 但当前由于：
  1. **分块粒度过大 (6.25MB)**：导致 16~18 核心并发时总工作集高达 152MB，彻底击穿 Apple Silicon L2 共享缓存（16~32MB），引发 DRAM 总线抖动与内存争用；
  2. **迭代缺乏不动点早退机制**：固定跑满 10~15 轮迭代，但在第 4~5 轮后 literal/match 决策序列已完全收敛锁死，后 60%+ 的迭代轮次属于纯粹的 CPU 时钟空转；
  3. **DAG 代价计算标量循环**：在 `deflate_find_min_cost_path` 中采用标量步进与串行比较，未充分释放 Apple Silicon NEON 向量寄存器吞吐；
- **目标**：在**压缩比绝对零损失（0.0000% 损失 / 压缩后体积 100% 不变）**的前提下，对 L6/L7 实施缓存感知分块、不动点无损早退与 SIMD 优化，将 L6 吞吐提升至 10+ MB/s，L7 吞吐提升至 1.0+ MB/s。

## 2. 用户场景与用户故事 (User Scenarios & User Stories)

### User Story 1: 缓存感知 2MB Tile 分块与滑动字典无缝衔接 (US1)
- **作为** 归档管线，
- **我希望** `ZipExtremeBlockWriter` 采用 L2 Cache-Aware 的 2MB Tile 分块，并保持 32KB 跨块滑动字典连续性，
- **以便于** 将多核并发工作集从 152MB 压减至 40MB 以内，大幅减少 L2 缓存失效率并消除 DRAM 总线阻塞。

### User Story 2: 不动点决策向量严格无损自适应早退 (US2)
- **作为** 图论动态规划压缩器，
- **我希望** 跟踪每轮迭代的决策向量哈希与比特代价，当检测到决策序列达到全局不动点（Fixed-Point Invariant）时立即终止后续冗余迭代，
- **以便于** 避免 60%+ 无效 CPU 时钟浪费，同时数学证明压缩体积 100% 绝对比特精确（Bit-Exact）。

### User Story 3: ARM NEON 4-Way 展开与 TLS 无锁内存复用 (US3)
- **作为** 底层 C 桥接引擎，
- **我希望** 在 DAG 最短路径代价计算中应用 NEON 向量化指令，并复用 TLS 内存 Arena，
- **以便于** 消除热路径循环内的数据冒险与动态分配开销。

## 3. 功能需求清单 (Functional Requirements)
- **FR-001**: `ZipExtremeBlockWriter.swift` 将 L6/L7 的分块粒度标准化为 2MB Tile (`actualBlockSize = min(2 * 1024 * 1024, rawData.count)`)，且严格保留 32KB 历史字典缓冲区。
- **FR-002**: 在 `ttzip_zopfli_engine.c` 中支持 `early_exit_threshold` 与决策向量不动点检测，一旦连续 2 轮迭代 Huffman 树代价变化 $\le 0.0001$ 且决策序列哈希一致，即刻收敛退出。
- **FR-003**: 确保 100MB enwik8 上压缩输出体积严格与基准持平（Tier 6 $\le 2.994\text{ MB}$, Tier 7 $\le 2.958\text{ MB}$），压缩率退化量断言 $\le 0.0000\%$。
- **FR-004**: 确保生成的 ZIP 文件 100% 通过系统原生 `/usr/bin/unzip -t` 校验。

## 4. 成功衡量指标 (Success Criteria)
- **SC-001**: 压缩率零损失：在 100MB enwik8 上，L6 体积 $\le 2,994,000$ 字节，L7 体积 $\le 2,958,000$ 字节。
- **SC-002**: 吞吐大幅提升：在 Apple Silicon 18 核心上，L6 吞吐提升至 $\ge 8.0\text{ MB/s}$（较基准提速 2.5x+），L7 吞吐提升至 $\ge 0.8\text{ MB/s}$（较基准提速 2.8x+）。
- **SC-003**: 全量单测与性能门禁 100% 绿色通过。

## 5. 澄清与会话记录 (Clarifications)
- **C-001**: 2MB Tile 分块会不会导致跨块压缩率损失？
  - 答：不会。TTZip 实现了 32KB 跨块滑动历史字典注入（`history_size = 32768`），在块边界无缝继承前一块末尾 32KB 字典，保证 LZ77 匹配长度与单大块完全等价。
- **C-002**: 为什么不动点早退能保证 0 损失？
  - 答：因为当第 $k$ 轮 Pass 的字面量与长度/距离决策序列与第 $k-1$ 轮完全一致时，其派生的 Huffman 树模型与比特流输出在数学上完全恒等。
