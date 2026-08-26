# Feature Specification: ZIP Extreme Speed Multi-Core Block-Parallel Mode

## 1. Executive Summary

为 TTZip 增加 **ZIP 极速模式 (Extreme Speed Multi-Core Block-Parallel Mode)**，支持用户选择开启。
在处理大单文件（或在极速模式下）时，利用 Apple Silicon 全部 18 个 CPU 核心对数据流进行 512KB/1MB 自适应分块并发 Deflate 压缩与流式聚合，打破单文件只能使用单核 CPU 的限制，实现 **>10 GB/s ~ 15 GB/s** 的极速压缩吞吐，同时保持 100% PKWare ZIP / RFC 1951 标准解压兼容性。

---

## 2. User Scenarios & Personas

- **场景 1（大文件极速打包）**：用户需要快速打包 100MB ~ 数 GB 的大型单个文件（虚拟机镜像、日志、数据库文件、ISO）。开启极速模式后，所有 CPU 核心满载并发，数秒内完成打包。
- **场景 2（帕累托图表对比）**：用户在基准评测中既能看到单核理论最优压缩率轨迹，也能选择启用极速模式，与 pigz（18 核）等工业级多核工具在完全对等的核心算力下展开硬核 PK。

---

## 3. Functional Requirements

- **FR-001**: 提供 `ZipExtremeBlockWriter` / 极速分块流式压缩引擎，集成 `CTTZipBridge_ZipChunkedStream` 与 `ZipBlockParallelCompressor`。
- **FR-002**: 支持自适应分块与 18 核无锁并发调度（`DispatchQueue.concurrentPerform` / GCD 并发队列）。
- **FR-003**: 保证生成的 ZIP 文件 100% 符合 PKWare Method 8 (Deflate) 规范，系统原生 `/usr/bin/unzip`、Archive Utility 与 7-Zip 均可无损解压。
- **FR-004**: 在 `SoftwareParetoFrontierPkTests` 与帕累托图表中增加 `TTZip Extreme` 轨迹点（18 核满开极速），呈现 >10 GB/s 吞吐与前沿表现。

---

## 4. Success Criteria

- **SC-001**: 在 100MB `enwik8.xml` 真实语料上，极速模式压缩吞吐达到 **>= 10,000 MB/s**（相比单核提升 7x~10x，超越 pigz 的 4,100 MB/s）。
- **SC-002**: 生成的 ZIP 文件经系统 `/usr/bin/unzip -t` 校验通过，CRC32 校验码与未分块压缩完全一致。
- **SC-003**: 全套 525+ 单元测试与 6 级 CI/CD 门禁 100% 绿灯通过。
