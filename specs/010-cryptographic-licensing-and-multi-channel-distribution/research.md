# Research: 密码学离线授权与 Steam/MAS/Direct 四轨分发技术决议

## 1. CryptoKit Ed25519 签名与验签选型

- **Decision**: 采用 Apple `CryptoKit.Curve25519.Signing.PublicKey`。
- **Rationale**:
  1. macOS 14+ / iOS 17+ 原生支持，零额外 C/Rust 动态库链接。
  2. 单次验签耗时约 $15\mu s$，内存开销 $< 1\text{KB}$。
  3. 公钥仅 32 字节，易于以 Base64 或 Hex 常量硬编码在客户端。
- **Alternatives Considered**:
  1. *RSA 2048 / 4096*: 公钥大（256~512 字节），Key 字符串过长，移动/桌面体验差。
  2. *ECDSA P-256*: 同样支持，但 Ed25519 签名性能与抗侧信道攻击能力更优。

---

## 2. 授权码格式与防篡改编码

- **Decision**: `TTZIP1-<Base64URL(Payload)>.<Base64URL(Signature)>`。
- **Rationale**:
  1. `TTZIP1-` 前缀指明版本，便于未来升级协议。
  2. Payload 与 Signature 由点号 `.` 分隔，结构清晰，可直接用 `components(separatedBy: ".")` 解析。
  3. 纯 Base64URL 字符集，用户在邮件、网页或微信复制时不会发生自动换行或特殊字符丢失。
- **Alternatives Considered**:
  1. *二进制 License 独立文件 (`.ttziplicense`)*: 用户需要拖拽导入文件，操作链路较长。支持单行 Key 粘贴是体验最优解。

---

## 3. Steam 商店发布与运行模式

- **Decision**: DRM-Free + `-DSTEAM_BUILD` 宏定义 + 标准 `TTZip.app` Steam Depot。
- **Rationale**:
  1. Valve 官方政策允许且推崇 DRM-Free 工具分发。
  2. 禁用 Sparkle 自动更新，交由 Steam 客户端统一执行高效增量补丁。
  3. 无沙盒限制，支持完整的 APFS 写时复制（Clonefile）与系统级极速解压。
- **Alternatives Considered**:
  1. *集成 Steamworks C API (`steam_api.dylib`)*: 增加动态库依赖和初始化失败风险。作为压缩工具，无强制绑定 Steam 客户端运行的必要性。

---

## 4. 四轨编译渠道与宏定义

- **Decision**:
  - `direct`: 默认 Release 编译，集成 Sparkle 2.0，支持 Ed25519 验签。
  - `mas`: `-DMAS_BUILD`，开启 App Sandbox，移除 Sparkle，`isPro = true`。
  - `steam`: `-DSTEAM_BUILD`，无沙盒，移除 Sparkle，`isPro = true`。
  - `community`: `-DCOMMUNITY_BUILD`，全功能开放，标记社区版，支持自愿输入 Key 激活。
