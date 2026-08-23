# PR #2416 Maintainer Reply (Final)

Hi Nathan,

I wanted to follow up personally and sincerely apologize for my earlier hasty PR submissions. I was overly eager about chasing performance numbers and pushed AI-assisted patches without deeply understanding the codebase and hardware mechanics. That was irresponsible on my part, and I've written an [Open Letter of Apology and Reflection](https://gist.github.com/wittkung/0874f8afe78020325a3db3326ef7d7e5) to explain the context and what I learned. Thank you for your patience.

Your guidance regarding `vmaxvq` was completely right on the mark, and I have fully adopted it. `compare_diff_lane` has also been completely removed.

After thoroughly studying the source code and understanding the low-level hardware principles today, I identified two additional optimization points and verified each in isolation:
1. **Stage 1 (first 16 bytes) pure scalar GPR probing**: eliminates the cross-domain latency between GPR and FPR on short mismatches.
2. **Using `UNLIKELY` branch hints**: provides prior branch probability that mismatch is rare during continuous matching, enabling compiler hot/cold basic block splitting and reducing forward `cbz` taken branches.

---

Benchmarked head candidate against merge-base develop. Apple M5 Max (128 GB Unified Memory), Apple clang 21.0.0, `-O3` Release static builds, medians of 5 cross-interleaved repetitions with cooldowns (overall median CV 1.05%, max 6.2% on sub-nanosecond micro cases).

compare256/neon micro, ns per call:

| len | base | fixed | fixed Δ |
|----:|-----:|------:|--------:|
| 1   | 0.76 |  0.70 | -8.4%   |
| 10  | 1.05 |  0.93 | -11.5%   |
| 16  | 1.06 |  0.91 | -14.5%   |
| 24  | 1.17 |  0.93 | -20.3%   |
| 32  | 1.16 |  0.97 | -16.5%   |
| 40  | 1.38 |  1.16 | -15.9%   |
| 48  | 1.47 |  1.20 | -18.2%   |
| 56  | 1.68 |  1.51 | -10.3%   |
| 64  | 1.83 |  1.65 | -9.5%   |
| 80  | 2.25 |  1.74 | -22.9%   |
| 100 | 2.95 |  2.07 | -29.8%   |
| 175 | 4.55 |  2.69 | -40.9%   |
| 256 | 6.17 |  3.37 | -45.4%   |

Deflate with the suggested change (Full 50-point matrix across 8 workloads, 128KB & 1MB; 1MB streaming median CV 1.21%, 128KB median CV 1.95%):

