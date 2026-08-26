# Feature Specification: TTZip 密码学离线授权、Steam 商店上架与四轨分发体系

- **Feature ID**: `010-cryptographic-licensing-and-multi-channel-distribution`
- **Classification**: `[Full SDD]` (涉及系统授权边界变更、离线密码学校验、Steam/MAS/Direct/Community 多渠道编译宏与分发流水线)
- **Status**: `SPECIFIED`
- **Authors**: TTZip Architecture & Release Team

---

## 1. 业务背景与问题定义 (Executive Summary & Problem Statement)

### 1.1 现状与技术痛点
1. **硬编码后门与伪门禁**：现有 `SystemServices.swift` 中存在 `AURA-PRO1-KEY8-2026` 首次启动硬编码激活漏洞，且 `validateKeyFormat` 仅检查字符串格式，无密码学签名保护。
2. **功能虚假拦截违背开源理念**：当前代码中虽声明 `ProFeature`，但未在全流程真正生效，且与“源码 100% 自由自编译、开源免费全功能”的定位产生冲突。
3. **分发渠道孤岛与宏定义缺失**：应用缺乏 Mac App Store (MAS)、Steam 商店、官网 Direct DMG 与 GitHub 开源构建的统一编译矩阵与渠道隔离机制。

### 1.2 目标与商业价值
1. **实现纯离线 Ed25519 密码学验签**：基于 Apple 原生 `CryptoKit`，使用内置 32 字节公钥完成 $< 20\mu s$ 的离线毫秒级验签，零外部网络与服务器依赖。
2. **落地“开源便利性变现模型 (Convenience Monetization)”**：
   - GitHub 自编译源码：100% 全功能可用，无功能阉割，显示 `Community Edition`。
   - Mac App Store (MAS)：¥29 一次性买断下载，严格合规 App Sandbox，自动静默更新。
   - Steam 商店：¥29 / $4.99 一次性买断入库，DRM-Free，覆盖全球 Mod 玩家与极客创作者。
   - 官网 Direct DMG：免费全功能试用 + ¥29 终身授权 Key（解锁 Pro 徽章与官方技术支持）。
3. **四轨打包流水线自动化**：在 `apple/scripts/bundle_app.sh` 中提供 `--channel [direct|mas|steam|community]` 统一打包支持。

---

## 2. 用户故事与验收标准 (User Stories & Acceptance Criteria)

### US1: 纯离线 Ed25519 密码学授权验签
- **As a** 官网直装版付费用户
- **I want to** 在设置中输入我的买断 License Key (`TTZIP1-<Payload>.<Signature>`)
- **So that** 应用能在离线状态下瞬间验证我的终身授权，点亮 Pro 徽章并展示注册邮箱与订单号。
- **Acceptance Criteria**:
  1. 正确签名的 Key 验签通过，写入 `UserDefaults`，设置页实时展示授权状态。
  2. 篡改 Payload、过期或错误签名的 Key 明确提示无效，且不会发生崩溃。
  3. 客户端只内置公钥，绝不泄露私钥。

### US2: 彻底清除硬编码后门与虚假门禁
- **As a** 开源社区开发者与审计人员
- **I want to** 确认代码中不存在任何硬编码激活码或偷梁换柱的伪门禁
- **So that** 源码可以放心地在 GitHub 开源自编译使用。
- **Acceptance Criteria**:
  1. 删除 `activate(key: "AURA-...")` 与 `validateKeyFormat`。
  2. 自编译版本在未输入 Key 时，正常提供 100% 格式压缩、解压、分卷、密码保护与检查功能，仅在状态栏/设置页提示 `Community Edition`。

### US3: Mac App Store 与 Steam 商店免激活合规
- **As a** MAS 或 Steam 付费用户
- **I want to** 下载安装后即自动拥有 Pro 授权，无需重复输入激活码
- **So that** 获得平台级原生无感知的购买体验。
- **Acceptance Criteria**:
  1. `-DMAS_BUILD` 构建产物沙盒合规，禁用 Sparkle，自动判定为 `Pro Lifetime (MAS)`。
  2. `-DSTEAM_BUILD` 构建产物禁用 Sparkle，自动判定为 `Pro Lifetime (Steam)`。

### US4: 多渠道参数化打包脚本
- **As a** 发布工程师
- **I want to** 通过 `./apple/scripts/bundle_app.sh --channel <channel>` 一键产出对应渠道的构建包
- **So that** 杜绝手动修改代码或宏定义导致的发版事故。
- **Acceptance Criteria**:
  1. `--channel direct`：生成 Hardened Runtime + Sparkle 2.0 的 App。
  2. `--channel mas`：生成 App Sandbox + 纯 App Store 架构的 App。
  3. `--channel steam`：生成适配 Steam Depot 启动规范的 App。
  4. `--channel community`：生成无签名的通用自编译 App。

---

## 3. 边界条件与非功能性要求 (Non-Functional Requirements)

1. **零网络调用**：验签过程绝不发起 HTTP 请求，绝不收集用户硬件指纹。
2. **执行性能**：验签在后台线程与 UI 主线程均可在 $< 1\text{ms}$ 内完成，无主线程掉帧。
3. **代码行数红线**：所有新建和重构文件严格遵守 $\le 800$ LOC。
