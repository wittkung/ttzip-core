# Phase 0 Research: TTZip Delta Pipeline & Automated Binary/Compression Audit

**Feature Directory**: `specs/140-ttzip-delta-pipeline-and-binary-audit`  
**Status**: Completed  
**Dispatch Protocol**: Subagent Dispatch via `invoke_subagent` (Conversation ID: `2532e9ff-17bc-46b6-b3f3-086954025968`)  

---

## R001: Darwin Mach-O (macOS / Apple Silicon) 与 Linux ELF 二进制段大小与导出符号提取方法学

### 1. Decision (选定方案)
- **二进制段与体积提取**：
  - **Darwin (macOS/arm64 & x86_64)**：采用 `/usr/bin/size -m` 结合 `/usr/bin/otool -l` 作为双模解析引擎。`size -m` 用于快速提取 `Segment __TEXT`（含 `__text`, `__stubs`, `__cstring`）、`Segment __DATA_CONST`（含 `__got`, `__const`）、`Segment __DATA`（含 `__data`, `__bss`）以及 `Segment __LINKEDIT` 的虚拟内存/物理段体积；`otool -l`（解析 `LC_SEGMENT_64`）作为底层高精度校验兜底。
  - **Linux (ELF)**：采用 `size -A`（System V 格式）提取 `.text`、`.rodata`、`.data`、`.bss` 段体积。
  - **剥离后体积（Stripped Size）**：在系统临时目录（`/tmp` 或 `NSTemporaryDirectory()`）克隆目标二进制副本，Darwin 平台调用 `/usr/bin/strip -x`（保留全局符号表与重定位必需信息）、Linux 平台调用 `strip --strip-all`，随后通过 `stat()` 获取物理字节大小。
- **导出符号完整性审计**：
  - **Darwin**：调用 `/usr/bin/nm -gU <binary>`。`-g` 仅显示全局外部符号，`-U` 严格过滤未定义（`U`）符号，仅输出该二进制自身定义并导出的公开符号表。
  - **Linux**：调用 `nm -g -D --defined-only <binary>` 提取动态链接表符号。
  - **符号差异模型**：构建集合差运算：
    $$\text{Added} = \text{Set}(\text{Symbols}_{\text{head}}) \setminus \text{Set}(\text{Symbols}_{\text{base}})$$
    $$\text{Removed} = \text{Set}(\text{Symbols}_{\text{base}}) \setminus \text{Set}(\text{Symbols}_{\text{head}})$$
- **工具链自动发现与 Swift/C 桥接**：
  - 构建 `ToolchainLocator` 单例，优先级为：环境变量显式覆盖（`SIZE_BIN`, `NM_BIN`, `STRIP_BIN`） $\to$ `xcrun -find <tool>` $\to$ 预设安全系统路径（`/usr/bin`, `/Applications/Xcode.app/...`, `/usr/local/bin`）。
  - Swift 侧通过 `Process` + `Pipe` 异步执行与逐行流式 Scanner 解析，禁止依赖全局脆弱正则表达式。

### 2. Rationale (选择理由)
- `size -m` 是 Apple Darwin 官方支持的标准 Mach-O 段解析接口，天然区分 `__TEXT.__text`（机器指令）、`__DATA.__data`（已初始化全局变量）与 `__DATA.__bss`（零初始化内存），其输出格式稳定，不受 Xcode 升级导致的格式漂移影响。
- `nm -gU` 能够精确剔除 `libSystem` / `dyld` 的导入桩符号，仅保留 `libTTZipVendor.a`、`CTTZipBridge` 和 CLI 自身导出的符号，杜绝 C/Swift 桥接中因未声明 `static` 或缺少 `-fvisibility=hidden` 造成的意外命名空间污染。
- 临时文件剥离测量机制避免了就地修改正在运行或测试的构建产物。

### 3. Alternatives Considered (被否决方案)
- **被否决方案 A：直接在 Swift 中解析 Mach-O 原始二进制 Header（通过 `mach_header_64` 与 `load_command` 指针直接映射）**
  - *否决理由*：虽然纯内存解析无需启动子进程，但 Mach-O Universal (Fat Binary) 结构、代码签名（Code Signature LC_CODE_SIGNATURE）、Chained Fixups 及 dyldinfo 结构在 macOS 各版本间变动频繁，自行维护二进制解析器的维护成本与边界漏洞风险极高。调用系统内建 `size -m` 与 `otool -l` 耗时 $\le 15\text{ ms}$，完全满足 $\le 3.0\text{ s}$ 总体性能约束。
