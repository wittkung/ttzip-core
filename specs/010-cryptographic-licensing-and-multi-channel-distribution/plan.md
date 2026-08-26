# Implementation Plan: TTZip 密码学离线授权、Steam 商店上架与四轨分发体系 (Feature 010)

- **Feature ID**: `010-cryptographic-licensing-and-multi-channel-distribution`
- **Classification**: `[Full SDD]`
- **Status**: `PLANNED`

---

## 1. 技术上下文与架构分析 (Technical Context)

### 1.1 依赖层级与交互边界
- **`TTZipCore` (Swift 6)**:
  - 移除历史伪授权逻辑与硬编码激活码。
  - 新增 `Ed25519LicenseVerifier` 与 `Ed25519LicenseManager`，基于 Apple `CryptoKit` 实现纯离线验签。
  - 维护 `ChannelLicenseState`（Community, DirectPro, MASPro, SteamPro）。
- **`TTZipApp` (SwiftUI / AppKit)**:
  - 升级 `SettingsView.swift` 授权标签页：展示质感状态徽章（如 Steam Pro / MAS Pro / Direct Pro / Community），提供 Key 输入与注销功能。
  - 确保全功能无障碍使用，杜绝负向弹窗与恶性阻断。
- **发布脚本与工具链 (`apple/scripts/` & `core/scripts/`)**:
  - `generate_license.py`: 开发者发码与公私钥管理脚本。
  - `bundle_app.sh --channel <channel>`: 参数化多渠道打包构建。

---

## 2. 实施分阶段执行计划 (Phased Implementation Plan)

### Phase 1: 核心授权引擎与 Ed25519 密码学校验器
- **T001**: 编写 `core/scripts/generate_license.py`，生成 Ed25519 开发与测试公私钥对，输出测试用例授权码。
- **T002**: 在 `core/Sources/TTZipCore/` 中重构 `LicenseManager`，移除 `AURA-PRO1-KEY8-2026` 与 `validateKeyFormat`，实现 `Ed25519LicenseVerifier`。
- **T003**: 编写 `TTZipCore` 密码学校验单元测试（测试合法 Key、篡改 Payload、错误签名、格式畸变等边界情况）。

### Phase 2: 四轨渠道状态机与 UI 呈现
- **T004**: 实现 `ChannelLicenseState`（识别 `-DMAS_BUILD`, `-DSTEAM_BUILD`, `-DCOMMUNITY_BUILD` 及 Direct 授权）。
- **T005**: 升级 `apple/Sources/TTZipApp/Views/SettingsView+Tabs.swift` 中的授权页，展示典雅的四轨渠道徽章与离线授权信息。

### Phase 3: 多渠道参数化打包与 Steam/MAS 适配
- **T006**: 更新 `apple/scripts/bundle_app.sh` 增加 `--channel [direct|mas|steam|community]` 选项。
- **T007**: 验证各渠道构建包产物（Direct 带 Sparkle，MAS 带 Sandbox，Steam 免 Sparkle，Community 社区版）。

### Phase 4: 开源治理、文档与全量回归验收
- **T008**: 补充 `apple/CONTRIBUTING.md` 与 `apple/SECURITY.md`。
- **T009**: 运行双端全量测试套件 (`swift test --package-path core` 与 `swift test --package-path apple`)。
- **T010**: 运行 LOC 架构门禁与许可证合规审计。
