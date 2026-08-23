# Implementation Plan: TTZip 对标全球顶级专业归档软件全维度差距补齐与工程落地

**Branch**: `082-pro-software-gap-audit` | **Date**: 2026-08-18 | **Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/082-pro-software-gap-audit/spec.md)

---

## 1. Summary

本规划基于全球 5 大顶级归档软件（BetterZip 5, Keka 1.4, WinRAR 7, 7-Zip 24, Bandizip 7）的横向审计与 5 个专项子 Agent 的深度研究结论，闭环落地 5 大专业级核心能力：
1. **智能解压与自动化流控**：基于两阶段有效根求解（有效顶层项过滤）与操作后自动化（自动移入废纸篓、Finder 高亮）；
2. **多格式自适应分卷归档**：零拷贝流式跨卷切片管道 (`MultiVolumeStreamSink`)，原生生成标准 7Z (`.7z.001`) 与 ZIP (`.z01`/`.zip.001`)；
3. **7Z 头部文件名加密与生物认证**：原生 C 引擎 `kEncodedHeader` (ID 0x17) NEON KDF 派生 + `LocalAuthentication` 结合 Keychain 硬件访问控制；
4. **外部编辑器就地编辑协同**：UUID 暂存沙盒 + Dual-Tier（父目录+文件）`DispatchSource` 监听 + 防抖哈希快照 + APFS 原子替换；
5. **Reed-Solomon 恢复记录与自愈**：$GF(2^{16})$ Cauchy Reed-Solomon 擦除纠错码 + 统一尾部扩展架构 (Dual-EOCD & Post-EOF Trailer)。

---

## 2. Technical Context

- **Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`, Strict Concurrency & Actor Isolation) + C11 / POSIX APIs.
- **Primary Dependencies**: In-process static C libraries (`libarchive`, `libdeflate`, `fast-lzma2`, `zstd`, `lz4`, `lzfse`, `libb2`), macOS `LocalAuthentication.framework`, `Security.framework` (Keychain), `AppKit.framework`.
- **Target Platform**: macOS 14.0+ (Sonoma, Sequoia; Apple Silicon ARM64 NEON prioritized, Intel x86_64 compatible).
- **Distribution Channels**: Mac App Store (MAS Sandbox `-DMAS_BUILD`) + Direct Independent Distribution (Sparkle 2.0).
- **Performance Invariants**:
  - Hot paths zero intermediate heap allocation (`UnsafeMutablePointer`, no `Data(count:)` kernel zeroing).
  - 7Z AES-256 KDF hardware derivation $\le 15\text{ms}$ (NEON SIMD).
  - Multi-volume stream slicing 0% secondary disk I/O overhead.
  - Recovery record generation throughput $\ge 4.5\text{ GB/s}$ (NEON `PMULL`).

### Phase 0 Research Index
- - R001 [SUBAGENT:research] 《macOS 智能解压启发式算法与 Apple 元数据过滤规范》：两阶段有效根求解与元数据黑名单预清洗。
- - R002 [SUBAGENT:research] 《7Z 与 ZIP 标准多卷分卷切片与流式跨卷写入管道》：零拷贝流式跨卷写入管道与 7Z 首卷 32 字节延迟回写。
- - R003 [SUBAGENT:research] 《7Z 头部文件名加密与 macOS LocalAuthentication / Touch ID 生物识别安全》：原生 C 引擎 `kEncodedHeader` 与 Keychain `SecAccessControl` 硬件双向绑定。
- - R004 [SUBAGENT:research] 《外部编辑器沙盒临时提取与 FSEvents/DispatchSource 双向热回写架构》：UUID 独立沙盒与 Dual-Tier 双层 FD 监听。
- - R005 [SUBAGENT:research] 《Reed-Solomon (RS-FEC) 前向纠错恢复记录与灾难自愈数学引擎》：$GF(2^{16})$ Cauchy Reed-Solomon 擦除纠错码与统一尾部扩展。

---

## 3. Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 宪法门禁原则 | 检查结论 | 说明与合规依据 |
| :--- | :--- | :--- |
| **1. 核心架构与 100% 进程内 C 绑定** | 🟢 PASS | 所有编解码与密码学逻辑均在 `CTTZipBridge` 进程内完成，零外部 CLI 进程调用。 |
| **2. 热路径零成本抽象与零内核页清零** | 🟢 PASS | 分卷切片与恢复记录写入均采用裸指针流式拉取，无动态对象树或 `Data(count:)` 清零中断。 |
| **3. 格式 Fast-Path 与硬件特化旁路** | 🟢 PASS | 7Z AES KDF 与 RS-FEC 纠错码优先调用 ARM64 NEON `PMULL` 与 SIMD 指令。 |
| **4. 四大系统工程铁律 (Stream/Invariant/Bounds/Oracle)** | 🟢 PASS | 流式切片零中间大文件；POSIX 原语级路径校验；Magic 结构体防 UAF；黄金语料库双向差分。 |
| **5. 敏感内存防死代码消除 (Dead-Store Immunity)** | 🟢 PASS | 密码、派生密钥与解密上下文释放前使用 `memset_s` / `explicit_bzero` 物理擦除。 |
| **6. MAS 沙盒合规性与条件编译** | 🟢 PASS | `LocalAuthentication` 与 `Security` 均为公有 API；Sparkle 严格包裹在 `#if !MAS_BUILD` 中。 |