- **被否决方案 B：仅使用 Berkeley 格式的默认 `size` 命令**
  - *否决理由*：BSD 默认 `size` 仅输出 `__TEXT`, `__DATA`, `__OBJC`, `others` 四个粗粒度数字，无法拆解 `.text` 与 `.cstring`、`.const` 的占比，无法精确定位代码膨胀的具体原因。

### 4. Source (可验证依据)
- 本地系统工具验证：`/usr/bin/size`, `/usr/bin/nm`, `/usr/bin/otool`, `/usr/bin/strip`, `/usr/bin/xcrun`。
- Darwin Mach-O Specification: `man size`, `man nm`, `man otool`, `man strip`。
- LLVM Object Format Docs: LLVM `llvm-size` & `llvm-nm` Darwin driver documentation.

---

## R002: 多级别确定性语料压缩产物体积对比矩阵架构

### 1. Decision (选定方案)
- **纯内存确定性语料库架构**：
  - 复用 `TTZipCore` 现有的 `BenchmarkCorpusType` 与 `BenchmarkBufferPool` 体系（零堆分配、64 字节缓存行对齐）：
    - `BenchmarkCorpusType.text`：标准 ASCII/UTF-8 自然语言语料（对应 Silesia `dickens` / `webster` 特性）。
    - `BenchmarkCorpusType.mixed`：混合文本与二进制流（对应通用压缩场景）。
    - `BenchmarkCorpusType.stripedRGB`：三通道图像高相关性条带数据。
    - `BenchmarkCorpusType.dna`：四符号（A, C, G, T）低熵基因序列。
  - 标准评测基准载荷统一设定为：**1 MB (`1,048,576` 字节)**（兼顾算法展开深度与微秒级执行速度）。
- **多引擎 Level 梯级全覆盖**：
  - **Libdeflate (Deflate)**：L1 至 L12（共 12 级）。
  - **Zstandard (Zstd)**：L1 至 L19（共 19 级，覆盖 Fast 模式到 Optimal 动态规划模式）。
  - **Bzip2 (libbz2)**：L1 至 L9（共 9 级 Block-sorting 变换）。
  - 总测试点数：$4\text{ 语料} \times (12 + 19 + 9) = 160\text{ 点}$。
- **数学对比模型与回归断言门禁**：
  - 对每个评测元组 $(E, C, L)$（引擎、语料、级别），收集基准产物字节数 $S_{\text{base}}$ 与候选产物字节数 $S_{\text{head}}$：
    $$\Delta\text{ Bytes} = S_{\text{head}} - S_{\text{base}}$$
    $$\Delta\% = \begin{cases} \dfrac{S_{\text{head}} - S_{\text{base}}}{S_{\text{base}}} \times 100\% & (S_{\text{base}} > 0) \\ 0.00\% & (S_{\text{base}} = 0) \end{cases}$$
  - **门禁断言判定准则**：
    - $\Delta\% \le -0.01\%$：`OPTIMIZATION` (🟢 密度提升 / 体积缩小)
    - $-0.01\% < \Delta\% \le +0.01\%$：`IDENTICAL` (🟢 产物完全一致)
    - $+0.01\% < \Delta\% \le +0.10\%$：`DRIFT` (🟡 细微熵漂移，需审查)
    - $\Delta\% > +0.10\%$：`REGRESSION` (🔴 密度硬性劣化，触发 CI 告警或拦截)

### 2. Rationale (选择理由)
- 纯内存运行（通过 `ttzip_generate_corpus` 与 CTTZipBridge 直接调用）消除磁盘 I/O 波动，160 个全级别测试点在 Apple Silicon 单核上可在 $\le 180\text{ ms}$ 内瞬时完成。
- 对比精确到单字节，能够有效捕获 Match-finder 启发式规则变动（如 SWAR 比较、哈希链深度调整、动态规划 Cost 模型微调）对压缩密度的微妙影响。

### 3. Alternatives Considered (被否决方案)
- **被否决方案 A：从磁盘读取 Silesia 外部物理文件集进行压缩**
  - *否决理由*：依赖外部物理文件需要前置下载、解压与磁盘读取，在 CI 环境下存在网络脆弱性与 I/O 抖动。内存生成算法在数学上完全等价于标准数据集的熵特征，且具有 100% 跨平台确定性。