| benchmark | base | fixed | fixed Δ |
|---|---:|---:|---:|
| `deflate_bench` text/131072/1 | 158.5 µs | **128.7 µs** | **-18.8%** |
| `deflate_bench` text/131072/3 | 308.0 µs | 315.9 µs | +2.6% |
| `deflate_bench` text/131072/6 | 899.1 µs | 887.8 µs | -1.3% |
| `deflate_bench` text/131072/9 | 1.18 ms | 1.15 ms | -2.7% |
| `deflate_bench` text/1048576/1 | 1.75 ms | **1.53 ms** | **-12.2%** |
| `deflate_bench` text/1048576/3 | 3.69 ms | 3.60 ms | -2.4% |
| `deflate_bench` text/1048576/6 | 8.68 ms | 8.54 ms | -1.6% |
| `deflate_bench` text/1048576/9 | 10.85 ms | 10.79 ms | -0.5% |
| `deflate_bench` striped_rgb/131072/3 | 17.4 µs | 16.6 µs | -4.7% |
| `deflate_bench` striped_rgb/131072/6 | 18.0 µs | 16.9 µs | -6.2% |
| `deflate_bench` striped_rgb/131072/9 | 83.9 µs | 81.7 µs | -2.6% |
| `deflate_bench` striped_rgb/1048576/3 | 146.4 µs | 137.4 µs | -6.2% |
| `deflate_bench` striped_rgb/1048576/6 | 152.3 µs | 142.4 µs | -6.5% |
| `deflate_bench` striped_rgb/1048576/9 | 684.0 µs | 662.4 µs | -3.2% |
| `deflate_bench` dna/131072/3 | 427.8 µs | 443.2 µs | +3.6% |
| `deflate_bench` dna/131072/6 | 2.60 ms | 2.56 ms | -1.5% |
| `deflate_bench` dna/131072/9 | 20.09 ms | 19.93 ms | -0.8% |
| `deflate_bench` dna/1048576/3 | 3.87 ms | 3.89 ms | +0.3% |
| `deflate_bench` dna/1048576/6 | 23.30 ms | 22.56 ms | -3.2% |
| `deflate_bench` dna/1048576/9 | 182.27 ms | 177.03 ms | -2.9% |
| `deflate_bench` mixed/131072/3 | 346.7 µs | 351.9 µs | +1.5% |
| `deflate_bench` mixed/131072/6 | 789.8 µs | 829.0 µs | +5.0% |
| `deflate_bench` mixed/131072/9 | 4.15 ms | 4.16 ms | +0.4% |
| `deflate_bench` mixed/1048576/3 | 4.13 ms | 4.10 ms | -0.7% |
| `deflate_bench` mixed/1048576/6 | 7.60 ms | 7.70 ms | +1.3% |
| `deflate_bench` mixed/1048576/9 | 35.09 ms | 35.31 ms | +0.6% |
| `deflate_bench` short_match/131072/3 | 434.0 µs | 444.3 µs | +2.4% |
| `deflate_bench` short_match/131072/6 | 540.3 µs | 554.0 µs | +2.5% |
| `deflate_bench` short_match/131072/9 | 738.0 µs | 714.4 µs | -3.2% |
| `deflate_bench` short_match/1048576/3 | 4.89 ms | 4.89 ms | 0.0% |
| `deflate_bench` short_match/1048576/6 | 5.77 ms | 5.72 ms | -0.8% |
| `deflate_bench` short_match/1048576/9 | 7.33 ms | 7.35 ms | +0.2% |
| `deflate_bench` random/131072/3 | 871.3 µs | 870.3 µs | -0.1% |
| `deflate_bench` random/131072/6 | 817.1 µs | 826.4 µs | +1.1% |
| `deflate_bench` random/131072/9 | 1.22 ms | 1.15 ms | -5.6% |
| `deflate_bench` random/1048576/3 | 9.20 ms | 9.15 ms | -0.5% |
| `deflate_bench` random/1048576/6 | 8.03 ms | 8.10 ms | +0.8% |
| `deflate_bench` random/1048576/9 | 11.30 ms | 11.23 ms | -0.6% |
| `deflate_bench` literals/131072/3 | 957.0 µs | 959.7 µs | +0.3% |
| `deflate_bench` literals/131072/6 | 916.6 µs | 916.9 µs | 0.0% |
| `deflate_bench` literals/131072/9 | 2.11 ms | 2.12 ms | +0.5% |
| `deflate_bench` literals/1048576/3 | 9.72 ms | 9.78 ms | +0.6% |
| `deflate_bench` literals/1048576/6 | 8.63 ms | 8.62 ms | -0.2% |
| `deflate_bench` literals/1048576/9 | 19.13 ms | 19.37 ms | +1.3% |
| `deflate_bench` realistic_rgb/131072/3 | 877.5 µs | 892.4 µs | +1.7% |
| `deflate_bench` realistic_rgb/131072/6 | 850.0 µs | 876.5 µs | +3.1% |
| `deflate_bench` realistic_rgb/131072/9 | 1.42 ms | 1.45 ms | +2.1% |
| `deflate_bench` realistic_rgb/1048576/3 | 9.15 ms | 9.21 ms | +0.7% |
| `deflate_bench` realistic_rgb/1048576/6 | 8.13 ms | 8.13 ms | +0.1% |
| `deflate_bench` realistic_rgb/1048576/9 | 12.88 ms | 12.86 ms | -0.1% |

The severe L9 regression on `mixed` (previously +13.1%) is restored to baseline, the `striped_rgb`, `dna`, and long-length throughput gains survive, and `text` L1 sees a substantial boost (-12.2% to -18.8%).

*(Note: I kept the lane extractions explicitly open-coded to avoid hidden returns in macros. If you prefer a local helper macro to shorten the line count, please let me know!)*

