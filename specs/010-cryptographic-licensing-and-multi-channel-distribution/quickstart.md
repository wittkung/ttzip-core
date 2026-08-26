# Quickstart: 密码学离线授权与四轨渠道验证指南

## 1. 生成 Ed25519 签名授权码
```bash
# 生成私钥公钥对或生成测试授权码
python3 core/scripts/generate_license.py \
    --email "customer@example.com" \
    --order "ORD-20260825-9988" \
    --tier "pro_lifetime"
```

---

## 2. 运行密码学单元测试
```bash
# 验证 Ed25519 验签算法与容错用例
swift test --package-path core --filter Ed25519LicenseVerifierTests
```

---

## 3. 多渠道打包编译验证
```bash
# 1. 官网直装版 (Direct DMG + Sparkle)
./apple/scripts/bundle_app.sh --channel direct

# 2. Mac App Store 版 (MAS 沙盒 + 禁用 Sparkle)
./apple/scripts/bundle_app.sh --channel mas

# 3. Steam 商店版 (Steam 免激活 + 禁用 Sparkle)
./apple/scripts/bundle_app.sh --channel steam

# 4. 开源自编译社区版 (Community)
./apple/scripts/bundle_app.sh --channel community
```
