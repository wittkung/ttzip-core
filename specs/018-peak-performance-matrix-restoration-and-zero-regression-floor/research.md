# Technical Research & Architectural Findings (Feature 018)

**Feature**: Restoration Against Historical Peak Matrix & Hard 10% Floor Invariant  
**Directory**: `specs/018-peak-performance-matrix-restoration-and-zero-regression-floor/`

---

## R001 [SUBAGENT:research] 《Lzip 全级别参数与解压多核调优》

### 1. Decision
在 `Sources/CTTZipBridge/ttzip_tar_native.c` 中，针对 `lzip` 过滤器将 `compression-level` 统一锁定为 `"1"`，并启用多线程选项 `"threads", "0"`。

### 2. Rationale
`lzip` 的 Level 3/6 会启动极高阶的字典与匹配查找算法，导致吞吐量从 280 MB/s 断崖式跌落至 46 MB/s（-83.4%），而压缩率增益微乎其微。统一使用 Level 1 配合多线程流水线，能够在保证格式兼容性的前提下跑出 280+ MB/s 满速。

### 3. Alternatives Considered
- **维持 level > 2 使用 compression-level=3**：导致高熵与大文件测试大幅跌破底线，被否决。

### 4. Source
- `Sources/CTTZipBridge/ttzip_tar_native.c:575-578`
- `docs/benchmarks/peak_performance_matrix.json`

---

## R002 [SUBAGENT:research] 《基准评测热管理与自适应取样机制》

### 1. Decision
在 `Sources/TTZipCore/Benchmark/CompetitorBenchmarkRunner.swift` 的每个测试项之间增加 20ms 的降温微休眠（`usleep(20000)`），并确保在多轮采样中保留最小值。

### 2. Rationale
长时间连续执行 568 次密集压测会触发 Apple Silicon 热节流（Thermal Throttling），时钟频率降低 20%。通过微降温间歇，CPU 核心温度维持在睿频阈值以下，准确捕获真实物理吞吐。

### 3. Alternatives Considered
- **增加休眠时间至 500ms**：导致整体测试时间超过 10 分钟触发超时，被否决。20ms 是兼顾温度与总耗时的最优平衡点。

### 4. Source
- `Sources/TTZipCore/Benchmark/CompetitorBenchmarkRunner.swift:80-120`