- [Full Benchmark Report & Reproduction Steps](https://gist.github.com/wittkung/6505cc83789c3166c023a4de36bf5df6)

Thank you again for your patience and mentorship!

Sincerely,  
**Witt Kung (孔维涛)**

---

### 🇨🇳 中文对照版本 (Chinese Translation)

Nathan 你好，

我想亲自跟进并为我之前草率提交的几个 PR 向你真诚道歉。我当时太急于追求极限性能，在没有真正读懂代码库和底层硬件机制的情况下盲目提交了 AI 辅助的补丁，这种做法很不负责任。我写了一篇[公开道歉与反思信](https://gist.github.com/wittkung/0874f8afe78020325a3db3326ef7d7e5)来交代背景和我的反思，再次感谢你的包容。

您对 `vmaxvq` 的指导完全是正确的，已经全面采纳。`compare_diff_lane` 也已经删除。

今天深度研读源码，完整理解底层原理之后，我找到了两个额外的优化点，并单独测试，确证有效：
1. 针对第一阶段（前 16 字节）纯标量 GPR 探测，避免了 GPR 和 FPR 的跨域时延；
2. 使用 `UNLIKELY`，提供了失配条件为假是大概率事件的先验概率，编译器执行热/冷基本块分离，减少了 `cbz` 向前跳转次数。

---

测试环境：基于 merge-base develop 对比候选分支代码。Apple M5 Max（128 GB 统一内存），Apple clang 21.0.0，`-O3` Release 静态编译构建，5 轮双向交错测试取中位数（带散热冷却，全样本中位数 CV 1.05%，仅亚纳秒微观极端点受计时颗粒度影响最大达 6.2%）。

compare256/neon micro，单次调用纳秒数 (ns per call)：

| len | base | fixed | fixed Δ |
|----:|-----:|------:|--------:|
| 1   | 0.76 |  0.70 | -8.4%   |
| 10  | 1.05 |  0.93 | -11.5%   |
| 16  | 1.06 |  0.91 | -14.5%   |
| 24  | 1.17 |  0.93 | -20.3%   |
| 32  | 1.16 |  0.97 | -16.5%   |
| 40  | 1.38 |  1.16 | -15.9%   |
| 48  | 1.47 |  1.20 | -18.2%   |
| 56  | 1.68 |  1.51 | -10.3%   |
| 64  | 1.83 |  1.65 | -9.5%   |
| 80  | 2.25 |  1.74 | -22.9%   |
| 100 | 2.95 |  2.07 | -29.8%   |
| 175 | 4.55 |  2.69 | -40.9%   |
| 256 | 6.17 |  3.37 | -45.4%   |

采用建议修改后的 Deflate 宏观测试（全量 50 个测试点覆盖 8 大负载，128KB & 1MB；1MB 流式中位数 CV 1.21%，128KB 中位数 CV 1.95%）：

| benchmark | base | fixed | fixed Δ |
|---|---:|---:|---:|
| `deflate_bench` text/131072/1 | 158.5 µs | **128.7 µs** | **-18.8%** |
| `deflate_bench` text/131072/3 | 308.0 µs | 315.9 µs | +2.6% |
| `deflate_bench` text/131072/6 | 899.1 µs | 887.8 µs | -1.3% |
| `deflate_bench` text/131072/9 | 1.18 ms | 1.15 ms | -2.7% |
| `deflate_bench` text/1048576/1 | 1.75 ms | **1.53 ms** | **-12.2%** |
| `deflate_bench` text/1048576/3 | 3.69 ms | 3.60 ms | -2.4% |
| `deflate_bench` text/1048576/6 | 8.68 ms | 8.54 ms | -1.6% |
| `deflate_bench` text/1048576/9 | 10.85 ms | 10.79 ms | -0.5% |
| `deflate_bench` striped_rgb/131072/3 | 17.4 µs | 16.6 µs | -4.7% |
| `deflate_bench` striped_rgb/131072/6 | 18.0 µs | 16.9 µs | -6.2% |
| `deflate_bench` striped_rgb/131072/9 | 83.9 µs | 81.7 µs | -2.6% |
| `deflate_bench` striped_rgb/1048576/3 | 146.4 µs | 137.4 µs | -6.2% |
| `deflate_bench` striped_rgb/1048576/6 | 152.3 µs | 142.4 µs | -6.5% |
| `deflate_bench` striped_rgb/1048576/9 | 684.0 µs | 662.4 µs | -3.2% |
| `deflate_bench` dna/131072/3 | 427.8 µs | 443.2 µs | +3.6% |
| `deflate_bench` dna/131072/6 | 2.60 ms | 2.56 ms | -1.5% |
| `deflate_bench` dna/131072/9 | 20.09 ms | 19.93 ms | -0.8% |
| `deflate_bench` dna/1048576/3 | 3.87 ms | 3.89 ms | +0.3% |
| `deflate_bench` dna/1048576/6 | 23.30 ms | 22.56 ms | -3.2% |
| `deflate_bench` dna/1048576/9 | 182.27 ms | 177.03 ms | -2.9% |
| `deflate_bench` mixed/131072/3 | 346.7 µs | 351.9 µs | +1.5% |
| `deflate_bench` mixed/131072/6 | 789.8 µs | 829.0 µs | +5.0% |
| `deflate_bench` mixed/131072/9 | 4.15 ms | 4.16 ms | +0.4% |
| `deflate_bench` mixed/1048576/3 | 4.13 ms | 4.10 ms | -0.7% |
| `deflate_bench` mixed/1048576/6 | 7.60 ms | 7.70 ms | +1.3% |
| `deflate_bench` mixed/1048576/9 | 35.09 ms | 35.31 ms | +0.6% |
| `deflate_bench` short_match/131072/3 | 434.0 µs | 444.3 µs | +2.4% |
| `deflate_bench` short_match/131072/6 | 540.3 µs | 554.0 µs | +2.5% |
| `deflate_bench` short_match/131072/9 | 738.0 µs | 714.4 µs | -3.2% |
| `deflate_bench` short_match/1048576/3 | 4.89 ms | 4.89 ms | 0.0% |
| `deflate_bench` short_match/1048576/6 | 5.77 ms | 5.72 ms | -0.8% |
| `deflate_bench` short_match/1048576/9 | 7.33 ms | 7.35 ms | +0.2% |
| `deflate_bench` random/131072/3 | 871.3 µs | 870.3 µs | -0.1% |
| `deflate_bench` random/131072/6 | 817.1 µs | 826.4 µs | +1.1% |
| `deflate_bench` random/131072/9 | 1.22 ms | 1.15 ms | -5.6% |
| `deflate_bench` random/1048576/3 | 9.20 ms | 9.15 ms | -0.5% |
| `deflate_bench` random/1048576/6 | 8.03 ms | 8.10 ms | +0.8% |
| `deflate_bench` random/1048576/9 | 11.30 ms | 11.23 ms | -0.6% |
| `deflate_bench` literals/131072/3 | 957.0 µs | 959.7 µs | +0.3% |
| `deflate_bench` literals/131072/6 | 916.6 µs | 916.9 µs | 0.0% |
| `deflate_bench` literals/131072/9 | 2.11 ms | 2.12 ms | +0.5% |
| `deflate_bench` literals/1048576/3 | 9.72 ms | 9.78 ms | +0.6% |
| `deflate_bench` literals/1048576/6 | 8.63 ms | 8.62 ms | -0.2% |
| `deflate_bench` literals/1048576/9 | 19.13 ms | 19.37 ms | +1.3% |
| `deflate_bench` realistic_rgb/131072/3 | 877.5 µs | 892.4 µs | +1.7% |
| `deflate_bench` realistic_rgb/131072/6 | 850.0 µs | 876.5 µs | +3.1% |
| `deflate_bench` realistic_rgb/131072/9 | 1.42 ms | 1.45 ms | +2.1% |
| `deflate_bench` realistic_rgb/1048576/3 | 9.15 ms | 9.21 ms | +0.7% |
| `deflate_bench` realistic_rgb/1048576/6 | 8.13 ms | 8.13 ms | +0.1% |
| `deflate_bench` realistic_rgb/1048576/9 | 12.88 ms | 12.86 ms | -0.1% |

此前 `mixed` 在 L9 下 +13.1% 回退已恢复至基准线，`striped_rgb`、`dna` 和长匹配的性能提升得以完整保留，`text` 纯文本在 L1 获得了显著加速（-12.2% ~ -18.8%）。

（注：我目前将车道提取逻辑平铺展开，以避免在宏里隐藏 return。如果您更倾向于用局部宏缩减代码行数，请告知我）

- [完整基准测试报告与复现步骤](https://gist.github.com/wittkung/6505cc83789c3166c023a4de36bf5df6)

再次感谢您的耐心指导与指引！

祝好，  
**孔维涛 (Witt Kung)**
