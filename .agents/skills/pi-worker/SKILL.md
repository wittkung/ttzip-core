---
name: pi-worker
description: Delegate isolated coding tasks, local batch refactoring, or offline model execution to the lightweight Pi CLI agent.
---

# Pi Worker Agent Delegation Skill

## Overview
Pi Worker is an ultra-lightweight, high-speed coding agent runtime driven by `@earendil-works/pi-coding-agent`. Antigravity delegates mechanical, repetitive, or compute-heavy coding tasks to Pi workers via native subagents or headless batch execution.

## Usage Protocol & Rules
1. **Tool Restriction**: Only use 4 operations: `view_file` (with line slices), `replace_file_content` (exact substring matching), `write_to_file` (atomic new files), and `run_command` (deterministic bash).
2. **Zero Fluff**: Do not output preamble or conversational filler. Immediately invoke tools.
3. **Context Shielding**: Isolate file operations in subagents and return concise git-diff summaries to the main agent.
