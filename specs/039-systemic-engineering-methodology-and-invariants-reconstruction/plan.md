# Implementation Plan: 系统性工程方法论与底层不变量体系重塑

**Feature Directory**: `specs/039-systemic-engineering-methodology-and-invariants-reconstruction`  
**Created**: 2026-08-16  
**Status**: In Progress  
**Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/039-systemic-engineering-methodology-and-invariants-reconstruction/spec.md)

---

## Technical Context

- **目标**: 将从 `libarchive` 提炼的四大系统工程心法（Stream-First、Invariant-First、Bounds-First、Oracle-First）全面注入项目宪法、架构规范、方法论指南与代码库治理体系。
- **改动工件**:
  1. `.specify/memory/constitution.md`：增加《四大系统工程铁律》。
  2. `GEMINI.md`：同步四大铁律与底层不变量审查硬准则。
  3. `docs/architecture/systemic_engineering_methodology.md`：编写全景方法论与心智模型指南。
  4. `specs/039-systemic-engineering-methodology-and-invariants-reconstruction/contracts/chunked_solid_stream_spec.json`：固化 7z 分块流式压缩架构契约。

---

## Constitution Check

- [x] **Zero-Cost Abstraction**: 严格禁止热路径上出现批处理式全量分配与隐式内核页清零。
- [x] **Stream-First & Zero-Memory-Assumption**: 一切数据流动面向微缓冲与分块流式管道。
- [x] **No Subprocess**: 核心引擎坚持 100% In-process C 绑定。
- [x] **Zero Bare Logging**: 测试日志遵循 TTLogger 规范。

---

## Phase 0: Research Items

- R001: 《7z 分块流式 Solid 压缩状态机与滑动窗口算法》：研究基于 32MB/64MB 分块的 Solid Block 流式编码与 SubStreamsInfo 结构体连续派发模型。
- R002: 《系统级工程心智模型与防御性架构落地矩阵》：研究 POSIX 原语原子性、内存毒化清零与全生命周期确界防线。

---

## Phase 1: Artifacts & Contracts

- `data-model.md`: 定义 `SystemicMethodologySpec` 与 `ChunkedSolidStreamSpec` 实体模型。
- `contracts/systemic_engineering_spec.json`: 强类型系统工程方法论契约。
- `contracts/chunked_solid_stream_spec.json`: 7z 分块流式压缩接口契约。
- `quickstart.md`: 宪法断言与方法论验证指南。

---

## Phase 2: Implementation Checklist

### 1. 宪法与规则注入
- [ ] 更新 `.specify/memory/constitution.md` 注入四大系统工程铁律
- [ ] 更新 `GEMINI.md` 注入四大系统工程铁律与底层不变量准则

### 2. 方法论指南编纂
- [ ] 编写 `docs/architecture/systemic_engineering_methodology.md`

### 3. 契约固化与自测验证
- [ ] 生成全套 JSON Schema 契约并通过 Draft-07 强校验
- [ ] 执行快速验证自测
