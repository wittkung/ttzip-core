# Requirements Checklist: Feature 013 (100% Grand Slam & Zero Regression)

## Quality & Coverage Gates
- [ ] 针对纯 TAR 启用 APFS `fcopyfile` 零拷贝直通打包与直接解析。
- [ ] 针对 TAR.ZST 升级 32MB 极速流式解压缓冲区。
- [ ] 针对 LZ4 / LZIP 调优高熵与多核压缩分块。
- [ ] 运行全格式 142 项对决验证 100% 满贯。
- [ ] 运行 `audit_performance_regression.py` 验证零倒退。
- [ ] 11 大性能门禁与 560+ 单测 100% 绿灯。
