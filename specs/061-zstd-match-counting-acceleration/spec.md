# Feature Specification: Zstandard Match Counting SWAR SIMD Acceleration & Upstream Alignment

**Feature Branch**: `061-zstd-match-counting-acceleration`

**Created**: 2026-08-17

**Status**: Draft

**Input**: User description: "facebook/zstd (Zstandard) Match Counting SWAR / SIMD 向量比对、哈希查找表、全零块预判定、双平台收益与利用方案"

---

## 1. User Scenarios & Testing *(mandatory)*

### User Story 1 - 极速 Zstandard / TAR.ZST 归档与解包体验 (Priority: P1)

用户在 macOS（Apple Silicon）或 Windows 平台上使用 TTZip 压缩海量数据或实时备份归档为 TAR.ZST 格式，底层 match finder 与 match counter 能够充分调用 64-bit SWAR 与 128-bit NEON / AVX2 硬件向量指令，提供零顿挫、满吞吐的压缩与解包体验。

**Why this priority**: TAR.ZST 与 ZSTD 是现代超大规模数据处理与无损备份的黄金标准，其性能直接决定了用户在大型工程构建与多媒体备份时的等待时间。

**Independent Test**:
通过 `ttzip-cli bench -f tar.zst` 以及单元测试验证 TAR.ZST 压缩与解压吞吐，在常规混合数据集（如 Silesia、代码仓库、二进制文件）上实现毫秒级响应并超越既定门禁指标。

**Acceptance Scenarios**:
1. **Given** 用户选择包含数十 GB 代码和数据的文件夹打包为 TAR.ZST，**When** 启动压缩，**Then** 压缩引擎在多核并行与硬件向量加速下稳定运行，吞吐达标且 CPU 利用率平稳。
2. **Given** 生成的 `.tar.zst` 文件，**When** 使用系统官方 `zstd` / `tar --zstd` 或 7-Zip 解包，**Then** 解压完全成功且解压后文件的 SHA-256 校验和 100% 一致。

---

### User Story 2 - 异构数据自适应匹配与稀疏全零快速旁路 (Priority: P2)

当用户压缩包含大量稀疏全零数据（如未分配虚拟磁盘镜像、稀疏数据库文件）或高复用文本时，系统能自动识别全零块并快速旁路跳过密集哈希计算，而在遇到长公共前缀时无缝切换为 128-bit 向量展开。

**Why this priority**: 稀疏文件与高重复度数据在日常归档中广泛存在，向量旁路能够避免无谓的 CPU 周期浪费。

**Independent Test**:
针对全零测试用例与 Silesia 字典用例执行微基准测试，验证零块探测延迟与长前缀比对速率。

**Acceptance Scenarios**:
1. **Given** 输入流中包含连续的 64 字节以上全零数据，**When** 进入扫描阶段，**Then** 零分支预判逻辑直接判定并短路，不进入哈希链表的逐项遍历。
2. **Given** 输入流中存在大于 16 字节的长匹配前缀，**When** 计算匹配长度，**Then** 自动触发 128-bit NEON / 64-bit SWAR 展开，单周期处理 16 字节比对。

---

### User Story 3 - 上游开源兼容性与跨平台跨架构一致性 (Priority: P3)

TTZip 对 Zstandard 的调用与底层算法抽象保持 100% 官方规范对齐，算法可独立提炼为补丁向上游 `facebook/zstd` 贡献，且在 macOS (ARM64/x86_64) 与 Windows (x64 AVX2) 平台均具备可验证的确定性行为。

**Why this priority**: 遵循开源标准与跨平台架构可维护性，避免平台私有非标行为导致数据损坏。

**Independent Test**:
在不同对齐边界（0~7 字节偏移）、边界尾部（<8 字节）以及大端/小端环境下运行跨架构比对测试。

**Acceptance Scenarios**:
1. **Given** 任意未对齐的内存地址（非 8 字节对齐）与尾部残余片段（1~7 字节），**When** 调用匹配计数与哈希计算，**Then** 算法通过安全边界保护与标量收敛，无 ASan 越界访问、零未定义行为。

---

### Edge Cases

- **未对齐与尾部越界**：输入缓冲区末尾不足 8 字节或 16 字节时，算法严格限制读取上界，杜绝读取超出有效内存边界。
- **全零与高熵随机数据交替**：在零块突变到随机高熵数据时，哈希与计数器能够平滑切换，避免命中率骤降引起的流水线阻塞。
- **内存极端受限场景**：大字典模式（如 WindowLog > 27）在内存吃紧时自适应回退到合理窗口大小，防止 OOM 崩溃。

---

## 2. Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: 必须在匹配比对核心中统一采用 **两级混合匹配长度计算体系（Tier 0 64-bit SWAR + Tier 1 128-bit NEON/AVX2）**，首 8 字节通过通用寄存器异或比对单周期定位，中长匹配通过 128 位向量展开。
- **FR-002**: 必须提供 **ARMv8 硬件 CRC32 指令 (`__crc32w`) 与通用乘法哈希的自适应分发**，在支持硬件加速的 CPU 上实现单周期哈希计算与更优离散度。
- **FR-003**: 必须支持 **64 字节向量全零预判定旁路**，在稀疏数据段实现零哈希检索开销并快速直通。
- **FR-004**: 必须保证 **零内存越界与未定义行为**，所有基于 SWAR 与向量的内存读取必须具备 `len + sizeof(T) <= max_len` 边界校验与尾部标量安全收敛。
- **FR-005**: 必须全面对齐 `facebook/zstd` 官方规范，生成的 Zstandard / TAR.ZST 压缩包必须通过官方 `zstd` CLI 与 libarchive 的严格解压校验。
- **FR-006**: 必须满足 `GEMINI.md` 规定的性能底线与零倒退硬门禁（TAR.ZST Direct 打包 >= 15,000 MB/s Debug / >= 22,000 MB/s Release，解压 >= 10,000 MB/s）。

---

### Key Entities

- **MatchFinderContext**: 维护滑动窗口、哈希查找表（单哈希/双哈希）及链表索引的上下文结构。
- **MatchCounter**: 封装两级 SWAR/SIMD 匹配计数逻辑的原语组件，提供跨架构一致的 `count_common_prefix` 接口。
- **DirectStreamPacker**: 负责 TAR.ZST 零拷贝直接流式打包的 I/O 调度通道。

---

## 3. Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001 (正确性与兼容性)**：所有 Zstandard 与 TAR.ZST 单元测试、黄金语料库测试 100% 通过，双向差分测试零误码。
- **SC-002 (微基准提升)**：底层公共前缀匹配比对（Match Length Counting）在 Apple Silicon 上的吞吐保持在 >= 4.5 GB/s。
- **SC-003 (端到端吞吐达标)**：TAR.ZST Direct 打包与解压保持历史最优表现，零性能倒退（$\Delta \ge -3.0\%$）。
- **SC-004 (安全性与确定性)**：AddressSanitizer (ASan) 与 UndefinedBehaviorSanitizer (UBSan) 全矩阵扫描 0 报错。

---

## 4. Assumptions

- 目标平台以 macOS 14.0+ (Apple Silicon NEON 为核心) 与现代 x86_64 / Windows 为主，均原生支持 64 位未对齐加载指令。
- 采用 BSD 3-Clause 开源协议，与 TTZip 及 upstream 完全合规。
- 绝不修改任何冻结在 `.agents/rules/zip-engine-freeze.md` 中的 ZIP 核心文件。
