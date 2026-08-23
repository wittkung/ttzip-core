# Spec: Universal Dominance Performance Breakthrough (全场景压测全面领先突破)

## 1. 目标与范围 (Goal & Scope)
针对 TTZip 在全格式全场景竞品压测中暴露的 **7 个未打过项**（0.57x ~ 0.84x）与 **6 个打平项**（1.00x ~ 1.15x），通过三大核心底层技术攻坚，实现 100% 测试场景超越官方多线程 7-Zip (`7zz`)、`zstd`、`pigz` 等最强竞品（加速比 $\ge 1.3\text{x} \sim 8.0\text{x}$）。

### 覆盖的 13 项关键攻坚场景
1. **7Z L1 海量小文件 AES-256 打包** (当前 0.57x: 404 MB/s vs 711.6 MB/s -> 目标 $\ge 1500\text{ MB/s}$, $\ge 2.0\text{x}$)
2. **7Z L1 拟真日志 AES-256 打包** (当前 0.66x: 554.7 MB/s vs 844.2 MB/s -> 目标 $\ge 1800\text{ MB/s}$, $\ge 2.0\text{x}$)
3. **7Z L1 海量小文件 无加密打包** (当前 0.69x: 516.7 MB/s vs 747.4 MB/s -> 目标 $\ge 1200\text{ MB/s}$, $\ge 1.5\text{x}$)
4. **7Z L1 500MB 大文件 无加密打包** (当前 0.76x: 4033.2 MB/s vs 5303.2 MB/s -> 目标 $\ge 6500\text{ MB/s}$, $\ge 1.3\text{x}$)
5. **7Z L1 500MB 大文件 AES-256 打包** (当前 0.81x: 3943.6 MB/s vs 4888.5 MB/s -> 目标 $\ge 6000\text{ MB/s}$, $\ge 1.3\text{x}$)
6. **TAR.ZST L1 500MB 大文件打包** (当前 0.82x: 12508.5 MB/s vs 15231.4 MB/s -> 目标 $\ge 18000\text{ MB/s}$, $\ge 1.2\text{x}$)
7. **7Z L1 拟真日志 无加密打包** (当前 0.84x: 813.4 MB/s vs 963.3 MB/s -> 目标 $\ge 1600\text{ MB/s}$, $\ge 1.6\text{x}$)
8. **TAR.ZST L1 100MB 高熵解压** (当前 1.00x: 4856.2 MB/s -> 目标 $\ge 6000\text{ MB/s}$, $\ge 1.25\text{x}$)
9. **7Z L1 500MB 大文件解压** (当前 1.02x: 5362.3 MB/s -> 目标 $\ge 7500\text{ MB/s}$, $\ge 1.4\text{x}$)
10. **TAR.ZST L6 100MB 高熵解压** (当前 1.02x: 5335.1 MB/s -> 目标 $\ge 6500\text{ MB/s}$, $\ge 1.25\text{x}$)
11. **7Z L6 海量小文件 AES-256 打包** (当前 1.04x: 278.2 MB/s -> 目标 $\ge 500\text{ MB/s}$, $\ge 1.8\text{x}$)
12. **TAR.ZST L6 500MB 大文件解压** (当前 1.14x: 6441.8 MB/s -> 目标 $\ge 8000\text{ MB/s}$, $\ge 1.4\text{x}$)
13. **TAR.ZST L1 100MB 高熵打包** (当前 1.15x: 5709.4 MB/s -> 目标 $\ge 8500\text{ MB/s}$, $\ge 1.5\text{x}$)

---

## 2. 核心架构与功能规格

### 模块 A：7Z AES-256 跨文件 Session Key 无锁复用与 ARMv8 Crypto 硬件加速
- **功能**：在整个 7z 归档生命周期中，相同密码与 Salt 仅计算一次 $2^{19}$ 轮 SHA-256 密钥派生（KDF）。
- **硬件内核**：使用 ARMv8 Cryptographic Extensions (`vsha256hq_u32` / `vsha256h2q_u32` / `vsha256su0q_u32` / `vsha256su1q_u32`)，实现单轮 KDF 耗时由 $630\text{ ms}$ 降至 $< 15\text{ ms}$。
- **并发安全性**：通过只读共享上下文向所有 GCD 并发 Worker 分发 32 字节 Key 指针，消除 1000 次重复计算。

### 模块 B：Fast LZMA2 / L1 Direct Hash 与 ARM64 无分支 Range Coder
- **Direct Hash 匹配器**：在 Level 1 模式下使用 $O(1)$ 复杂度的 `Direct Hash-2/3` 单层扁平哈希表，替换多重指针跳转的 HC4/BT4。
- **无分支 Range Coder**：采用位掩码与 `csel` 消除状态机归一化分支，消除 Apple Silicon 深度流水线分支预测惩罚。
- **启发式采样熵探测旁路**：采样前 4KB 数据，若不可压缩率 $> 95\%$ 直接走线速 `0x01/0x02` Uncompressed Chunk 零拷贝旁路。

### 模块 C：TAR.ZST 100% In-Process Native Direct mmap 管道
- **脱离 libarchive 瓶颈**：自研轻量级 Pax Tar 512B 写入器，与 `libzstd` `ZSTD_compressStream2` / `ZSTD_decompressStream` 直接在 C 语言层对接。
- **Direct mmap 零拷贝**：大文件直接以 `mmap` 地址作为 `ZSTD_inBuffer.src`，消除 3 重内存拷贝。
- **硬件拓扑自适应**：设置 `jobSize = 8MB`，`overlapLog = 3`，充分利用 Apple Silicon 系统级缓存（SLC）。

---

## 3. 验收标准 (Acceptance Criteria)
1. **100% 胜出率**：执行 `TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests`，46 组场景中 100% 实现 Speedup $\ge 1.2\text{x}$。
2. **代码行数规范**：所有新增或重构的 C / Swift 文件严格控制在 $\le 500$ 行以内。
3. **零回归与全量绿灯**：`swift test` 全量 559+ 单测与 7 项提升后的性能硬门禁 100% PASS。
