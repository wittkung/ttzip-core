# TTZip - App Store Connect & Steam 商店中英文文案与元数据清单

## 1. 基础应用信息 (Basic Application Metadata)

- **应用主标题 (App Name)**: TTZip - High Performance Archiver / TTZip 极速解压与原生归档
- **副标题 (Subtitle / Tagline)**: Apple Silicon 原生极速压缩与智能预览 (29 字) / Native Apple Silicon Fast Archiver (28 chars)
- **主要类别 (Primary Category)**: 实用工具 (Utilities)
- **次要类别 (Secondary Category)**: 效率 (Productivity)
- **年龄分级 (Age Rating)**: 4+ (适合所有年龄段，无任何敏感内容)
- **定价策略 (Pricing)**: 
  - Tier 4: ¥29.00 (人民币) / $4.99 (美元) 一次性买断，终身免费更新，零内购，零广告订阅
- **版权声明 (Copyright)**: © 2026 TTZip Technologies. All rights reserved.

---

## 2. 中文商店详情文案 (Chinese Store Description)

### 宣传文本 (Promotional Text - 170字符以内，可随时更新)
> 专为 Apple Silicon 芯片硬件级重构的新一代 macOS 极速解压与归档利器。彻底告别卡顿、乱码与等待，体验微秒级瞬时响应！

### 完整应用介绍 (Full Description)
TTZip 是一款为 macOS 深度定制的高性能归档与压缩工具。全代码基于 Swift 6 严格并发模型与 Rust 零成本抽象重构，深度挖掘 Apple Silicon（M1/M2/M3/M4/M5 系列芯片）的 NEON 硬件矢量加速与 APFS 现代文件系统潜能。

【核心性能与硬件优势】
• Apple Silicon 原生优化：利用 NEON SIMD 指令集进行并行 CRC32 校验与数据流编解码，性能超越传统解压工具 3~5 倍。
• APFS 写时复制（Clonefile）：在同一磁盘内提取或复制归档，实现接近 0 秒的微秒级极速克隆，零额外磁盘损耗。
• 16KB 物理页内存对齐：彻底杜绝跨页内存搬运，降低多核解压时的 CPU 温度与能耗。

【全能格式与广泛兼容】
• 完美支持 16 种主流归档格式：ZIP、7Z、TAR、GZ、BZ2、XZ、ZSTD、LZ4、LZFSE、RAR (解压)、ISO、DMG、CAB、ARJ、CPGZ、PAX。
• 智能编码识别引擎：内置 uchardet 多语言特征检测，彻底解决 Windows 发送的 ZIP 压缩包在 Mac 下文件名乱码的顽疾（自动适配 GB18030、Big5、Shift-JIS、EUC-KR）。
• 智能去污防污染：自动过滤 macOS 系统生成的 `__MACOSX`、`.DS_Store` 等隐藏垃圾文件，让发往 Windows 端的压缩包干净清爽。

【现代化桌面交互与效率】
• 隔空即时预览 (Quick Look)：无需完整解压，直接在压缩包内按空格键预览 Office 文档、代码高亮、音视频与图片。
• 典雅 Miller Columns（分栏浏览）：支持在压缩包内像 Finder 一样层层穿透、单文件独立拖拽提取。
• 银行级安全保险箱 (Password Vault)：AES-256-GCM 强加密支持，内存采用安全零化（Zeroization）技术，密码阅后即焚，杜绝内存泄漏风险。
• 纯净与隐私至上：零联网请求、零数据追踪、零广告、零强制订阅，给您最纯粹的 macOS 工具体验。

### 搜索关键词 (Keywords - 100字符以内，逗号分隔)
解压,压缩,zip,7z,rar,tar,快速解压,mac解压,文件解压,压缩工具,解压专家,归档,apple silicon

---

## 3. 英文商店详情文案 (English Store Description)

### Promotional Text
> Engineered natively for Apple Silicon. Experience ultra-fast, zero-overhead archive compression and decompression on macOS.

### Full Description
TTZip is a state-of-the-art native macOS archiver built from the ground up with Swift 6 and Rust. Designed exclusively for Apple Silicon (M1/M2/M3/M4/M5), TTZip unlocks the full hardware potential of your Mac with SIMD NEON acceleration and APFS copy-on-write capabilities.

KEY HIGHLIGHTS:
• Apple Silicon Hardware Acceleration: Hardware-vectorized CRC32 computation and parallel codec pipelines deliver up to 5x faster processing throughput.
• APFS Instant Clonefile: Near-instantaneous zero-copy extraction across the same volume without doubling disk wear.
• Broad Format Compatibility: Comprehensive support for 16 major archive standards including ZIP, 7Z, TAR, GZ, BZ2, XZ, ZSTD, LZ4, LZFSE, RAR, ISO, and more.
• Intelligent Encoding Engine: Automatic CJK charset detection eliminates corrupted or garbled filenames generated from legacy Windows systems.
• Smart Apple Junk Filter: Automatically strips `__MACOSX` and `.DS_Store` artifacts when sharing archives with Windows and Linux users.
• In-Archive Quick Look: Preview images, audio waveforms, video clips, markdown, and code syntax directly inside archives without full extraction.
• Miller Column Navigation: Fluid multi-level directory exploration matching native macOS Finder ergonomics.
• Enterprise Security: AES-256 encryption with zeroized memory buffers for bulletproof cryptographic safety.

### Keywords
unzip,unarchiver,zip,7z,rar,tar,extractor,compression,fast unzip,apple silicon,file compressor,archiver

---

## 4. 官方支持与隐私链接 (URLs)
- **支持主页 (Support URL)**: `https://ttzip.app/support`
- **营销主页 (Marketing URL)**: `https://ttzip.app`
- **隐私政策 (Privacy Policy URL)**: `https://ttzip.app/privacy`
