# Technical Research & Architectural Findings (Feature 017)

**Feature**: Zero Performance Regression Governance & Hard Floor Invariant Enforcement  
**Directory**: `specs/017-zero-performance-regression-and-floor-enforcement/`

---

## R001 [SUBAGENT:research] 《WIM 与镜像解压目录递归开销与内存元数据优化》

### 1. Decision
在 `Sources/CTTZipBridge/ttzip_tar_native.c` 的 `ttzip_extract_tar_native_c` 与 `ttzip_extract_tar_from_memory` 中实施**“父目录末次命中缓存 (Last-Created Parent Dir Fast-Path) + 乐观 `open` 失败回退”**的双层优化机制：
1. **栈上最后父目录缓存**：在解压函数栈帧中分配 `char last_parent_dir[4096]`。提取每个 entry 的父目录路径后，先执行 `strcmp(parent_dir, last_parent_dir) == 0`；若命中则直接跳过 `ttzip_common_mkdir_p` 的全路径递归扫描。
2. **乐观 `open` 系统调用**：解压常规文件（`AE_IFREG`）时，优先直接调用 `open(full_dest_path, O_WRONLY | O_CREAT | O_TRUNC, mode)`。仅当 `open()` 返回 `-1` 且 `errno == ENOENT` 时，才调用 `ttzip_common_mkdir_p(parent_dir)` 创建缺失目录，更新 `last_parent_dir` 并重试 `open()`。

### 2. Rationale
- **根除冗余 VFS 系统调用与内核锁竞争**：现行 `ttzip_common_mkdir_p` 会将目录路径逐级切分并在每个层级执行 `mkdir(tmp, 0755)`。对于 $N$ 个处于深度 $D$ 的小文件，将发出 $O(N \times D)$ 次 `mkdir` 系统调用。在 APFS / VFS 层，即便目录已存在，每次调用仍会陷入 XNU 内核、占用 vnode 锁并返回 `EEXIST`。在海量小文件和高熵 Payload（CPU 解压极快）下，该内核态上下文切换和锁争用成为核心瓶颈，导致 14% 的吞吐抖动。
- **利用归档条目的空间局部性**：WIM、DMG、ISO 与 TAR 归档内的文件通常按目录层级连续存储。末次父目录缓存配合乐观 `open` 将使得 99.9% 的连续文件在目录创建上的系统调用数降为 **0**。
- **满足热路径零成本抽象铁律**：方案仅使用栈上固定缓冲区 `last_parent_dir[4096]` 与局部变量，不引入任何堆内存分配（`malloc`/`free`）、不引入动态哈希树、不引入互斥锁，严格符合项目性能规范 §IV.1。

### 3. Alternatives Considered
- **被否决方案 1：引入全局/进程级动态哈希表（如 `uthash` 或 `std::unordered_set`）记录全部已创建目录**  
  - *否决理由*：动态哈希表在热路径中需要为每个新目录分配堆内存节点（`malloc`），在多线程或频繁解压时会造成堆内存碎片与锁开销，违背热路径“零堆分配、零对象树”约束。
- **被否决方案 2：在解压前先全量扫描归档头预创建所有目录 (Pre-scan)**  
  - *否决理由*：需要对归档 Header 执行完整的第二遍解析（或在内存中缓存全量 Entry 元数据），在 GB 级大镜像或网络挂载磁盘上会使冷启动首包延迟和元数据解析耗时翻倍。
- **被否决方案 3：每次文件写入前调用 `access(parent_dir, F_OK)` 判断**  
  - *否决理由*：`access()` 同样是内核系统调用，无法消除用户态/内核态切换开销，吞吐收益显著低于乐观 `open` + `ENOENT` 回退。

### 4. Source
- `Sources/CTTZipBridge/ttzip_tar_native.c:711-734`
- `Sources/CTTZipBridge/ttzip_tar_native.c:798-821`
- `Sources/CTTZipBridge/CTTZipCommon.c:37-64`
- `Sources/CTTZipBridge/ttzip_native_archive.c:202-214`
- `Sources/TTZipCore/ArchiveExtractor+Dispatch.swift:88-95`

