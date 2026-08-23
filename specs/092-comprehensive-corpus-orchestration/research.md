# Research: Comprehensive Corpus Orchestration & Geometric Mean Benchmark Matrix

## R001: 压缩基准测试工业界语料库科学编排方案

- **Decision**: 建立 5-Tier 科学多模态语料库矩阵（Tier 1: Large Text & Web 25%, Tier 2: Binary Exec 20%, Tier 3: Structured/DB 20%, Tier 4: Mixed SourceTree 20%, Tier 5: Dense Matrix 15%）。基于 `Bundle.module` + 环境变量覆盖 + 用户缓存三级自适应发现，底层基于 POSIX `mmap` 零拷贝共享池管理。
- **Rationale**: 彻底消除单一文本语料（如仅用 enwik8）导致的算法族偏置、多文件 VFS 边界偏置、空间局部性偏置与窗口偏置。
- **Alternatives Considered**: 仅用 enwik8 单一文本（严重偏置，被否决）；动态网络下载（偶发网络抖动导致 CI 失败，被否决）。
- **Source**: Silesia Compression Corpus (Deorowicz, 2003), lzbench, Squash Compression Benchmark, TurboBench, Hutter Prize.

---

## R002: 多语料综合效能指数与加权几何平均数计算体系研究

- **Decision**: 全面采用加权几何平均数（Weighted Geometric Mean）聚合跨语料吞吐速率与压缩比，采用 Cobb-Douglas 效用模型计算综合效能指数（CEI）与以 Deflate L6 为 1,000 分基准的千分制 SPECScore。
- **Rationale**: 依据 Fleming & Wallace (1986) 及 SPEC CPU 规范，几何平均数具有尺度不变性（Scale Invariance）与倒数一致性（Inversion Invariance），彻底根除算术平均数引发的“排序反转悖论”，且不会被万兆级解压吞吐劫持。
- **Alternatives Considered**: 算术平均数（排序反转且被解压速率劫持，被否决）；纯调和平均数（对极致压缩算法惩罚过重，被否决）。
- **Source**: Fleming & Wallace (1986), CACM 29(3); SPEC CPU 2017/2026 Benchmark Rules; TurboBench Pareto Skyline.
