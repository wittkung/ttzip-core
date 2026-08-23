# Quickstart & Validation Guide: Google Zopfli 官方上游集成与极限压制验证

**Feature ID**: `105-zopfli-upstream-integration-and-research`  
**Created**: 2026-08-19  
**Status**: DRAFT (Phase 1 Validation Guide)  

---

## 1. 验证场景一：Vendor Upstream 独立构建与测试

### 1.1 Command
```bash
cd Vendor/zopfli-upstream && make clean && make -j8 && ./zopfli --help
```

### 1.2 Expected Output
- 控制台编译无任何 error，成功生成 `zopfli` 可执行二进制。
- 运行 `./zopfli --help` 正常打印官方命令行参数列表（包含 `--i5`、`--i15`、`--blocksplitting` 等）。

### 1.3 Failure Diagnostic
- 若报头文件未找到：检查 `src/zopfli/` 目录下 `.c` 与 `.h` 是否完整。
- 若链接失败：检查是否缺失 `m` 标准数学库（`-lm`）。

---

## 2. 验证场景二：TTZip 进程内 18 核心分块多线程 Zopfli 单元测试

### 2.1 Command
```bash
swift test --filter ZipExtremeBlockWriterTests
```

### 2.2 Expected Output
- `ZipExtremeBlockWriterTests` 2 个测试全部通过（`0 failures`）。
- 控制台输出系统自带 `/usr/bin/unzip -t` 校验通过，`0 errors detected`。

### 2.3 Failure Diagnostic
- 若 `/usr/bin/unzip -t` 报告 CRC 错误或 `unexpected EOF`：检查前 $N-1$ 块是否正确设置了 `BFINAL=0` 与 `Z_SYNC_FLUSH` 对齐标记。

---

## 3. 验证场景三：全量 8 档位现场实测与帕累托前沿判定

### 3.1 Command
```bash
TTZIP_BENCH_ALL_LIVE=1 swift test --filter ZipMultiCoreParetoFrontierPkTests
```

### 3.2 Expected Output
- Tier 6 现场实跑产出体积 $< 3.01\text{ MB}$（$\le 2.99\text{ MB}$），实测吞吐 $\ge 4.5\text{ MB/s}$。
- Tier 7 现场实跑产出体积 $< 2.99\text{ MB}$（$\le 2.95\text{ MB}$），实测吞吐 $\ge 1.5\text{ MB/s}$。
- 生成的 `pareto_pk_zip_multicore.png` 中，TTZip L6 严格位于 `pigz -11` 右上方，L7 严格位于 `advzip -4` 右上方。

### 3.3 Failure Diagnostic
- 若 Tier 6 体积超过 $3.01\text{ MB}$：检查 `num_iterations` 是否生效以及 32KB 跨块字典历史是否成功加载。
