# Tasks: 上游开源贡献质量规范体系与 3 个 PR 严谨重构交付

**Input**: Design documents from `specs/035-upstream-contribution-guardrails-and-pr-remediation/`  
**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/)

## Format: `[ID] [P?] [Story] Description`
- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3, US4)

---

## Phase 1: Setup & Prerequisite Governance (User Story 1 - Priority: P1)

**Goal**: 升级全局代码审查规范与建立专属上游贡献 SOP，从根源确立工程防线。

- [x] T001 [P] [US1] 升级全局代码审查规范，在 `/Users/kevintung/.agents/skills/code-review/SKILL.md` 中新增第 5 节《系统级 C / 跨平台防御性编程审查规范》（包含 32/64 位溢出 clamp、流式 I/O NULL 判空、状态消费返回值检查）
- [x] T002 [P] [US1] 创建开源上游贡献专员技能规范在 `/Users/kevintung/.agents/skills/upstream-contribution/SKILL.md`（包含 Git 纯净分支物理隔离断言、原子 Commit 序列编排、官方原生预言机对齐与社区沟通礼仪）
- [x] T003 [US1] 更新 `/Users/kevintung/Documents/dev/TTZip/GEMINI.md`，在第七节注入上游贡献审查硬门禁协议


**Checkpoint**: 规范与硬门禁就绪，所有后续 PR 代码修改必须无条件遵循此标准。

---

## Phase 2: PR #3391 (CRC32 硬件加速) 纯净分支重建与测试预言机对齐 (User Story 3 - Priority: P1)

**Goal**: 彻底清除 PR #3391 的 7z 污染代码，从 clean upstream master 重建分支，全面接入 `bitcrc32()` 黄金预言机。

- [x] T004 [US3] 从 `origin/master` (`22e3e20`) 检出纯净分支 `armv8-crc32-acceleration`，物理断言无任何上游之外的脏 commit
- [x] T005 [US3] 实现纯粹的 ARMv8 ACLE 硬件加速在 `Vendor/libarchive-upstream/libarchive/archive_crc32.h`
- [x] T006 [US3] 重写 `Vendor/libarchive-upstream/libarchive/test/test_archive_crc32.c`，使用 `test_utils.h` 中的 `bitcrc32()` 黄金预言机替代所有硬编码常量
- [x] T007 [US3] 在 `Vendor/libarchive-upstream/CMakeLists.txt` 和 `Makefile.am` 中注册新测试
- [x] T008 [US3] 编译并运行 CRC32 单元测试 `libarchive_test test_archive_crc32` 验证 5 组用例 100% 通过
- [x] T009 [US3] 物理断言 `git diff origin/master..HEAD --stat` 严格仅有 4 个文件，创建单一原子 commit 并更新远端分支


**Checkpoint**: PR #3391 达到 100% 纯净、测试预言机完全对齐、零无关代码泄露。

---

## Phase 3: PR #3388 (7z AES-256 解密) 深度防御修复与原子 Commit 序列拆分 (User Story 2 - Priority: P1)

**Goal**: 消除 @stoeckmann 指出的 5 大技术隐患，将 7z AES 解密重构为 3 个独立可二分编译的原子 Commit。

- [x] T010 [US2] 从 `origin/master` (`22e3e20`) 检出纯净分支 `feat/7z-aes256-decryption`
- [x] T011 [US2] [Commit 1/3: Infra] 实现 `archive_cryptor.c` 和 `archive_cryptor_private.h` 对称加密接口并提交
- [x] T012 [US2] [Commit 2/3: Feat] 在 `archive_read_support_format_7zip.c` 中实现 7z 解密流水线：注入 32 位 clamp 保护、`buff_in == NULL` 判空、`__archive_read_consume` 错误捕获、结构体移至顶部，并提交
- [x] T013 [US2] [Commit 3/3: Test] 添加 3 组加密回归测试 `test_read_format_7zip_encryption_*.c` 并在 `Makefile.am` 和 `CMakeLists.txt` 注册并提交
- [x] T014 [US2] 编译并运行全量 7z 加密回归测试，验证全部通过
- [x] T015 [US2] 物理断言 `git log origin/master..HEAD --oneline` 严格呈现 3 个原子 commit，零 CRC32 污染，并更新远端分支


**Checkpoint**: PR #3388 5 项 Review 缺陷 100% 闭环，commit 结构严格满足上游可二分编译标准。

---

## Phase 4: PR #3393 (磁盘预分配) 技术答辩与设计演进 (User Story 4 - Priority: P2)

**Goal**: 针对 kientzle 提出的稀疏文件与小文件疑问，形成详实的技术答辩分析文档，为 Issue #3392 达成社区共识提供事实依据。

- [x] T016 [US4] 深入分析 `archive_write_disk_posix.c` 源码中的稀疏文件检测机制（`archive_entry_sparse_count` 与 `ARCHIVE_EXTRACT_SPARSE`）以及小文件预分配系统调用开销
- [x] T017 [US4] 在 Issue #3392 中沉淀客观、专业的技术答辩回复草案，保持 PR #3393 为 Draft 等待 Maintainer 共识


**Checkpoint**: 完成全套 PR 重构与技术答辩闭环，准备提交通报。

---

## Phase 5: Git Physical Worktree Isolation (User Story 5 - Priority: P1)

**Goal**: 建立完全隔离的本地物理工作区，根除分支切换时的构建产物与分支派生链交叉污染。

- [x] T018 [US5] 为 3 个 PR 分别创建独立的 `Vendor/worktrees/pr-*` 物理工作目录
- [x] T019 [US5] 将 `Vendor/worktrees/` 添加入 `.gitignore` 确保零脏文件泄露
- [x] T020 [US5] 验证各工作区独立 CMake 配置与 `ctest` 执行互不干扰

---

## Phase 6: Adversarial Audit & Low-Priority Hardening (User Story 6 - Priority: P1)

**Goal**: 执行最高标准批判性全量审计，消除一切代码风格、内存安全、测试覆盖及沟通礼仪盲点。

- [x] T021 [US6] 密钥清理死码消除防御：在 `archive_read_support_format_7zip.c` 中引入 `volatile` 函数指针模式 `archive_7zip_secure_clear`
- [x] T022 [US6] 敏感向量全面擦除：在 cleanup 路径增加 `aes_iv` 显式擦除并在释放前清理解密缓冲区
- [x] T023 [US6] 头文件包含顺序整理：将所有 `#include` 指令在 `archive_read_support_format_7zip.c` 顶部严格归组
- [x] T024 [US6] 硬件 CRC 宏精简：在 `archive_crc32.h` 中精简宏分支，移除编译器 pragma，增加大端序 `!defined(__ARM_BIG_ENDIAN)` 保护
- [x] T025 [US6] 阈值旁路负向测试：在 `test_write_disk_preallocate.c` 中新增 32KB 小文件旁路负向测试 `test_write_disk_preallocate_small_file`
- [x] T026 [US6] 沟通回复谦逊度重构：重新撰写全部 11 条 Review 回复草稿，诚恳致谢、承认不足并主动向创始人及核心贡献者征求专业建议

---

**Final Checkpoint**: 100% 审计通过，13/13 单元测试绿灯，代码与沟通草稿达到最高开源贡献标准。

