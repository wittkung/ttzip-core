# Quickstart: Dedicated Per-Format Benchmark Charts

## 验证场景：全量单格式专属 PK 图表生成与多软件对比

### 1. Command (可执行命令)
```bash
SPECIFY_FEATURE_DIRECTORY="specs/089-dedicated-per-format-benchmark-charts" swift test --filter SoftwareParetoFrontierPkTests
```

### 2. Expected Output (成功输出断言)
```text
Test Suite 'SoftwareParetoFrontierPkTests' passed (8.5s)
📂 已生成 4 张专属格式图表与 1 张综合全景图:
------------------------------------------------------------------------
1. pareto_pk_zip.png      (ZIP 专场: TTZip vs. 7-Zip vs. Apple ditto / zip -1 / zip -6)
2. pareto_pk_7z.png       (7Z 专场: TTZip vs. 7-Zip 26.02 Fast, Normal, Ultra)
3. pareto_pk_tar_zst.png  (TAR.ZST 专场: TTZip Direct L1, L3)
4. pareto_pk_lz4.png      (LZ4 专场: TTZip Direct In-Memory)
5. software_pareto_pk.png (4-Tier 全景图)
```

### 3. Failure Diagnostic (失败排查路径)
- 若 `/usr/bin/zip` 或 `/usr/bin/ditto` 失败，检查系统权限或临时目录可用磁盘空间。
