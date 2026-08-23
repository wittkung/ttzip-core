# TTZip 性能比对与零倒退审计报告

- **审计时间**: 2026-08-24 06:39:28
- **基准版本 (Before)**: `docs/benchmarks/benchmark_report_2026-08-16_014050.json`
- **最新版本 (After)**: `docs/benchmarks/benchmark_report_2026-08-19_054234.json`

## 一、 统计摘要

- 🟢 **提升项数 (> +3.0%)**: 0
- ⚪ **持平项数 (±3.0% 以内)**: 0
- 🟡 **轻微倒退告警 (-3.0% ~ -10.0%)**: 0
- 🔴 **严重倒退阻断 (< -10.0%)**: 0

## 四、 全量维度性能逐项对比表

| 格式 | 场景/维度 | 级别 | 加密 | 压缩前 (MB/s) | 压缩后 (MB/s) | 压缩增益 | 解压前 (MB/s) | 解压后 (MB/s) | 解压增益 |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| 7z | 500MB Large Dataset (500MB) | L1 | 无 | 0.0 | 5163.5 | +  0.0% | 0.0 | 8964.6 | +  0.0% |
| 7z | 500MB Large Dataset (500MB) | L1 | AES | 0.0 | 4816.2 | +  0.0% | 0.0 | 8117.8 | +  0.0% |
| 7z | 500MB Large Dataset (500MB) | L6 | 无 | 0.0 | 4764.6 | +  0.0% | 0.0 | 9551.7 | +  0.0% |
| 7z | 500MB Large Dataset (500MB) | L6 | AES | 0.0 | 4866.8 | +  0.0% | 0.0 | 8334.4 | +  0.0% |
| 7z | Float32 Sensor Matrix (50MB) | L1 | 无 | 0.0 | 351.5 | +  0.0% | 0.0 | 758.1 | +  0.0% |
| 7z | Float32 Sensor Matrix (50MB) | L1 | AES | 0.0 | 329.2 | +  0.0% | 0.0 | 709.3 | +  0.0% |
| 7z | Float32 Sensor Matrix (50MB) | L6 | 无 | 0.0 | 56.4 | +  0.0% | 0.0 | 659.0 | +  0.0% |
| 7z | Float32 Sensor Matrix (50MB) | L6 | AES | 0.0 | 55.6 | +  0.0% | 0.0 | 605.1 | +  0.0% |
| 7z | High-Entropy Payload (100MB) | L1 | 无 | 0.0 | 4237.8 | +  0.0% | 0.0 | 6560.6 | +  0.0% |
| 7z | High-Entropy Payload (100MB) | L1 | AES | 0.0 | 1324.7 | +  0.0% | 0.0 | 5662.1 | +  0.0% |
| 7z | High-Entropy Payload (100MB) | L6 | 无 | 0.0 | 5606.0 | +  0.0% | 0.0 | 7167.3 | +  0.0% |
| 7z | High-Entropy Payload (100MB) | L6 | AES | 0.0 | 1311.5 | +  0.0% | 0.0 | 5915.5 | +  0.0% |
| 7z | Log Text (10MB) | L1 | 无 | 0.0 | 3348.7 | +  0.0% | 0.0 | 8133.7 | +  0.0% |
| 7z | Log Text (10MB) | L1 | AES | 0.0 | 1393.5 | +  0.0% | 0.0 | 1257.0 | +  0.0% |
| 7z | Log Text (10MB) | L6 | 无 | 0.0 | 587.0 | +  0.0% | 0.0 | 6807.6 | +  0.0% |
| 7z | Log Text (10MB) | L6 | AES | 0.0 | 535.5 | +  0.0% | 0.0 | 1191.1 | +  0.0% |
| 7z | Small Files (10MB/100 files) | L1 | 无 | 0.0 | 2866.0 | +  0.0% | 0.0 | 1682.8 | +  0.0% |
| 7z | Small Files (10MB/100 files) | L1 | AES | 0.0 | 1738.4 | +  0.0% | 0.0 | 907.1 | +  0.0% |
| 7z | Small Files (10MB/100 files) | L6 | 无 | 0.0 | 687.4 | +  0.0% | 0.0 | 1514.1 | +  0.0% |
| 7z | Small Files (10MB/100 files) | L6 | AES | 0.0 | 695.4 | +  0.0% | 0.0 | 833.7 | +  0.0% |
| 7z | Structured JSON (50MB) | L1 | 无 | 0.0 | 3953.2 | +  0.0% | 0.0 | 9186.9 | +  0.0% |
| 7z | Structured JSON (50MB) | L1 | AES | 0.0 | 4047.8 | +  0.0% | 0.0 | 3708.8 | +  0.0% |
| 7z | Structured JSON (50MB) | L6 | 无 | 0.0 | 1330.2 | +  0.0% | 0.0 | 9415.9 | +  0.0% |
| 7z | Structured JSON (50MB) | L6 | AES | 0.0 | 1383.9 | +  0.0% | 0.0 | 3784.5 | +  0.0% |
| aar | 500MB Large Dataset (500MB) | L1 | 无 | 0.0 | 3181.3 | +  0.0% | 0.0 | 6760.4 | +  0.0% |
| aar | 500MB Large Dataset (500MB) | L6 | 无 | 0.0 | 3057.8 | +  0.0% | 0.0 | 6507.1 | +  0.0% |
| aar | Float32 Sensor Matrix (50MB) | L1 | 无 | 0.0 | 2097.4 | +  0.0% | 0.0 | 2748.6 | +  0.0% |
| aar | Float32 Sensor Matrix (50MB) | L6 | 无 | 0.0 | 2092.7 | +  0.0% | 0.0 | 2824.3 | +  0.0% |
| aar | High-Entropy Payload (100MB) | L1 | 无 | 0.0 | 2313.6 | +  0.0% | 0.0 | 3120.5 | +  0.0% |
| aar | High-Entropy Payload (100MB) | L6 | 无 | 0.0 | 2333.1 | +  0.0% | 0.0 | 3087.2 | +  0.0% |
| aar | Log Text (10MB) | L1 | 无 | 0.0 | 1625.6 | +  0.0% | 0.0 | 2591.3 | +  0.0% |
| aar | Log Text (10MB) | L6 | 无 | 0.0 | 1592.7 | +  0.0% | 0.0 | 2572.8 | +  0.0% |
| aar | Small Files (10MB/100 files) | L1 | 无 | 0.0 | 1899.0 | +  0.0% | 0.0 | 2175.6 | +  0.0% |
| aar | Small Files (10MB/100 files) | L6 | 无 | 0.0 | 1905.9 | +  0.0% | 0.0 | 2176.2 | +  0.0% |
| aar | Structured JSON (50MB) | L1 | 无 | 0.0 | 3709.2 | +  0.0% | 0.0 | 2963.0 | +  0.0% |
| aar | Structured JSON (50MB) | L6 | 无 | 0.0 | 3816.6 | +  0.0% | 0.0 | 3086.9 | +  0.0% |
| brotli | 500MB Large Dataset (500MB) | L1 | 无 | 0.0 | 1115.4 | +  0.0% | 0.0 | 1524.6 | +  0.0% |
| brotli | 500MB Large Dataset (500MB) | L6 | 无 | 0.0 | 1052.1 | +  0.0% | 0.0 | 1489.9 | +  0.0% |
| brotli | Float32 Sensor Matrix (50MB) | L1 | 无 | 0.0 | 322.5 | +  0.0% | 0.0 | 244.8 | +  0.0% |
| brotli | Float32 Sensor Matrix (50MB) | L6 | 无 | 0.0 | 332.2 | +  0.0% | 0.0 | 250.8 | +  0.0% |
| brotli | High-Entropy Payload (100MB) | L1 | 无 | 0.0 | 1605.3 | +  0.0% | 0.0 | 3662.6 | +  0.0% |
| brotli | High-Entropy Payload (100MB) | L6 | 无 | 0.0 | 1628.4 | +  0.0% | 0.0 | 3821.7 | +  0.0% |
| brotli | Log Text (10MB) | L1 | 无 | 0.0 | 1157.2 | +  0.0% | 0.0 | 1582.7 | +  0.0% |
| brotli | Log Text (10MB) | L6 | 无 | 0.0 | 1149.6 | +  0.0% | 0.0 | 1538.0 | +  0.0% |
| brotli | Small Files (10MB/100 files) | L1 | 无 | 0.0 | 936.0 | +  0.0% | 0.0 | 1169.9 | +  0.0% |
| brotli | Small Files (10MB/100 files) | L6 | 无 | 0.0 | 930.3 | +  0.0% | 0.0 | 1205.8 | +  0.0% |
| brotli | Structured JSON (50MB) | L1 | 无 | 0.0 | 1188.6 | +  0.0% | 0.0 | 1679.9 | +  0.0% |
| brotli | Structured JSON (50MB) | L6 | 无 | 0.0 | 1183.9 | +  0.0% | 0.0 | 1723.9 | +  0.0% |
| dmg | 500MB Large Dataset (500MB) | L1 | 无 | 0.0 | 3139.7 | +  0.0% | 0.0 | 5257.4 | +  0.0% |
| dmg | 500MB Large Dataset (500MB) | L1 | AES | 0.0 | 3192.4 | +  0.0% | 0.0 | 5032.9 | +  0.0% |
| dmg | 500MB Large Dataset (500MB) | L6 | 无 | 0.0 | 3663.8 | +  0.0% | 0.0 | 7147.5 | +  0.0% |
| dmg | 500MB Large Dataset (500MB) | L6 | AES | 0.0 | 3795.8 | +  0.0% | 0.0 | 6560.2 | +  0.0% |
| dmg | Float32 Sensor Matrix (50MB) | L1 | 无 | 0.0 | 3136.0 | +  0.0% | 0.0 | 5550.1 | +  0.0% |
| dmg | Float32 Sensor Matrix (50MB) | L1 | AES | 0.0 | 2210.2 | +  0.0% | 0.0 | 4771.9 | +  0.0% |
| dmg | Float32 Sensor Matrix (50MB) | L6 | 无 | 0.0 | 3118.1 | +  0.0% | 0.0 | 4780.9 | +  0.0% |
| dmg | Float32 Sensor Matrix (50MB) | L6 | AES | 0.0 | 1977.0 | +  0.0% | 0.0 | 5424.5 | +  0.0% |
| dmg | High-Entropy Payload (100MB) | L1 | 无 | 0.0 | 3342.8 | +  0.0% | 0.0 | 6410.7 | +  0.0% |
| dmg | High-Entropy Payload (100MB) | L1 | AES | 0.0 | 3634.0 | +  0.0% | 0.0 | 6470.0 | +  0.0% |
| dmg | High-Entropy Payload (100MB) | L6 | 无 | 0.0 | 3483.2 | +  0.0% | 0.0 | 6441.3 | +  0.0% |
| dmg | High-Entropy Payload (100MB) | L6 | AES | 0.0 | 3497.6 | +  0.0% | 0.0 | 6568.3 | +  0.0% |
| dmg | Log Text (10MB) | L1 | 无 | 0.0 | 2849.2 | +  0.0% | 0.0 | 5646.0 | +  0.0% |
| dmg | Log Text (10MB) | L1 | AES | 0.0 | 2608.1 | +  0.0% | 0.0 | 4973.2 | +  0.0% |
| dmg | Log Text (10MB) | L6 | 无 | 0.0 | 2658.8 | +  0.0% | 0.0 | 5123.2 | +  0.0% |
| dmg | Log Text (10MB) | L6 | AES | 0.0 | 2850.6 | +  0.0% | 0.0 | 5332.2 | +  0.0% |
| dmg | Small Files (10MB/100 files) | L1 | 无 | 0.0 | 1689.5 | +  0.0% | 0.0 | 1099.7 | +  0.0% |
| dmg | Small Files (10MB/100 files) | L1 | AES | 0.0 | 1679.6 | +  0.0% | 0.0 | 1098.3 | +  0.0% |
| dmg | Small Files (10MB/100 files) | L6 | 无 | 0.0 | 1703.9 | +  0.0% | 0.0 | 1087.6 | +  0.0% |
| dmg | Small Files (10MB/100 files) | L6 | AES | 0.0 | 1737.8 | +  0.0% | 0.0 | 1076.4 | +  0.0% |
| dmg | Structured JSON (50MB) | L1 | 无 | 0.0 | 3482.3 | +  0.0% | 0.0 | 6573.2 | +  0.0% |
| dmg | Structured JSON (50MB) | L1 | AES | 0.0 | 2962.5 | +  0.0% | 0.0 | 5530.1 | +  0.0% |
| dmg | Structured JSON (50MB) | L6 | 无 | 0.0 | 3145.6 | +  0.0% | 0.0 | 6200.2 | +  0.0% |
| dmg | Structured JSON (50MB) | L6 | AES | 0.0 | 2997.2 | +  0.0% | 0.0 | 5587.4 | +  0.0% |
| lrzip | 500MB Large Dataset (500MB) | L1 | 无 | 0.0 | 4156.6 | +  0.0% | 0.0 | 1002.9 | +  0.0% |
| lrzip | 500MB Large Dataset (500MB) | L6 | 无 | 0.0 | 2500.1 | +  0.0% | 0.0 | 913.2 | +  0.0% |
| lrzip | Float32 Sensor Matrix (50MB) | L1 | 无 | 0.0 | 132.6 | +  0.0% | 0.0 | 48.2 | +  0.0% |
| lrzip | Float32 Sensor Matrix (50MB) | L6 | 无 | 0.0 | 110.3 | +  0.0% | 0.0 | 48.8 | +  0.0% |
| lrzip | High-Entropy Payload (100MB) | L1 | 无 | 0.0 | 94.9 | +  0.0% | 0.0 | 559.9 | +  0.0% |
| lrzip | High-Entropy Payload (100MB) | L6 | 无 | 0.0 | 109.0 | +  0.0% | 0.0 | 526.0 | +  0.0% |
| lrzip | Log Text (10MB) | L1 | 无 | 0.0 | 1560.6 | +  0.0% | 0.0 | 80.8 | +  0.0% |
| lrzip | Log Text (10MB) | L6 | 无 | 0.0 | 796.0 | +  0.0% | 0.0 | 78.8 | +  0.0% |
| lrzip | Small Files (10MB/100 files) | L1 | 无 | 0.0 | 1426.3 | +  0.0% | 0.0 | 98.2 | +  0.0% |
| lrzip | Small Files (10MB/100 files) | L6 | 无 | 0.0 | 886.1 | +  0.0% | 0.0 | 94.9 | +  0.0% |
| lrzip | Structured JSON (50MB) | L1 | 无 | 0.0 | 2788.4 | +  0.0% | 0.0 | 271.7 | +  0.0% |
| lrzip | Structured JSON (50MB) | L6 | 无 | 0.0 | 1453.9 | +  0.0% | 0.0 | 262.0 | +  0.0% |
| lz4 | 500MB Large Dataset (500MB) | L1 | 无 | 0.0 | 15474.7 | +  0.0% | 0.0 | 2506.5 | +  0.0% |
| lz4 | 500MB Large Dataset (500MB) | L6 | 无 | 0.0 | 15627.3 | +  0.0% | 0.0 | 1967.5 | +  0.0% |
| lz4 | Float32 Sensor Matrix (50MB) | L1 | 无 | 0.0 | 4782.1 | +  0.0% | 0.0 | 369.8 | +  0.0% |
| lz4 | Float32 Sensor Matrix (50MB) | L6 | 无 | 0.0 | 1227.4 | +  0.0% | 0.0 | 352.0 | +  0.0% |
| lz4 | High-Entropy Payload (100MB) | L1 | 无 | 0.0 | 5057.2 | +  0.0% | 0.0 | 613.0 | +  0.0% |
| lz4 | High-Entropy Payload (100MB) | L6 | 无 | 0.0 | 1266.2 | +  0.0% | 0.0 | 606.4 | +  0.0% |
| lz4 | Log Text (10MB) | L1 | 无 | 0.0 | 5267.1 | +  0.0% | 0.0 | 85.3 | +  0.0% |
| lz4 | Log Text (10MB) | L6 | 无 | 0.0 | 5200.1 | +  0.0% | 0.0 | 85.0 | +  0.0% |
| lz4 | Small Files (10MB/100 files) | L1 | 无 | 0.0 | 3023.8 | +  0.0% | 0.0 | 101.7 | +  0.0% |
| lz4 | Small Files (10MB/100 files) | L6 | 无 | 0.0 | 2733.7 | +  0.0% | 0.0 | 101.8 | +  0.0% |
| lz4 | Structured JSON (50MB) | L1 | 无 | 0.0 | 10457.0 | +  0.0% | 0.0 | 319.9 | +  0.0% |
| lz4 | Structured JSON (50MB) | L6 | 无 | 0.0 | 11288.3 | +  0.0% | 0.0 | 320.2 | +  0.0% |
| lzip | 500MB Large Dataset (500MB) | L1 | 无 | 0.0 | 4345.2 | +  0.0% | 0.0 | 1007.5 | +  0.0% |
| lzip | 500MB Large Dataset (500MB) | L6 | 无 | 0.0 | 2617.7 | +  0.0% | 0.0 | 1018.2 | +  0.0% |
| lzip | Float32 Sensor Matrix (50MB) | L1 | 无 | 0.0 | 132.9 | +  0.0% | 0.0 | 48.8 | +  0.0% |
| lzip | Float32 Sensor Matrix (50MB) | L6 | 无 | 0.0 | 115.0 | +  0.0% | 0.0 | 49.6 | +  0.0% |
| lzip | High-Entropy Payload (100MB) | L1 | 无 | 0.0 | 99.8 | +  0.0% | 0.0 | 576.5 | +  0.0% |
| lzip | High-Entropy Payload (100MB) | L6 | 无 | 0.0 | 116.4 | +  0.0% | 0.0 | 575.1 | +  0.0% |
| lzip | Log Text (10MB) | L1 | 无 | 0.0 | 1438.4 | +  0.0% | 0.0 | 80.8 | +  0.0% |
| lzip | Log Text (10MB) | L6 | 无 | 0.0 | 838.4 | +  0.0% | 0.0 | 79.3 | +  0.0% |
| lzip | Small Files (10MB/100 files) | L1 | 无 | 0.0 | 1360.2 | +  0.0% | 0.0 | 97.1 | +  0.0% |
| lzip | Small Files (10MB/100 files) | L6 | 无 | 0.0 | 869.2 | +  0.0% | 0.0 | 94.9 | +  0.0% |
| lzip | Structured JSON (50MB) | L1 | 无 | 0.0 | 2929.6 | +  0.0% | 0.0 | 271.7 | +  0.0% |
| lzip | Structured JSON (50MB) | L6 | 无 | 0.0 | 1428.5 | +  0.0% | 0.0 | 256.6 | +  0.0% |
| tar | 500MB Large Dataset (500MB) | L1 | 无 | 0.0 | 5751.9 | +  0.0% | 0.0 | 8687.0 | +  0.0% |
| tar | 500MB Large Dataset (500MB) | L6 | 无 | 0.0 | 5779.7 | +  0.0% | 0.0 | 8495.5 | +  0.0% |
| tar | Float32 Sensor Matrix (50MB) | L1 | 无 | 0.0 | 5406.4 | +  0.0% | 0.0 | 7919.8 | +  0.0% |
| tar | Float32 Sensor Matrix (50MB) | L6 | 无 | 0.0 | 5541.1 | +  0.0% | 0.0 | 8079.0 | +  0.0% |
| tar | High-Entropy Payload (100MB) | L1 | 无 | 0.0 | 5418.8 | +  0.0% | 0.0 | 8778.5 | +  0.0% |
| tar | High-Entropy Payload (100MB) | L6 | 无 | 0.0 | 5612.7 | +  0.0% | 0.0 | 9474.5 | +  0.0% |
| tar | Log Text (10MB) | L1 | 无 | 0.0 | 4582.7 | +  0.0% | 0.0 | 7195.2 | +  0.0% |
| tar | Log Text (10MB) | L6 | 无 | 0.0 | 4616.5 | +  0.0% | 0.0 | 7220.5 | +  0.0% |
| tar | Small Files (10MB/100 files) | L1 | 无 | 0.0 | 2963.5 | +  0.0% | 0.0 | 1505.3 | +  0.0% |
| tar | Small Files (10MB/100 files) | L6 | 无 | 0.0 | 2948.2 | +  0.0% | 0.0 | 1508.7 | +  0.0% |
| tar | Structured JSON (50MB) | L1 | 无 | 0.0 | 5732.4 | +  0.0% | 0.0 | 9996.7 | +  0.0% |
| tar | Structured JSON (50MB) | L6 | 无 | 0.0 | 5643.4 | +  0.0% | 0.0 | 9553.4 | +  0.0% |
| tar.bz2 | 500MB Large Dataset (500MB) | L1 | 无 | 0.0 | 2667.2 | +  0.0% | 0.0 | 2049.0 | +  0.0% |
| tar.bz2 | 500MB Large Dataset (500MB) | L6 | 无 | 0.0 | 2694.7 | +  0.0% | 0.0 | 2047.3 | +  0.0% |
| tar.bz2 | Float32 Sensor Matrix (50MB) | L1 | 无 | 0.0 | 251.0 | +  0.0% | 0.0 | 35.4 | +  0.0% |
| tar.bz2 | Float32 Sensor Matrix (50MB) | L6 | 无 | 0.0 | 227.0 | +  0.0% | 0.0 | 31.9 | +  0.0% |
| tar.bz2 | High-Entropy Payload (100MB) | L1 | 无 | 0.0 | 216.7 | +  0.0% | 0.0 | 39.1 | +  0.0% |
| tar.bz2 | High-Entropy Payload (100MB) | L6 | 无 | 0.0 | 230.6 | +  0.0% | 0.0 | 33.7 | +  0.0% |
| tar.bz2 | Log Text (10MB) | L1 | 无 | 0.0 | 89.0 | +  0.0% | 0.0 | 258.2 | +  0.0% |
| tar.bz2 | Log Text (10MB) | L6 | 无 | 0.0 | 79.5 | +  0.0% | 0.0 | 249.4 | +  0.0% |
| tar.bz2 | Small Files (10MB/100 files) | L1 | 无 | 0.0 | 106.2 | +  0.0% | 0.0 | 209.8 | +  0.0% |
| tar.bz2 | Small Files (10MB/100 files) | L6 | 无 | 0.0 | 82.6 | +  0.0% | 0.0 | 155.0 | +  0.0% |
| tar.bz2 | Structured JSON (50MB) | L1 | 无 | 0.0 | 123.8 | +  0.0% | 0.0 | 250.2 | +  0.0% |
| tar.bz2 | Structured JSON (50MB) | L6 | 无 | 0.0 | 96.3 | +  0.0% | 0.0 | 243.6 | +  0.0% |
| tar.gz | 500MB Large Dataset (500MB) | L1 | 无 | 0.0 | 15249.9 | +  0.0% | 0.0 | 5885.3 | +  0.0% |
| tar.gz | 500MB Large Dataset (500MB) | L6 | 无 | 0.0 | 12269.7 | +  0.0% | 0.0 | 2953.2 | +  0.0% |
| tar.gz | Float32 Sensor Matrix (50MB) | L1 | 无 | 0.0 | 1783.4 | +  0.0% | 0.0 | 536.2 | +  0.0% |
| tar.gz | Float32 Sensor Matrix (50MB) | L6 | 无 | 0.0 | 1340.1 | +  0.0% | 0.0 | 506.8 | +  0.0% |
| tar.gz | High-Entropy Payload (100MB) | L1 | 无 | 0.0 | 1831.7 | +  0.0% | 0.0 | 5161.4 | +  0.0% |
| tar.gz | High-Entropy Payload (100MB) | L6 | 无 | 0.0 | 1568.3 | +  0.0% | 0.0 | 5446.0 | +  0.0% |
| tar.gz | Log Text (10MB) | L1 | 无 | 0.0 | 8270.5 | +  0.0% | 0.0 | 5198.5 | +  0.0% |
| tar.gz | Log Text (10MB) | L6 | 无 | 0.0 | 5260.3 | +  0.0% | 0.0 | 5372.9 | +  0.0% |
| tar.gz | Small Files (10MB/100 files) | L1 | 无 | 0.0 | 3694.0 | +  0.0% | 0.0 | 1129.2 | +  0.0% |
| tar.gz | Small Files (10MB/100 files) | L6 | 无 | 0.0 | 3209.8 | +  0.0% | 0.0 | 1109.1 | +  0.0% |
| tar.gz | Structured JSON (50MB) | L1 | 无 | 0.0 | 10713.6 | +  0.0% | 0.0 | 5283.6 | +  0.0% |
| tar.gz | Structured JSON (50MB) | L6 | 无 | 0.0 | 8687.4 | +  0.0% | 0.0 | 6236.4 | +  0.0% |
| tar.xz | 500MB Large Dataset (500MB) | L1 | 无 | 0.0 | 4350.1 | +  0.0% | 0.0 | 1031.9 | +  0.0% |
| tar.xz | 500MB Large Dataset (500MB) | L6 | 无 | 0.0 | 2661.7 | +  0.0% | 0.0 | 1019.4 | +  0.0% |
| tar.xz | Float32 Sensor Matrix (50MB) | L1 | 无 | 0.0 | 134.2 | +  0.0% | 0.0 | 48.8 | +  0.0% |
| tar.xz | Float32 Sensor Matrix (50MB) | L6 | 无 | 0.0 | 109.1 | +  0.0% | 0.0 | 49.5 | +  0.0% |
| tar.xz | High-Entropy Payload (100MB) | L1 | 无 | 0.0 | 105.8 | +  0.0% | 0.0 | 577.1 | +  0.0% |
| tar.xz | High-Entropy Payload (100MB) | L6 | 无 | 0.0 | 121.4 | +  0.0% | 0.0 | 542.5 | +  0.0% |
| tar.xz | Log Text (10MB) | L1 | 无 | 0.0 | 1960.0 | +  0.0% | 0.0 | 80.7 | +  0.0% |
| tar.xz | Log Text (10MB) | L6 | 无 | 0.0 | 877.0 | +  0.0% | 0.0 | 78.9 | +  0.0% |
| tar.xz | Small Files (10MB/100 files) | L1 | 无 | 0.0 | 1677.9 | +  0.0% | 0.0 | 98.7 | +  0.0% |
| tar.xz | Small Files (10MB/100 files) | L6 | 无 | 0.0 | 962.9 | +  0.0% | 0.0 | 94.5 | +  0.0% |
| tar.xz | Structured JSON (50MB) | L1 | 无 | 0.0 | 2905.5 | +  0.0% | 0.0 | 270.8 | +  0.0% |
| tar.xz | Structured JSON (50MB) | L6 | 无 | 0.0 | 1484.6 | +  0.0% | 0.0 | 259.1 | +  0.0% |
| tar.zst | 500MB Large Dataset (500MB) | L1 | 无 | 0.0 | 21069.4 | +  0.0% | 0.0 | 4637.3 | +  0.0% |
| tar.zst | 500MB Large Dataset (500MB) | L6 | 无 | 0.0 | 19099.6 | +  0.0% | 0.0 | 5052.7 | +  0.0% |
| tar.zst | Float32 Sensor Matrix (50MB) | L1 | 无 | 0.0 | 3188.4 | +  0.0% | 0.0 | 1042.0 | +  0.0% |
| tar.zst | Float32 Sensor Matrix (50MB) | L6 | 无 | 0.0 | 3108.0 | +  0.0% | 0.0 | 1091.3 | +  0.0% |
| tar.zst | High-Entropy Payload (100MB) | L1 | 无 | 0.0 | 4765.9 | +  0.0% | 0.0 | 4399.4 | +  0.0% |
| tar.zst | High-Entropy Payload (100MB) | L6 | 无 | 0.0 | 4931.5 | +  0.0% | 0.0 | 4668.8 | +  0.0% |
| tar.zst | Log Text (10MB) | L1 | 无 | 0.0 | 9777.4 | +  0.0% | 0.0 | 6029.5 | +  0.0% |
| tar.zst | Log Text (10MB) | L6 | 无 | 0.0 | 5703.9 | +  0.0% | 0.0 | 5887.7 | +  0.0% |
| tar.zst | Small Files (10MB/100 files) | L1 | 无 | 0.0 | 4190.5 | +  0.0% | 0.0 | 1656.2 | +  0.0% |
| tar.zst | Small Files (10MB/100 files) | L6 | 无 | 0.0 | 3511.0 | +  0.0% | 0.0 | 1721.0 | +  0.0% |
| tar.zst | Structured JSON (50MB) | L1 | 无 | 0.0 | 15812.1 | +  0.0% | 0.0 | 4577.6 | +  0.0% |
| tar.zst | Structured JSON (50MB) | L6 | 无 | 0.0 | 12165.0 | +  0.0% | 0.0 | 5330.4 | +  0.0% |
| wim | 500MB Large Dataset (500MB) | L1 | 无 | 0.0 | 5839.6 | +  0.0% | 0.0 | 9981.8 | +  0.0% |
| wim | 500MB Large Dataset (500MB) | L1 | AES | 0.0 | 6035.8 | +  0.0% | 0.0 | 10273.8 | +  0.0% |
| wim | 500MB Large Dataset (500MB) | L6 | 无 | 0.0 | 5730.2 | +  0.0% | 0.0 | 9871.4 | +  0.0% |
| wim | 500MB Large Dataset (500MB) | L6 | AES | 0.0 | 5897.1 | +  0.0% | 0.0 | 10173.4 | +  0.0% |
| wim | Float32 Sensor Matrix (50MB) | L1 | 无 | 0.0 | 5532.5 | +  0.0% | 0.0 | 8214.3 | +  0.0% |
| wim | Float32 Sensor Matrix (50MB) | L1 | AES | 0.0 | 5733.3 | +  0.0% | 0.0 | 8271.4 | +  0.0% |
| wim | Float32 Sensor Matrix (50MB) | L6 | 无 | 0.0 | 5675.2 | +  0.0% | 0.0 | 8215.3 | +  0.0% |
| wim | Float32 Sensor Matrix (50MB) | L6 | AES | 0.0 | 5748.3 | +  0.0% | 0.0 | 8264.2 | +  0.0% |
| wim | High-Entropy Payload (100MB) | L1 | 无 | 0.0 | 5761.4 | +  0.0% | 0.0 | 9727.4 | +  0.0% |
| wim | High-Entropy Payload (100MB) | L1 | AES | 0.0 | 5836.2 | +  0.0% | 0.0 | 9717.3 | +  0.0% |
| wim | High-Entropy Payload (100MB) | L6 | 无 | 0.0 | 5801.7 | +  0.0% | 0.0 | 9752.9 | +  0.0% |
| wim | High-Entropy Payload (100MB) | L6 | AES | 0.0 | 5810.0 | +  0.0% | 0.0 | 9475.5 | +  0.0% |
| wim | Log Text (10MB) | L1 | 无 | 0.0 | 4843.3 | +  0.0% | 0.0 | 8020.3 | +  0.0% |
| wim | Log Text (10MB) | L1 | AES | 0.0 | 4874.2 | +  0.0% | 0.0 | 7736.0 | +  0.0% |
| wim | Log Text (10MB) | L6 | 无 | 0.0 | 4990.7 | +  0.0% | 0.0 | 8176.7 | +  0.0% |
| wim | Log Text (10MB) | L6 | AES | 0.0 | 4974.8 | +  0.0% | 0.0 | 8074.3 | +  0.0% |
| wim | Small Files (10MB/100 files) | L1 | 无 | 0.0 | 2952.0 | +  0.0% | 0.0 | 1547.0 | +  0.0% |
| wim | Small Files (10MB/100 files) | L1 | AES | 0.0 | 2976.1 | +  0.0% | 0.0 | 1456.0 | +  0.0% |
| wim | Small Files (10MB/100 files) | L6 | 无 | 0.0 | 2974.4 | +  0.0% | 0.0 | 1448.5 | +  0.0% |
| wim | Small Files (10MB/100 files) | L6 | AES | 0.0 | 2863.6 | +  0.0% | 0.0 | 1426.2 | +  0.0% |
| wim | Structured JSON (50MB) | L1 | 无 | 0.0 | 5407.8 | +  0.0% | 0.0 | 9023.2 | +  0.0% |
| wim | Structured JSON (50MB) | L1 | AES | 0.0 | 5557.6 | +  0.0% | 0.0 | 9450.1 | +  0.0% |
| wim | Structured JSON (50MB) | L6 | 无 | 0.0 | 5548.1 | +  0.0% | 0.0 | 9109.7 | +  0.0% |
| wim | Structured JSON (50MB) | L6 | AES | 0.0 | 5464.5 | +  0.0% | 0.0 | 9316.7 | +  0.0% |
| zip | 500MB Large Dataset (500MB) | L1 | 无 | 0.0 | 6674.3 | +  0.0% | 0.0 | 9761.2 | +  0.0% |
| zip | 500MB Large Dataset (500MB) | L1 | AES | 0.0 | 6631.3 | +  0.0% | 0.0 | 9550.2 | +  0.0% |
| zip | 500MB Large Dataset (500MB) | L6 | 无 | 0.0 | 111.8 | +  0.0% | 0.0 | 11353.6 | +  0.0% |
| zip | 500MB Large Dataset (500MB) | L6 | AES | 0.0 | 1300.2 | +  0.0% | 0.0 | 10832.4 | +  0.0% |
| zip | Float32 Sensor Matrix (50MB) | L1 | 无 | 0.0 | 179.2 | +  0.0% | 0.0 | 577.7 | +  0.0% |
| zip | Float32 Sensor Matrix (50MB) | L1 | AES | 0.0 | 168.3 | +  0.0% | 0.0 | 0.0 | +  0.0% |
| zip | Float32 Sensor Matrix (50MB) | L6 | 无 | 0.0 | 22.8 | +  0.0% | 0.0 | 573.6 | +  0.0% |
| zip | Float32 Sensor Matrix (50MB) | L6 | AES | 0.0 | 128.3 | +  0.0% | 0.0 | 0.0 | +  0.0% |
| zip | High-Entropy Payload (100MB) | L1 | 无 | 0.0 | 173.6 | +  0.0% | 0.0 | 9862.1 | +  0.0% |
| zip | High-Entropy Payload (100MB) | L1 | AES | 0.0 | 164.4 | +  0.0% | 0.0 | 2202.1 | +  0.0% |
| zip | High-Entropy Payload (100MB) | L6 | 无 | 0.0 | 4738.2 | +  0.0% | 0.0 | 10042.1 | +  0.0% |
| zip | High-Entropy Payload (100MB) | L6 | AES | 0.0 | 135.3 | +  0.0% | 0.0 | 2195.7 | +  0.0% |
| zip | Log Text (10MB) | L1 | 无 | 0.0 | 5049.3 | +  0.0% | 0.0 | 7414.0 | +  0.0% |
| zip | Log Text (10MB) | L1 | AES | 0.0 | 3736.1 | +  0.0% | 0.0 | 0.0 | +  0.0% |
| zip | Log Text (10MB) | L6 | 无 | 0.0 | 3.3 | +  0.0% | 0.0 | 6992.0 | +  0.0% |
| zip | Log Text (10MB) | L6 | AES | 0.0 | 1224.8 | +  0.0% | 0.0 | 4896.6 | +  0.0% |
| zip | Small Files (10MB/100 files) | L1 | 无 | 0.0 | 7396.9 | +  0.0% | 0.0 | 2274.1 | +  0.0% |
| zip | Small Files (10MB/100 files) | L1 | AES | 0.0 | 2350.6 | +  0.0% | 0.0 | 2098.1 | +  0.0% |
| zip | Small Files (10MB/100 files) | L6 | 无 | 0.0 | 5954.6 | +  0.0% | 0.0 | 2252.6 | +  0.0% |
| zip | Small Files (10MB/100 files) | L6 | AES | 0.0 | 2185.2 | +  0.0% | 0.0 | 2027.9 | +  0.0% |
| zip | Structured JSON (50MB) | L1 | 无 | 0.0 | 4816.3 | +  0.0% | 0.0 | 8081.8 | +  0.0% |
| zip | Structured JSON (50MB) | L1 | AES | 0.0 | 4269.8 | +  0.0% | 0.0 | 0.0 | +  0.0% |
| zip | Structured JSON (50MB) | L6 | 无 | 0.0 | 5.7 | +  0.0% | 0.0 | 8755.3 | +  0.0% |
| zip | Structured JSON (50MB) | L6 | AES | 0.0 | 1316.3 | +  0.0% | 0.0 | 0.0 | +  0.0% |