---

## 4. Phase 1 Design Artifacts & Contracts Index

### Contracts List (System Boundary Interfaces)
- `contracts/smart-extract-contract.json` [SUBAGENT:research] — 智能解压策略决策、冲突处理与路径解析接口契约。
- `contracts/split-volume-contract.json` [SUBAGENT:research] — 多格式自适应分卷切片创建、配置与连续卷合并接口契约。
- `contracts/external-edit-contract.json` [SUBAGENT:research] — 外部编辑器提取、双向 FSEvents 侦听与原子热回写接口契约。
- `contracts/recovery-record-contract.json` [SUBAGENT:research] — Reed-Solomon RS-FEC 恢复记录生成、坏块定位与灾难自愈接口契约。
- `contracts/biometric-auth-contract.json` [SUBAGENT:research] — Touch ID / Apple Watch 生物认证与 Keychain 硬件密钥解密接口契约。

### Data Models & Quickstart Index
- `specs/082-pro-software-gap-audit/data-model.md` — 包含 6 大核心领域实体、字段约束、状态机生命周期与验证边界。
- `specs/082-pro-software-gap-audit/quickstart.md` — 包含 5 大验收场景（智能解压、分卷创建、头部加密、就地编辑、恢复记录）的可执行验证指南。

---

## 5. Component Modification & Code Mapping

```text
TTZip/
├── Sources/
│   ├── CTTZipBridge/
│   │   ├── CTTZipBridge_7zNativeDecoder.c    # [MODIFY] 7Z kEncodedHeader 状态机与 NEON KDF
│   │   ├── ttzip_7z_header_writer.c         # [MODIFY] 7Z StartHeader 32 字节延迟回写
│   │   └── ttzip_rs_fec.c                   # [NEW] ARM NEON PMULL GF(2^16) Cauchy RS 编解码
│   ├── TTZipCore/
│   │   ├── Split/
│   │   │   ├── MultiVolumeStreamSink.swift   # [MODIFY] 跨卷零拷贝流式切片管道
│   │   │   └── SplitVolumeConfig.swift       # [MODIFY] 分卷预设标准对齐
│   │   ├── InPlaceEdit/
│   │   │   ├── InPlaceArchiveMutationEngine.swift # [MODIFY] Dual-Tier 监听与 APFS 原子替换
│   │   │   └── InPlaceEditSession.swift      # [MODIFY] 会话生命周期与防抖哈希比对
│   │   ├── Security/
│   │   │   ├── TouchIDAuthenticator.swift    # [MODIFY] LAContext + LAPolicy.deviceOwnerAuthentication
│   │   │   └── PathPatternFilterEngine.swift # [MODIFY] isSystemMetadata 智能解压预清洗
│   │   └── ArchiveRepairEngine.swift        # [MODIFY] RS-FEC 坏块定位与自愈重建
│   └── TTZipApp/
│       ├── ViewModels/
│       │   └── AppViewState.swift           # [MODIFY] 智能解压分发与外部编辑桥接
│       └── Views/
│           ├── ArchiveExplorerView.swift    # [MODIFY] 外部编辑双击交互与保存提示
│           ├── CompressModalView.swift      # [MODIFY] 分卷预设选择与恢复记录勾选
│           └── Benchmark/
│               └── BenchmarkView.swift      # [MODIFY] GUI 原生 MIPS 算力仪表盘
```
