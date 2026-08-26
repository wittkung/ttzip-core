# Quickstart Validation Guide: TTZip 专业归档能力验收指南

**Feature Branch**: `082-pro-software-gap-audit`  
**Purpose**: 验证 TTZip 5 大专业级能力（智能解压、分卷创建、7Z 头部加密、就地编辑、恢复记录）的端到端正确性与鲁棒性。

---

## 1. 验证场景一：智能解压与元数据清洗 (Smart Extraction & Metadata Clean)

### Command
```bash
# 1. 准备单根目录归档与多散落文件归档
mkdir -p /tmp/ttzip_smart_test/MyProject && echo "hello" > /tmp/ttzip_smart_test/MyProject/main.c
swift run ttzip-cli compress -f zip -i /tmp/ttzip_smart_test/MyProject -o /tmp/ttzip_smart_test/SingleRoot.zip

echo "file1" > /tmp/ttzip_smart_test/file1.txt && echo "file2" > /tmp/ttzip_smart_test/file2.txt
swift run ttzip-cli compress -f zip -i /tmp/ttzip_smart_test/file1.txt /tmp/ttzip_smart_test/file2.txt -o /tmp/ttzip_smart_test/MultiRoot.zip

# 2. 执行智能解压
swift run ttzip-cli extract --smart /tmp/ttzip_smart_test/SingleRoot.zip -o /tmp/ttzip_smart_out_single
swift run ttzip-cli extract --smart /tmp/ttzip_smart_test/MultiRoot.zip -o /tmp/ttzip_smart_out_multi
```

### Expected Output
```text
[SmartExtract] Single root detected: 'MyProject'. Direct extracting without redundant nesting.
Extraction completed successfully: /tmp/ttzip_smart_out_single/MyProject/main.c

[SmartExtract] Multiple roots detected (2 items). Wrapping in container folder 'MultiRoot'.
Extraction completed successfully: /tmp/ttzip_smart_out_multi/MultiRoot/file1.txt, file2.txt
```

### Failure Diagnostic
- **若生成了 `MyProject/MyProject/` 双层嵌套**：检查 `PathPatternFilterEngine` 是否未能排除 `.DS_Store` 或 `__MACOSX` 导致 `effectiveRootCount > 1`。
- **若多文件直接散落在目标根目录**：检查 `smartExtractStrategy` 的 `resolutionMode` 是否误判为 `directExtract`。

---

## 2. 验证场景二：多格式自适应分卷归档切片与合并 (Split Volume Spanning)

### Command
```bash
# 生成 50MB 测试文件并按 10MB 分卷切片压缩为 7Z
dd if=/dev/urandom of=/tmp/ttzip_split_test.bin bs=1M count=50
swift run ttzip-cli compress -f 7z --split 10M -i /tmp/ttzip_split_test.bin -o /tmp/ttzip_split.7z

# 验证生成切片
ls -l /tmp/ttzip_split.7z.*

# 验证解压合并
swift run ttzip-cli extract /tmp/ttzip_split.7z.001 -o /tmp/ttzip_split_out/
shasum -a 256 /tmp/ttzip_split_test.bin /tmp/ttzip_split_out/ttzip_split_test.bin
```

### Expected Output
```text
/tmp/ttzip_split.7z.001  10485760 bytes
/tmp/ttzip_split.7z.002  10485760 bytes
/tmp/ttzip_split.7z.003  10485760 bytes
/tmp/ttzip_split.7z.004  10485760 bytes
/tmp/ttzip_split.7z.005  10485760 bytes
/tmp/ttzip_split.7z.006   2457600 bytes
[SplitExtract] Detected multi-volume set (6 volumes). Slicing stitched stream...
SHA-256 Checksums match 100%.
```

### Failure Diagnostic
- **若解压报 `StartHeader CRC mismatch`**：检查首卷 `.7z.001` 的 32 字节延迟修补（Rewind Patching）是否因缺少 `fsync` 未落盘。
- **若第三方 7-Zip 提示缺少分卷**：检查分卷命名是否为标准 3 位十进制编号 `.001` .. `.006`。

---

## 3. 验证场景三：7Z 头部文件名加密与生物认证 (7Z Header Encryption)

### Command
```bash
# 创建头部加密归档 (-mhe)
swift run ttzip-cli compress -f 7z --password "SecretPass123" --encrypt-header -i /tmp/ttzip_smart_test/MyProject -o /tmp/ttzip_secret.7z

# 无密码尝试列出文件清单
swift run ttzip-cli list /tmp/ttzip_secret.7z
```

### Expected Output
```text
[7Z Header Parser] kEncodedHeader (ID 0x17) detected. Central directory is AES-256 encrypted.
Error: Password required to read archive structure.
```

### Failure Diagnostic
- **若无需密码即可列出文件名**：检查 `ttzip_7z_header_writer.c` 是否未将 `kFilesInfo` 封装至 `kEncodedHeader` 加密流。

---

## 4. 验证场景四：Reed-Solomon 恢复记录与灾难自愈 (Recovery Record & Repair)

### Command
```bash
# 1. 创建带 5% 恢复记录的归档
swift run ttzip-cli compress -f zip --recovery-record 5 -i /tmp/ttzip_split_test.bin -o /tmp/ttzip_protected.zip

# 2. 人为注入 512KB 损坏坏块
dd if=/dev/zero of=/tmp/ttzip_protected.zip bs=1k seek=1000 count=512 conv=notrunc

# 3. 启动体检与自愈修复
swift run ttzip-cli test --repair /tmp/ttzip_protected.zip
```

### Expected Output
```text
[RecoveryRecord] TTZIP_RR footer detected. Parity budget: 5% (128 recovery slices).
[RS-FEC Localization] 8 corrupted slices detected via BLAKE3 hash check.
[RS-FEC Reconstruction] Solving Cauchy Matrix over GF(2^16)... Slices repaired: 8/8.
[Result] Archive 100% repaired and verified.
```

### Failure Diagnostic
- **若修复失败报 `Unrecoverable corruption`**：检查损坏切片数是否超过 $M$（5% 预算），或 Cauchy 矩阵消元是否有溢出。
