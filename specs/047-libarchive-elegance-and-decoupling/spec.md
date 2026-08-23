# Feature Specification: 047-libarchive-elegance-and-decoupling

**Feature Name**: `libarchive-elegance-and-decoupling`  
**Status**: `Draft`  
**Target Milestone**: Industrial Code Standards, Architectural Decoupling & Comprehensive Documentation Alignment  
**Created**: 2026-08-17  

---

## 一、 业务动机与第一性原理 (Context & First Principles)

世界顶级开源基础库 `libarchive` 历经 20 年工业验证，其代码优雅性、分层解耦架构与专业级自解释注释体系被业界奉为典范。对比之下，TTZip 现有代码在部分模块仍存在**耦合过紧（业务逻辑与底层 I/O 揉杂）、注释单薄（缺少对内存所有权、生命周期、确界约束与错误语义的深度说明）、代码规范不统一**的问题。

本 Feature 旨在全面对标 `libarchive` 的代码哲学与架构标准，对 TTZip 核心层与 C 桥接层进行系统级重构升级：

1. **工业级三段式自解释文档体系 (Industrial DocC / Doxygen Documentation Standard)**：
   - 建立对标 `archive.h` 的全套 API 文档规范：涵盖 `@brief`（意图语义）、`@details`（架构原理与跨平台行为）、`@note`（所有权 Ownership、线程安全并发模型、锁级别）、`@param`/`@return`（确界与错误模型）。
2. **正交管道解耦架构 (Orthogonal Pipeline & Format-Filter Decoupling)**：
   - 全面解耦格式解析器（Format）、传输过滤器（Filter/Codec）与存储后端（Storage/IO），消除循环依赖，建立纯单向数据流。
3. **算法分支系统工程不变式注释 (Invariant & Rationale Documentation)**：
   - 在复杂硬件加速、SIMD 加解密、分卷拼接与路径安全分段处，深度记录“为什么这样做（Why）”与“被否决路径（Alternatives）”，而非仅陈述“做什么（What）”。
4. **强类型内存所有权与确界规范 (Ownership & Bounds Specification)**：
   - 显式声明借用（Borrowing）、转移（Ownership Transfer）与生命周期范围（Lifetime Scope），杜绝隐式所有权泄漏与并发数据竞争。

---

## 二、 用户故事 (User Stories)

### User Story 1 (P1): 开发者与上游贡献者获得世界顶级规范的自解释代码与 API 文档
作为开源维护者或接入 TTZip 的开发者，我阅读代码与 API 头文件时能如同阅读 `libarchive` 官方文档一样清晰，能瞬间理解每个方法的所有权规则、线程安全性、时间/空间复杂度与异常分支。

- **Acceptance Criteria**:
  - `Sources/TTZipCore/` 与 `Sources/CTTZipBridge/` 核心公共类型与方法 100% 具备工业级 DocC/Doxygen 三段式规范注释。
  - 所有 C 导出函数均明确标注 `[in]`, `[out]`, `[in,out]` 语义与内存释放责任方。

### User Story 2 (P1): 核心归档引擎与编解码管道实现彻底正交解耦
作为系统架构师，我希望格式解析（Format）、压缩过滤（Filter）、数据源（Source/Sink）与安全校验（Validation）高度解耦，各层之间通过标准抽象接口通信，杜绝横向反向耦合。

- **Acceptance Criteria**:
  - 核心引擎消除任何格式间的隐式耦合与循环依赖。
  - 数据流遵循严格的单向 Pipeline 架构。

### User Story 3 (P2): 关键复杂算法与硬件加速分支具备系统不变式原理解释
作为安全与性能审查员，在阅读 SIMD 向量化、APFS 零拷贝与多卷拼接代码时，能清晰看到每一步位运算、内存对齐与系统调用的物理动机。

- **Acceptance Criteria**:
  - SIMD 加解密、页对齐内存借还与多卷穿透逻辑均具备清晰的 Invariant 注释。

---

## 三、 功能需求 (Functional Requirements)

1. **FR001**: 必须制定并落地 `TTZip Code & Documentation Standards Guide`，规范 DocC / Doxygen、参数方向、所有权标记语法。
2. **FR002**: 必须对 `Sources/CTTZipBridge/include/` 下的所有头文件进行全面文档化重构，对标 `archive.h` 规范。
3. **FR003**: 必须对 `Sources/TTZipCore/Platform/`（PAL 模块）与 `Sources/TTZipCore/Services/` 全面补齐工业级文档与架构分层注释。
4. **FR004**: 必须重构核心管道中的跨模块耦合点，确保符合 Template Method / Strategy / Pipeline 设计模式与纯单向依赖。
5. **FR005**: 必须在持续集成（CI）与自动化脚本中增加文档与注释覆盖率检测。

---

## 四、 成功指标 (Success Criteria)

- **SC001**: 核心公共 API 注释覆盖率达 100%，无任何无注释或单行敷衍注释的公共符号。
- **SC002**: 模块依赖关系保持严格的有向无环图 (DAG)，循环依赖数为 0。
- **SC003**: 全量 584+ 单元测试 100% 通过，本地 CI 保持 10s 内满分通关。
