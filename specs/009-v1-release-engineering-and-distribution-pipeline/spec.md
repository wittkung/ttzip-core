# Feature Specification: TTZip v1.0.0 生产级发布工程与全生态自动化分发流水线 (v1.0.0 Release Engineering & Distribution Pipeline)

- **Feature ID**: `009-v1-release-engineering-and-distribution-pipeline`
- **Pipeline Mode**: `[Full SDD]`
- **Status**: `SPECIFIED`
- **Created**: 2026-08-24
- **Target Subsystems & Packages in Scope**:
  - `core/Package.swift` & `core/Sources/` (Core SPM Package Topology Decoupling & Pure SDK Export)
  - `apple/Package.swift` & `apple/Sources/` (Standalone Apple Client Consumer Topology)
  - `apple/scripts/` & `core/scripts/` (Unified Release Pipeline: Universal App Bundle, Retina DMG, Notarization, Sparkle EdDSA Feed)
  - `homebrew/Formula/ttzip.rb` (Homebrew Tap Release Manifest & SHA256 Sync)
  - `core/pyproject.toml`, `core/CMakeLists.txt`, `core/ttzip.pc.in` (Multi-Language Artifact Build & Checksum Verification)
  - License Compliance & LOC Defense Gates (`audit_licenses.py`, `lint_loc_gate.py`)

---

## 1. 业务背景与问题定义 (Problem Statement & Motivation)

TTZip 的底层微内核 (Rust)、C-ABI 2.0 桥接层、10 大语言 SDK、命令行与 TUI 工具以及 macOS 原生客户端代码均已开发完毕。为实现 **v1.0.0 GA (General Availability)** 生产级正式发布，必须建立标准化、确定性、无冲突的发布工程体系：

1. **SPM 目标命名空间冲突阻断客户端独立构建**：`core/Package.swift` 仍残留 `TTZipApp`、`TTZipFinderSync`、`TTZipQuickLook` 目标定义，导致 `apple/Package.swift` 依赖 Core 时产生重名冲突，阻断 `apple/` 编译与测试。
2. **开源合规与许可证头存在残留缺漏**：UniFFI 自动生成的 3 个中间文件缺失 SPDX 许可证头，导致 `audit_licenses.py` 门禁未通过。
3. **客户端与 SDK 分发流水线未打通端到端闭环**：
   - macOS 客户端需要构建 Universal2 架构应用包、注入 Hardened Runtime 签名、生成 Retina 布局 DMG 镜像、提交 Apple Notary Service 公证并装订票据、生成带 EdDSA 签名的 Sparkle 2.0 `appcast.xml`。
   - CLI / TUI 与 SDK 需要打包发布 tarball、生成 `checksums.txt` 清单并对齐 Homebrew Formula。

---

## 2. 用户故事与核心用例 (User Stories & Scenarios)

### User Story 1 (P0): SPM 仓库拓扑解耦与客户端编译测试闭环 (Core/Apple SPM Decoupling)
> **作为** 跨平台开发者与 CI 系统，
> **我希望** `core/Package.swift` 作为独立纯净的 SDK 库分发，`apple/Package.swift` 作为消费者应用直接接入，
> **以便于** 本地与远程构建无任何 Target 重名冲突，并通过全套 XCTest 单元测试。

- **Scenario 1.1 (Core 纯库导出)**: `core/Package.swift` 仅导出 `TTZipCore`、`CTTZipBridge`、`TTZipVendor` 与 `ttzip-bench`，无任何 UI/App targets。
- **Scenario 1.2 (Apple 本地依赖与测试通过)**: `apple/Package.swift` 依赖本地 `../core`（或 `ttzip-core`），执行 `swift test --package-path apple` 跑通全部 17 组 XCTest 用例。

### User Story 2 (P0): 静态合规、LOC 门禁与许可证注入闭环 (License & Compliance Gates)
> **作为** 开源合规与发布质检负责人，
> **我希望** 全域源码均带有标准 SPDX 许可证头部且单文件代码行数 $\le 800$ LOC，
> **以便于** 满足法律合规与宪章架构约束。

- **Scenario 2.1 (SPDX 注入与校验)**: 运行 `inject_spdx_headers.py` 与 `audit_licenses.py`，实现 100% 源码文件合规。
- **Scenario 2.2 (LOC 门禁)**: 校验 `core` 与 `apple` 全部文件行数均在 800 行安全红线以内。

### User Story 3 (P0): macOS 客户端打包、签名、DMG 与 Sparkle 发布流水线 (macOS Production Delivery)
> **作为** macOS 终端用户，
> **我希望** 获得经过 Developer ID 签名、Hardened Runtime 保护、Apple 公证装订且附带自动更新源的 Retina DMG 安装包，
> **以便于** 在 macOS 14+ 上无 Gatekeeper 拦截地一键拖拽安装与更新。

- **Scenario 3.1 (Universal App 封装与签名)**: `bundle_app.sh` 编译并组装 `TTZip.app`，注入签名与 Entitlements。
- **Scenario 3.2 (Retina DMG 制作与公证)**: `create_dmg_installer.sh` 生成高压缩 UDZO DMG 并执行 Apple Notarization 与 Stapling。
- **Scenario 3.3 (Sparkle 2.0 Feed 与 EdDSA 签名)**: `generate_appcast.sh` 生成带有 `sparkle:edSignature`、正确字节数与版本号的 `appcast.xml`。

### User Story 4 (P0): 全生态分发包、Checksums 清单与 Homebrew Formula 闭环 (Distribution & Homebrew)
> **作为** 命令行与开源生态用户，
> **我希望** 通过 Homebrew Tap 或直接下载发布 Tarball 安装 `ttzip`，
> **以便于** 快速在终端使用高吞吐压缩引擎。

- **Scenario 4.1 (CLI Tarball 与 Checksums)**: 打包 `ttzip-cli-v1.0.0-darwin-universal.tar.gz` 并输出 `checksums.txt`。
- **Scenario 4.2 (Homebrew Formula 对齐)**: 更新 `homebrew/Formula/ttzip.rb`，使其正确引用发布 Tarball 与计算所得的 SHA256 校验和。

---

## 3. 验收标准 (Acceptance Criteria)

1. **SPM 构建无冲突**: `swift test --package-path core` (166 用例) 与 `swift test --package-path apple` (17 套件) 均 100% 成功通过。
2. **合规审计 100% 通过**: `python3 core/scripts/audit_licenses.py` 退出码为 0，零违规文件。
3. **LOC 防御门禁 100% 通过**: `core` 与 `apple` 下所有源码文件均 $\le 800$ 行。
4. **客户端发布产物就绪**: 成功生成 `dist/TTZip.app`、`dist/TTZip-1.0.0.dmg` 与 `apple/appcast.xml`。
5. **分发清单与 Checksum 闭环**: `checksums.txt` 记录 CLI Tarball 与 DMG 的 SHA256，`homebrew/Formula/ttzip.rb` 语法与测试断言验证通过。
