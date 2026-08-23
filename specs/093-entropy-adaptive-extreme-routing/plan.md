# Implementation Plan: Entropy-Adaptive Intelligent Extreme Routing

## Technical Context

将香农熵与微采样探测下沉至 C 底层，并整合至 Swift `ZipExtremeBlockWriter`：
1. **C 探测器**：`ttzip_fast_entropy_probe(const uint8_t* data, size_t size, double* entropy_out, double* estimated_ratio_out)`
   - 256-bin 栈上直方图，单循环累加；
   - 计算 $H = -\sum p_i \log_2(p_i)$；
   - 若 $H \ge 7.35$ 且试探压缩率 $< 1.03$，判定为 `TTZIP_ROUTING_STORE` (Method 0)；否则为 `TTZIP_ROUTING_DEFLATE` (Method 8)。
2. **Swift 调度**：`ZipExtremeBlockWriter` 在读取数据后执行前置探测：
   - 若判定为 Method 0：写入 Method 0 Local Header，零拷贝直写原始数据流，写入 Method 0 Central Directory；
   - 若判定为 Method 8：进入 18 核极速分块多核 Deflate 流。

## Constitution Check

- [P0] 零中间堆分配：探测器使用 256 元素栈上数组 `uint32_t freq[256]`，零 `malloc`。
- [P1] 吞吐硬门禁：高熵数据吞吐 $\ge 15,000\text{ MB/s}$，低熵维持 $\ge 5,000\text{ MB/s}$。
- [P2] 规范合规：生成 ZIP 包 100% 经 `/usr/bin/unzip -t` 校验。

---

## Phase 0: Research Tasks & Findings

- R001 [SUBAGENT:research] 《香农信息熵与压缩率前置采样探测算法与 Apple Silicon 硬件加速研究》

---

## Phase 1: Design Artifacts & Contracts

- `research.md`
- `data-model.md`
- `contracts/entropy-adaptive-routing-contract.json`
- `quickstart.md`
- `tasks.md`

---

## Planned Changes by Component

- [MODIFY] `Sources/CTTZipBridge/include/CTTZipStreamCoder.h`: 声明 `ttzip_probe_entropy_and_compressibility`。
- [MODIFY] `Sources/CTTZipBridge/CTTZipStreamCoder.c`: 实现 4KB 栈上直方图香农熵探测器。
- [MODIFY] `Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift`: 集成熵分流与 Method 0 直通写入。
- [NEW] `Tests/TTZipTests/EntropyAdaptiveExtremeRoutingTests.swift`: 单元测试与高熵/低熵实测验证。
