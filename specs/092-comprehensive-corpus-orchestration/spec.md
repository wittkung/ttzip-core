# Feature Specification: Comprehensive Corpus Orchestration & Geometric Mean Benchmark Matrix

## 1. Executive Summary

为 TTZip 构建 **5-Tier 科学多模态真实语料库编排与加权几何平均综合评测中枢 (Comprehensive Corpus Orchestration & Geometric Mean Benchmark Matrix)**。
将项目内置的 Silesia 12 语料全集 (211.9MB)、enwik8 (100MB) 及 HyperCompress 500 文件真实源码树，科学划分为 5 大标准场景（大文本、二进制机器码、结构化数据库、多文件源码树、科学图像矩阵），采用加权几何平均数（Weighted Geometric Mean）消除语料偏置与排序反转，输出符合学术与工业界公理的综合效能指数（CEI）与帕累托图表。

---

## 2. User Scenarios & Personas

- **场景 1（多维度无偏评测）**：评测不再局限于单一文本文件，用户可通过 5-Tier 矩阵全面评测 TTZip 与竞品在文本、二进制、数据库、多文件工程和科学矩阵上的各自分布与综合排名。
- **场景 2（科学综合效能指数）**：通过加权几何平均与 Cobb-Douglas 效用模型，生成标准化的 SPECScore 与 CEI 评分，直观展现综合效能帕累托前沿。
- **场景 3（零堆分配常驻生命周期）**：基准测试过程基于 `mmap` 只读虚拟内存池，消除 GC/内存分配抖动对微秒级物理性能时钟的干扰。

---

## 3. Functional Requirements

- **FR-001**: 定义 `BenchmarkTierCategory` 枚举，涵盖 Tier 1 (Large Text), Tier 2 (Binary Exec), Tier 3 (Structured/DB), Tier 4 (SourceTree & VFS), Tier 5 (Dense Matrix)。
- **FR-002**: 提供 `CorpusOrchestrator` 统一调度中枢，支持三级自适应发现（`Bundle.module`、环境变量、本地缓存），使用 POSIX `mmap` 提供零堆内存分配的全局只读数据切片与目录树路径。
- **FR-003**: 提供 `CompositeEfficiencyCalculator`，实现严格的加权几何平均数计算、Cobb-Douglas 综合效能指数（CEI）与以 Deflate L6 为基准的千分制 SPECScore。
- **FR-004**: 在 `ComprehensiveCorpusBenchmarkPkTests` 中集成 5 大 Tier 评测流水线，生成多语料综合效能帕累托图表 `pareto_composite_geometric.png` 及 JSON 报告。

---

## 4. Success Criteria

- **SC-001**: 5 大 Tier 语料库全部实现 100% 自动解析与零拷贝只读映射，无任何测试用例发生缺失。
- **SC-002**: 几何平均与 CEI 计算通过数学公理单元测试验证（满足参考系不变性与倒数一致性）。
- **SC-003**: 生成全新的综合效能帕累托图表并落盘。
- **SC-004**: 全量 530+ 单元测试与 6 级 CI/CD 本地门禁 100% 绿灯。
