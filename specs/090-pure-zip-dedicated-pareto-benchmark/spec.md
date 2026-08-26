# Feature Specification: Pure ZIP Format Dedicated Pareto Benchmark

## 1. 业务目标与聚焦范围 (Goal & Focus Demarcation)

本特性 **100% 专项聚焦于 ZIP 格式（Universal ZIP Pareto Benchmark）**：
1. **纯粹性（Zero Non-ZIP Contamination）**：彻底剔除 7Z、TAR.ZST、LZ4 等非 ZIP 格式，只对比同在 `.zip` 容器标准下的各软件真实表现。
2. **高密度多档位深度采样**：
   - 👑 **TTZip**：Level 1 (极速), Level 3 (快速), Level 6 (标准), Level 9 (最大), Level 12 (Ultra)
   - 🔷 **7-Zip 26.02 官方 ARM64 (`7zz -tzip -mmt=on`)**：`-mx=1`, `-mx=3`, `-mx=5`, `-mx=7`, `-mx=9`
   - 🍎 **Apple Native 工具链**：`ditto -c -k`, `zip -1`, `zip -3`, `zip -6`, `zip -9`
3. **针对 ZIP 压缩率区间的专属视口与美学优化**：
   - X 轴自适应缩放（聚焦于 94.5% ~ 97.2% 的高精度区间，步长 0.5%），横向呼吸空间充裕；
   - Y 轴（5 MB/s ~ 2,000 MB/s 对数刻度）；
   - 3 大软件家族（TTZip 蓝、7-Zip 橙、Apple 红）完整呈现三条独立的三次 Hermite 样条曲线；
   - Hero 蓝色药丸卡片与 8 槽位 AABB 碰撞避让。

---

## 2. 交付工件定义

- `pareto_pk_zip.png`：纯 ZIP 格式 2x Retina 高保真图表（直接内嵌展示给用户）。
- `pareto_pk_zip.svg`：纯矢量响应式图表。
- `pareto_zip_report.md`：纯 ZIP 格式专项评测报告。
