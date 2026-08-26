# Research: All 16 Formats Competitor Benchmark Architecture

## 1. 16 种归档格式与竞品 CLI 对照映射表

| 归档格式 | 格式标识 | 核心算法 / 特性 | 竞品 1（推荐 / 多核） | 竞品 2（系统级 / 官方） | 加密支持 |
| :--- | :---: | :--- | :--- | :--- | :---: |
| **ZIP** | `.zip` | Deflate / Zstd / Store | Apple `ditto` | Info-ZIP `zip` | AES-256 / ZipCrypto |
| **7Z** | `.7z` | LZMA2 / BCJ / Copy | 7-Zip `7zz` (v24.x) | 7-Zip `7z` | AES-256 |
| **TAR.GZ** | `.tar.gz` | Gzip / Deflate + Tar | `pigz` (Multi-core) | macOS `bsdtar` / `gzip` | 无 |
| **TAR.ZST** | `.tar.zst` | Zstandard (v1.5.6) + Tar | Meta `zstd -T0` | BSD `tar` (zstd) | 无 |
| **TAR.BZ2** | `.tar.bz2` | Bzip2 + Tar | `pbzip2` (All Cores) | macOS `bzip2` | 无 |
| **TAR.XZ** | `.tar.xz` | XZ / LZMA2 + Tar | `pixz` (Parallel XZ) | `xz -T0` / 7-Zip `7zz` | 无 |
| **TAR** | `.tar` | POSIX USTAR / Pax | macOS `bsdtar` | GNU `tar` | 无 |
| **LZIP** | `.lz` | LZMA + Tar | `plzip` (Multi-thread) | `lzip` (Standard) | 无 |
| **LZ4** | `.lz4` | LZ4 Frame + Tar | Official `lz4` CLI | liblz4 | 无 |
| **BROTLI** | `.br` | Brotli + Tar | Google `brotli` CLI | libbrotli | 无 |
| **LRZIP** | `.lrz` | Long Range Zip + rzip | `lrzip` (Multi-core) | rzip | 无 |
| **AAR** | `.aar` | AppleArchive LZFSE/LZ4 | Apple `aa` (`/usr/bin/aa`) | AppleArchive | 无 |
| **SNAPPY** | `.sz` | Snappy / Framed | `snappy` / `szip` | Snappy C | 无 |
| **WIM** | `.wim` | Windows Imaging LZX | `wimlib-imagex` | 7-Zip `7zz` | 无 |
| **DMG** | `.dmg` | Apple Disk Image UDZO | Apple `hdiutil` | 7-Zip `7zz` | 无 |
| **ISO** | `.iso` | ISO-9660 / Joliet | Apple `hdiutil` | 7-Zip `7zz` | 无 |

## 2. 自动化执行与生命周期保障
1. **自动工具探查与优雅回退**：
   在执行前使用 `CompetitorDetector.findExecutable` 扫描 `/opt/homebrew/bin`、`/usr/bin` 与 `/usr/local/bin`。若未安装特定独立 CLI，自动降级至 7zz 或跳过竞品执行，确保测试流水线不中断。
2. **生命周期磁盘隔离**：
   每个格式生成临时归档与解压文件夹后，立即调用 `removeItem`，避免 16 种格式在多轮测试中累积产生数十 GB 磁盘脏页。