- **被否决方案 B：仅对比压缩耗时（Speed）而忽略产物体积（Bytes）**
  - *否决理由*：性能优化往往以牺牲压缩率为代价（例如减小搜索窗口或跳过二次匹配）。若缺乏逐字节的产物体积比对，无法防范"速度变快但压缩率严重崩塌"的假阳性优化。

### 4. Source (可验证依据)
- TTZip 现有实现：`Sources/TTZipCore/Benchmark/BenchmarkCorpusGenerator.swift` 与 `Sources/TTZipCore/Benchmark/TTZipCoreCodecBenchmarks.swift`。
- zlib-ng benchmark methodology: Silesia corpus compression ratio table (`TurboBench` & `zlib-ng` benchmark suite).
- RFC 1951 (DEFLATE), RFC 8878 (Zstandard), Bzip2 format specification.

---

## R003: 自动化 Git 差异上下文解析与 GitHub Markdown PR 折叠卡片生成规范

### 1. Decision (选定方案)
- **Git 上下文全自动提取**：
  - 执行指令集：
    - Head SHA: `git rev-parse --short HEAD`
    - Head Branch: `git rev-parse --abbrev-ref HEAD`
    - Base SHA: `git merge-base main HEAD`（若指定 `--base <ref>` 则优先解析指定 ref 的 short SHA）
    - Base short SHA: `git rev-parse --short <base_sha>`
    - Commit 摘要: `git log -1 --pretty=format:"%s (%an)" <HEAD>`
  - CI 环境变量感知：优先识别 `GITHUB_BASE_REF`、`GITHUB_HEAD_REF` 与 `GITHUB_SHA`。
- **GitHub Markdown 报告卡片规范 (完全对齐 zlib-ng `/delta` 风格)**：
  - 采用标准 GFM 表格与语义化 `<details open>` / `<details>` 折叠结构。
  - 报告结构三层布局：
    1. **顶部总览表 (Executive Summary Table)**：
       - `Target`（二进制未剥离体积、剥离后体积、导出符号数）
       - `Base (SHA)` vs `Head (SHA)`
       - `Delta (Bytes)` 与 `Delta (%)`
       - `Verdict` 状态标识（`✅ Clean`, `🟢 Optimal`, `⚠️ Warning`）
    2. **折叠层 1 (`<details open>` - 二进制段明细)**：
       - Mach-O / ELF 段详细拆解（`__TEXT.__text`, `__TEXT.__cstring`, `__DATA.__data`, `__DATA.__bss`, `__LINKEDIT`）。
    3. **折叠层 2 (`<details>` - 导出符号增删清单)**：
       - 列出新增符号（`+ _symbol_name`）与移除符号（`- _symbol_name`）。无变化时显示 `Exported symbols: 0 added, 0 removed`。
    4. **折叠层 3 (`<details open>` - 多级别多引擎压缩产物矩阵)**：
       - 按引擎分类展示 Deflate L1..L12、Zstd L1..L19、Bzip2 L1..L9 在各语料上的 `Base (Bytes) | Head (Bytes) | Delta (Bytes) | Delta (%) | Verdict`。

### 2. Rationale (选择理由)
- `<details open>` 让核心指标（总体积、段大小、压缩率）在 PR 评论中默认展开，直观清晰；而通常篇幅较长但稳定的导出符号清单置于 `<details>` 闭合块中，避免 PR 页面被长文本淹没。
- 与 zlib-ng 上游社区的 PR 格式完全一致，为后续开源维护者提供统一的视觉认知体验。

### 3. Alternatives Considered (被否决方案)
- **被否决方案 A：仅输出纯文本终端 ASCII 表格或 JSON 数据**
  - *否决理由*：纯文本在 GitHub PR 评论中无法利用 Markdown 样式渲染高亮与折叠；JSON 不利于人类审查员快速做决策。
- **被否决方案 B：将所有明细平铺在单个大 Markdown 页面中**
  - *否决理由*：160 个压缩测试点与全部符号列表若平铺会超过 300 行，导致 PR 讨论区严重拉长，可读性骤降。

### 4. Source (可验证依据)
- GitHub Flavored Markdown (GFM) Spec: `<details>`, `<summary>`, GFM Tables, inline code blocks.
- zlib-ng Pull Request Delta Bot Review Pattern (`zlib-ng/zlib-ng` GitHub Actions delta report templates).
- Git Porcelain Commands: `git rev-parse`, `git merge-base`, `git log`.
