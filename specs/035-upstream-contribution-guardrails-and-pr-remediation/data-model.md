# Data Model: 上游开源贡献质量规范体系与 3 个 PR 严谨重构

> 对应工件：`specs/035-upstream-contribution-guardrails-and-pr-remediation/plan.md`  
> 契约定义：`contracts/upstream_pr_metadata.json`, `contracts/atomic_commit_sequence.json`

---

## 1. UpstreamPullRequestSubmission 实体

描述一个向上游开源仓库提交的 Pull Request 的完整元数据与合规断言模型。

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `prNumber` | `integer` | 否 | 上游 GitHub PR 编号（若已创建） |
| `issueNumber` | `integer` | 否 | 关联的 upstream Issue 编号 |
| `title` | `string` | 是 | 符合项目约定的 PR 标题 |
| `targetBranch` | `string` | 是 | 上游目标分支（通常为 `master` 或 `main`） |
| `sourceBranch` | `string` | 是 | 本地 Fork 推送的特性分支名称 |
| `baseCommitSha` | `string` | 是 | 干净检出点的 Commit SHA（如 upstream `22e3e20`） |
| `isDraft` | `boolean` | 是 | 是否处于 Draft 状态 |
| `scopeValidationPassed` | `boolean` | 是 | 物理断言 `git diff base..HEAD` 是否零污染 |
| `commits` | `AtomicCommitNode[]` | 是 | 该 PR 包含的按序原子提交清单 |

---

## 2. ReviewFindingModel 实体

描述 Reviewer 提出的每一条审查意见及其生命周期状态。

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `findingId` | `string` | 是 | 唯一标识符（如 `F-3388-001`） |
| `reviewer` | `string` | 是 | 审查人 GitHub 用户名（如 `stoeckmann`、`kientzle`） |
| `reviewerRole` | `string` | 是 | 枚举：`founder`, `collaborator`, `contributor` |
| `severity` | `string` | 是 | 枚举：`BLOCKER`, `HIGH`, `MEDIUM`, `LOW` |
| `category` | `string` | 是 | 枚举：`INTEGER_SAFETY`, `STREAM_DEFENSE`, `GIT_ISOLATION`, `TEST_ORACLE`, `COMMIT_GRANULARITY`, `COMMUNITY_PROCESS` |
| `targetFile` | `string` | 否 | 涉及的文件路径 |
| `lineRange` | `string` | 否 | 涉及的代码行范围 |
| `reviewerComment` | `string` | 是 | Reviewer 原始反馈内容 |
| `rootCause` | `string` | 是 | 深度技术根因分析 |
| `resolutionStatus` | `string` | 是 | 枚举：`OPEN`, `IN_PROGRESS`, `RESOLVED`, `WITHDRAWN` |
| `fixCommitIndex` | `integer` | 否 | 修复该问题的 Commit 序列索引 |

---

## 3. AtomicCommitNode 实体

描述拆解后的原子 Commit 结构，保证 Git Bisect 期间每个节点独立可编译。

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `sequenceIndex` | `integer` | 是 | 提交序号（从 1 开始递增） |
| `commitType` | `string` | 是 | 枚举：`infra`, `feat`, `test`, `fix`, `docs` |
| `subject` | `string` | 是 | 单行 Commit Subject（≤ 72 字符） |
| `body` | `string` | 是 | 详细 Commit Message Body |
| `filesIncluded` | `string[]` | 是 | 该 commit 包含的文件路径列表 |
| `compileCommand` | `string` | 是 | 验证该 commit 独立编译与测试的命令 |