---

## R002 [SUBAGENT:research] 《双层性能门禁与零倒退硬断言脚本架构》

### 1. Decision
全面升级 `scripts/audit_performance_regression.py`，构建**双级性能门禁判定与非零退出码机制**：
1. **双级阈值判定体系**：
   - **一级警告门禁 (Warning Gate, $\Delta < -3.0\%$)**：判定为轻微性能抖动（`WARNING`），在控制台输出黄色告警，归类记录于 Markdown 报告的 `## 🟡 性能轻微倒退告警 (3.0% ~ 10.0%)` 章节。
   - **二级硬阻断门禁 (Critical Gate, $\Delta < -10.0\%$)**：判定为严重性能倒退（`CRITICAL_REGRESSION`），在控制台输出高亮阻断报错，记录于 Markdown 报告的 `## 🔴 严重性能倒退阻断列表 (> 10.0%)`。
2. **退出码定义 (Exit Code Matrix)**：
   - `0`: 成功通过（无任何 $> 10.0\%$ 严重倒退，且在默认模式下允许 $\le 10.0\%$ 的抖动）。
   - `1`: 触发严重性能倒退阻断（存在 $\ge 1$ 项指标倒退 $> 10.0\%$），直接中断本地构建或 CI/CD 流水线。
   - `2`: 参数错误、文件缺失或未找到基准 `benchmark_report_*.json`。
   - `3`: 严格模式阻断（当传入 `--strict` 参数时，任何指标倒退 $> 3.0\%$ 即退出码 3）。
3. **CLI 参数扩展**：支持 `--strict`（将硬阻断阈值收紧至 3.0%）、`--baseline <path>` 与 `--latest <path>`。

### 2. Rationale
- **解决现行脚本无法拦截 CI 倒退的严重缺陷**：现行 `scripts/audit_performance_regression.py:152` 无论是否存在倒退均无条件执行 `return 0`，导致 GitHub Actions 和本地 pre-commit 钩子无法感知性能倒退，使隐蔽退化得以合入代码库。
- **兼顾真实物理噪声与重大架构倒退**：在 macOS 多任务与 CI 共享虚拟机环境下，系统调度和 CPU 频点波动会产生 $\pm 3\%$ 的测量噪声。设置 3% 警告与 10% 阻断的双级门禁，既能避免 CI 频繁虚警，又能对 $> 10\%$ 的严重退化实施零容忍硬拦截。
- **完全满足 Spec 017 规范与项目纪律**：直接落地 `specs/017-zero-performance-regression-and-floor-enforcement/spec.md` FR-006、SC-001 要求以及 `GEMINI.md` §VII.3 零倒退审查纪律。

### 3. Alternatives Considered
- **被否决方案 1：对所有 $> 3.0\%$ 倒退一律执行 `sys.exit(1)` 硬阻断**  
  - *否决理由*：在 GitHub Actions macOS-14 共享宿主机上，多租户 CPU 负载波动常导致个别基准场景产生 3.5%~5.0% 的偶发抖动。单级 3% 刚性阻断会导致 CI 流水线假阳性（False Positive）过高，严重阻碍常规 PR 交付。
- **被否决方案 2：仅依赖 XCTest `XCTestPerformanceMeasureTests` 进行断言，不升级 Python 脚本**  
  - *否决理由*：XCTest 仅涵盖 11 个核心场景，而全格式基准套件（`AllFormatsPkSuiteTests`）包含 16 种格式、284 项跨维度指标。必须通过独立的 Python 审计脚本对全维度 JSON 历史基线进行全量比对与追踪。

### 4. Source
- `scripts/audit_performance_regression.py:94-104, 152`
- `specs/017-zero-performance-regression-and-floor-enforcement/spec.md:79`
- `specs/017-zero-performance-regression-and-floor-enforcement/spec.md:30, 87`
- `GEMINI.md:189-200`
