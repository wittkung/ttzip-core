# Feature Specification: Entropy-Adaptive Intelligent Extreme Routing

## 1. Executive Summary

为 TTZip 极速模式 (`ZipExtremeBlockWriter`) 引入 **微秒级轻量前置香农熵探测器与自适应智能分流引擎 (Entropy-Adaptive Intelligent Extreme Routing)**。
通过对输入数据前 4KB 执行超轻量 256-bin 字节直方图与信息熵分析（耗时 $< 1.5\mu\text{s}$）：
- **高熵不可压缩数据 ($H \ge 7.35\text{ bits/byte}$)**：自动透明降级为 PKWare Method 0 (Direct Store I/O)，跳过昂贵的 Deflate 字典查找，吞吐量直达 **>15,000 ~ 25,000 MB/s**，杜绝体积膨胀与无效 CPU 算力浪费；
- **低中熵可压缩数据 ($H < 7.35\text{ bits/byte}$)**：自动激活 18 核心饱和分块多核 Deflate 压缩（PKWare Method 8），维持 5,500+ MB/s 极速压缩。

---

## 2. User Scenarios & Personas

- **场景 1（混合多媒体与多格式归档）**：用户打包包含大量照片、视频、MP4/MKV 以及日志、代码的混合文件夹时，极速模式智能识别媒体文件并以 >20 GB/s 直通打包，文本代码自动多核压缩，总用时大幅缩短 60% 以上。
- **场景 2（零体积膨胀保障）**：对已压缩或加密的不可压缩数据，绝不产生多余字节膨胀，严格保证 CRC-32 比特精确对齐与原生解压 100% 兼容。

---

## 3. Functional Requirements

- **FR-001**: 在 C 桥接层与 Swift 层实现 `ttzip_probe_entropy_and_compressibility(data, size)`，在 4KB 前置采样内统计 256-bin 频次并计算香农熵与试探压缩比。
- **FR-002**: 扩展 `ZipExtremeBlockWriter`，在分块压缩前执行熵探测；当判定为高熵时，直接写入 PKWare Method 0 (Stored) 容器分块与 Central Directory。
- **FR-003**: 建立双向契约 `contracts/entropy-adaptive-routing-contract.json`，确保分流决策具有严格的输入/输出强类型约束。
- **FR-004**: 编写单元测试与高熵/低熵实测基准，并在帕累托图表中直观反映高熵极速穿透效果。

---

## 4. Success Criteria

- **SC-001**: 4KB 熵探测执行耗时 $\le 2.0\mu\text{s}$，热路径零堆内存分配。
- **SC-002**: 高熵数据（JPEG/MP4/Encrypted）实测吞吐量 $\ge 15,000\text{ MB/s}$（提升 300%+）。
- **SC-003**: 低熵数据（XML/Text/Code）维持 $\ge 5,000\text{ MB/s}$ 压缩吞吐且空间节省率 $\ge 95\%$。
- **SC-004**: 生成的所有 ZIP 包通过 `/usr/bin/unzip -t` 100% 校验。
- **SC-005**: 全量回归测试与 6 级 CI/CD 本地门禁 100% 绿灯。
