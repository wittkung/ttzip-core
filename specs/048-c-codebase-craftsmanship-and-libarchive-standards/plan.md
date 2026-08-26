# Implementation Plan: 048-c-codebase-craftsmanship-and-libarchive-standards

**Feature Name**: `048-c-codebase-craftsmanship-and-libarchive-standards`  
**Milestone**: Comprehensive C Codebase Modernization, Industrial Standards & libarchive Alignment  
**Dependencies**: [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)  

---

## 一、 技术上下文 (Technical Context)

全面重构 `Sources/CTTZipBridge/` 下的核心 C 头文件与源文件，对标 `libarchive` 黄金标准：

```mermaid
graph TD
    subgraph "C 桥接层工业级改造 (Feature 048)"
        H["C 头文件自解释契约<br>(CTTZipCommon.h, CTTZipIO.h, CTTZipSysAlloc.h, CTTZipBridge_Archive.h)"]
        SRC["C 实现文件加固<br>(CTTZipCommon.c, CTTZipIO.c, CTTZipSysAlloc.c, ttzip_lzma2_enc_native.c)"]
        H -->|@brief / @note [Ownership] / @param [in,out] / 错误码| AUDIT["100% 工业级契约"]
        SRC -->|Arena 安全释放 / 64位 Clamp / SSIZE_MAX 分块 / 死锁消除| AUDIT
    end
```

---

## 二、 架构原则审查 (Constitution Check)

1. **零成本抽象与热路径性能保持**：
   - 保持 Arena 连续大内存块分配与零额外堆分配，杜绝 per-block 小内存频繁借还。
2. **零性能倒退铁律**:
   - 保持 46 项全矩阵吞吐门禁 100% 达标。

---

## 三、 Phase 0: 深度技术调研 (Research)

- R001 [SUBAGENT:research] 《TTZip C 桥接层全量代码规范、Arena 内存所有权、64 位 Clamp 与并发死锁防御深度审计》

---

## 四、 Phase 1: 数据模型与契约 (Data Model & Contracts)

- [x] `data-model.md`: 定义 `CBridgeContractAudit`。
- [x] `contracts/c_bridge_contract_schema.json`: 强类型 Schema。
- [x] `quickstart.md`: 3 大验证场景。

---

## 五、 改动清单与组件设计 (Component Breakdown)

### 1. C 头文件契约重构
- `[MODIFY]` [`Sources/CTTZipBridge/include/CTTZipCommon.h`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/CTTZipCommon.h): 补齐 HeaderDoc、所有权、Clamp 宏与 6 级错误码体系。
- `[MODIFY]` [`Sources/CTTZipBridge/include/CTTZipSysAlloc.h`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/CTTZipSysAlloc.h): 补齐分配器说明与对齐释放职责。
- `[MODIFY]` [`Sources/CTTZipBridge/include/CTTZipIO.h`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/CTTZipIO.h): 补齐 I/O Entry 结构体文档与 SSIZE_MAX 保护说明。
- `[MODIFY]` [`Sources/CTTZipBridge/include/CTTZipBridge_Archive.h`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/CTTZipBridge_Archive.h): 补齐解密、检视与提取契约。

### 2. C 实现文件安全性与死锁防御加固
- `[MODIFY]` [`Sources/CTTZipBridge/CTTZipCommon.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipCommon.c): 统一 APFS 预分配与内存写屏障。
- `[MODIFY]` [`Sources/CTTZipBridge/CTTZipSysAlloc.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipSysAlloc.c): 强化算术溢出保护与统一 APFS 逻辑。
- `[MODIFY]` [`Sources/CTTZipBridge/CTTZipIO.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipIO.c): 修复 payload 释放遗漏与 SSIZE_MAX 分块。
- `[MODIFY]` [`Sources/CTTZipBridge/ttzip_lzma2_enc_native.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/ttzip_lzma2_enc_native.c): 统一 Arena 析构，修复错误分支 interior pointer 非法释放。
- `[MODIFY]` [`Sources/CTTZipBridge/CTTZipBridge_GzParallel.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipBridge_GzParallel.c): 修复失败分支条件变量唤醒，消除死锁隐患。
