# Implementation Plan: 上游开源贡献质量规范体系与 3 个 PR 严谨重构交付

**Feature Directory**: `specs/035-upstream-contribution-guardrails-and-pr-remediation`  
**Feature Branch**: `035-upstream-contribution-guardrails-and-pr-remediation`  
**Spec**: [spec.md](./spec.md)  
**Created**: 2026-08-16  
**Status**: Ready for Implementation

---

## 1. Technical Context & Upstream Architectural Invariants

### 1.1 Context
在向核心开源基础库 `libarchive/libarchive` 贡献特性时，由于未能严格遵循目标项目的极简可移植性哲学、流式 I/O 状态机防御规范与 Git 纯净分支隔离纪律，导致 3 个 PR（#3388 7z AES 解密、#3391 CRC32 硬件加速、#3393 磁盘预分配）暴露出代码缺陷、分支污染与沟通脱节问题。本项目旨在建立全套上游贡献质量保障体系，并对 3 个 PR 进行彻底的工业级技术重构。

### 1.2 Upstream C89 & POSIX Invariants
- **C89 Variable Declaration**: 所有局部变量必须置于函数块最顶部，严禁 C99 混杂声明。
- **Cross-Arch Integer Safety**: `int64_t` 向 `size_t` 转换必须使用 `(uint64_t)` 上限比较与 clamp，杜绝 32 位溢出。
- **Streaming Read/Consume Safety**: `__archive_read_ahead(a, 1, &avail)` 必须严格判定 `buff != NULL && avail > 0`；`__archive_read_consume(a, len)` 必须捕获 `< 0` 错误。
- **Independent Test Oracle**: 校验算法必须以 `test_utils.h` 中的 `bitcrc32()` 为唯一黄金标准。

---

## 2. Constitution Check

- [x] **Zero-Cost Abstraction & Freeze Compliance**: 不修改 TTZip Core 内部冻结文件；全部改动集中在 `Vendor/libarchive-upstream/` 与全局 Agent Skills 中。
- [x] **No Bare Printfs/Logs**: libarchive 内部所有错误必须经由 `archive_set_error()` 状态机向上传播。
- [x] **Spec Kit Multi-Agent Isolation**: 严格通过环境变量声明 `SPECIFY_FEATURE_DIRECTORY`，不覆盖全局 `feature.json`。

---

## 3. Phase 0: Research Items Index

- R001 [SUBAGENT:research] 《32位/跨架构整型截断防御》：在 C89 环境下安全截断 64-bit 偏移量至 size_t/UBUFF_SIZE (见 [research.md](./research.md#r001-32-bit--multi-arch-integer-truncation--safe-clamping-patterns))
- R002 [SUBAGENT:research] 《流式 I/O 状态机非满读与指针解引用安全》：__archive_read_ahead 与 __archive_read_consume 的错误传播不变式 (见 [research.md](./research.md#r002-libarchive-streaming-read-ahead--consumption-state-machine-invariants))
- R003 [SUBAGENT:research] 《PR #3388 原子 Commit 拆分与可编译性保障》：将 7z AES 划分为 3 个独立可编译的提交序列 (见 [research.md](./research.md#r003-atomic-commit-splitting-strategy-for-pr-3388))
- R004 [SUBAGENT:research] 《测试预言机对齐与公共 API 黑盒测试》：bitcrc32() 在 libarchive 测试套件中的集成与 Zip 端到端验证 (见 [research.md](./research.md#r004-libarchive-test-oracle-alignment--public-api-integration-test-patterns))

---

## 4. Phase 1: Artifacts Index

- **Data Model**: [data-model.md](./data-model.md)（定义上游 PR 元数据、审查问题分级模型、Commit 原子契约模型）
- **Contracts**: 
  - `contracts/upstream_pr_metadata.json` [SUBAGENT:research]（上游 PR 元数据与审查检查表 Schema）
  - `contracts/atomic_commit_sequence.json` [SUBAGENT:research]（原子 Commit 序列与文件拓扑 Schema）
- **Validation**: [quickstart.md](./quickstart.md)（包含验证命令、预期输出及失败诊断）

---

## 5. Component Modification List

### Component 1: Global Agent Skills & Governance
- `[MODIFY]` `/Users/kevintung/.agents/skills/code-review/SKILL.md` (注入系统级 C 跨平台防御与对抗性审查检查表)
- `[NEW]` `/Users/kevintung/.agents/skills/upstream-contribution/SKILL.md` (创建上游开源贡献全生命周期 SOP Skill)
- `[MODIFY]` `/Users/kevintung/Documents/dev/TTZip/GEMINI.md` (添加 upstream contribution 硬门禁规范)

### Component 2: PR #3391 (CRC32 Hardware Acceleration)
- `[MODIFY]` `Vendor/libarchive-upstream/libarchive/test/test_archive_crc32.c` (全量接入 bitcrc32() 黄金预言机)
- `[MODIFY]` `Vendor/libarchive-upstream/libarchive/archive_crc32.h` (确保 ACLE 硬件加速在 clean master 上的原子实现)
- `[NEW/REBUILD]` 分支 `armv8-crc32-acceleration` 从 upstream master (`22e3e20`) 纯净检出，仅包含 4 个文件

### Component 3: PR #3388 (7z AES-256 Decryption)
- `[MODIFY]` `Vendor/libarchive-upstream/libarchive/archive_read_support_format_7zip.c` (修复 32-bit 截断、read_ahead 边界、consume 返回值检查、struct 位置)
- `[REFACTOR]` 分支 `feat/7z-aes256-decryption` 拆分为 3 个原子 commit

### Component 4: PR #3393 (Disk Pre-allocation)
- `[DOCUMENT]` 准备技术事实答辩分析（解释 `archive_write_disk_posix.c` 稀疏文件与小文件自动旁路机制）
