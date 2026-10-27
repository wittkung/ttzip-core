---
trigger: model_decision
description: "执行 SpecKit 工作流，特别是多任务并发时，检查 feature 上下文与写入隔离。"
---

# SpecKit 上下文与并发隔离

- 仅在任务需要 SpecKit 时使用，不要求所有修改走完整流水线。
- 执行前确认实际包含 `.specify/` 的项目根目录；大仓根目录与模块根目录不一定相同。
- 并发执行时显式设置 `SPECIFY_FEATURE_DIRECTORY` 和 `SPECIFY_FEATURE_NO_PERSIST=1`，先核实本地脚本支持；不直接修改共享 `.specify/feature.json`。
- 使用独立 worktree 或划分互不重叠的文件范围，避免不同任务同时写同一份 spec、tasks 或源码。
- 是否委派依据任务、工具和当前会话约束决定，不按文件数强制派发，也不绑定工具名称。
- 使用对应版本的实际命令和参数；升级时保留本地定制，并在临时目录验证上下文解析和无持久化行为。
