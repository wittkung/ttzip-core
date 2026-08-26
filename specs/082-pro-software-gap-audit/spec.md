# Feature Specification: TTZip 对标全球顶级专业归档软件全维度差距审计与工程落地方案

**Feature Branch**: `082-pro-software-gap-audit`  
**Created**: 2026-08-18  
**Status**: Draft  
**Input**: User description: "全面审计 ttzip，看看距离专业软件差距在哪 /speckit-specify"

---

## 1. 全球顶级专业归档软件全维度对标矩阵 (Professional Competitor Benchmark Audit)

经过对全球五大顶级商业与开源归档标杆软件（**BetterZip 5**, **Keka 1.4**, **WinRAR 7**, **7-Zip 24**, **Bandizip 7**）的深度横向对比与 TTZip 现状全面审计，TTZip 在 **底层 C 引擎吞吐量（Apple Silicon NEON 加速、libdeflate、fast-lzma2、zstd、pmull crc64）** 已经建立了绝对优势（在 16 种格式上领先竞品 1.5x ~ 10x），但在 **桌面工作流自动化**、**分卷归档创建**、**7Z 文件名头部加密**、**外部应用就地编辑协同**、**灾难恢复纠错记录** 以及 **系统级快捷集成** 维度仍存在明显差距：

| 评测维度 | BetterZip 5 (macOS) | Keka 1.4 (macOS) | WinRAR 7 (Win) | 7-Zip 24 (Win/Linux) | TTZip (当前现状) | TTZip 专业版目标 |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **底层多核吞吐** | 中等 (依赖外部 CLI) | 良好 (p7zip 移植) | 良好 (x86 多核) | 优秀 (原生 C++ 多核) | **行业最高 (NEON 原生 C)** | **行业最高 (保持绝对领先)** |
| **格式覆盖度** | 30+ 格式浏览 | 15+ 格式解压 | RAR/ZIP/7Z 主导 | 7Z/ZIP/TAR 主导 | **16 种格式完整压缩/解压** | **16+ 全格式完整支持** |
| **智能解压 (Smart Extract)** | 支持 | 支持 | 需手动选择 | 不支持 | 基础解压 | **完整智能去重与自动建夹** |
| **分卷压缩 (Split Volumes)** | 支持 (zip, 7z) | 支持 (7z, zip, tar) | 极强 (part1.rar) | 极强 (.7z.001) | 仅支持分卷读取解压 | **全格式分卷创建与合并** |
| **7Z 头部加密 (-mhe)** | 支持 | 支持 | 支持 (RAR5) | 支持 (-mhe) | 仅支持数据内容加密 | **完整 7Z 文件名加密** |
| **密码库生物识别** | 主密码/Keychain | Keychain 基础 | 不支持 | 不支持 | Keychain 基础 | **Touch ID / Apple Watch 认证** |
| **外部编辑器热协同** | 行业标杆 (自动回写) | 不支持 | 临时解压回写 | 基础临时文件 | 具备底层引擎，缺少 GUI 桥接 | **双向 FSEvents 自动侦听回写** |
| **恢复记录 (Recovery Record)** | 不支持 | 不支持 | 行业标杆 (RR 1-10%) | 不支持 | 基础 CRC 校验 | **Reed-Solomon 1-10% 纠错自愈** |
| **批处理排队流水线** | 队列管理 | 批量并发 | 脚本批处理 | 命令行批处理 | 基础并发任务 | **多任务流控与操作后自动处理** |
| **系统级 QuickLook / Finder** | 极强 (原生插件) | Finder 扩展 | 资源管理器右键 | 资源管理器右键 | 视图内预览 | **QuickLook 预览与 Finder 服务** |
| **GUI 算力基准测试** | 不支持 | 不支持 | 内置 Benchmark | 行业标杆 (MIPS) | 仅 CLI 基准 | **原生 SwiftUI MIPS 算力仪表盘** |

---

## 2. User Scenarios & Testing *(mandatory)*

### User Story 1 - 智能解压与操作后自动化流水线 (Smart Extraction & Post-Operation Workflow Automation) (Priority: P1)

作为经常需要处理海量外部压缩包的 macOS 专业用户，我希望拖拽压缩包解压时能够“智能判断根目录”（若包内已有单一顶级文件夹则不额外建夹，若包含多个散乱文件则自动创建同名文件夹），并在解压或压缩完成后支持自动将原文件移入废纸篓、在 Finder 中定位目标文件并触发系统提示音，从而实现完全零摩擦的自动化归档体验。

**Why this priority**: 智能解压与操作后清理是 macOS 桌面归档工具最核心的高频体验功能（对标 Keka / Bandizip），直接决定日常使用的顺手度与效率。

**Independent Test**: 传入单文件夹归档与多文件归档分别触发解压，验证目标目录结构无多余嵌套，且源文件按配置自动移至废纸篓并发送系统通知。

**Acceptance Scenarios**:
1. **Given** 一个包含单一顶层目录 `Project/` 的归档，**When** 用户执行智能解压，**Then** 直接在解压目标目录解压出 `Project/`，不创建多余的嵌套层级 `archive/Project/`。
2. **Given** 一个包含 5 个根文件的归档 `photos.zip`，**When** 用户执行智能解压，**Then** 系统自动创建 `photos/` 目录并将 5 个文件放入其中。
3. **Given** 开启了“完成后将源文件移入废纸篓”与“在 Finder 中显示”，**When** 压缩或解压任务完成，**Then** 源文件安全移入 macOS 废纸篓，系统通知弹出并在 Finder 窗口高亮输出文件。

---

### User Story 2 - 多格式多卷自适应分卷归档创建与无缝合并 (Multi-Format Split-Volume Spanning & Merging) (Priority: P1)

作为需要通过网盘、邮件或 FAT32 U 盘传输超大文件的工程人员，我希望在创建 7Z、ZIP、TAR 等归档时，能够自由选择分卷预设（CD 700MB, DVD 4.7GB, FAT32 4095MB, 邮件/微信 25MB/100MB 或自定义 MB/GB），生成标准多卷切片（`.7z.001`, `.7z.002` 或 `.zip.001`, `.zip.002`），并支持在任何切片上双击一键合并解压。

**Why this priority**: 分卷压缩是大文件存储和跨设备分发不可或缺的工业级能力（对标 WinRAR / 7-Zip / Keka）。

**Independent Test**: 创建一个 100MB 测试文件并设置分卷大小为 25MB 压缩为 7Z，验证生成 4 个精准切片，且使用官方 7-Zip、The Unarchiver 及 TTZip 均能 100% 校验解压出原文件。

**Acceptance Scenarios**:
1. **Given** 用户选择待压缩文件夹，**When** 在压缩面板选择分卷大小为 100MB，**Then** 输出 `Archive.7z.001`, `Archive.7z.002` 等标准切片，每个切片尺寸精确受控。
2. **Given** 多个分卷切片文件，**When** 用户将 `.001` 文件拖入 TTZip，**Then** 系统自动探测并串联后续所有分卷切片，呈现完整目录树并无缝解压。
3. **Given** 某个中间分卷切片缺失或损坏，**When** 尝试解压，**Then** 明确报错指出缺失的分卷编号（如“缺少分卷 003”），不产生未完成的脏文件。

---

### User Story 3 - 7Z 头部文件名加密与 Touch ID 生物识别凭据库 (7Z Header Encryption & Biometric Touch ID) (Priority: P1)

作为高度注重商业机密与个人隐私的用户，我希望在 7Z 压缩时开启“加密文件名（Encrypt Header / -mhe）”，使得未经授权的人员即使使用十六进制查看器也无法获知包内文件清单；同时我希望使用 macOS Touch ID / Apple Watch 生物认证安全解锁已保存的密码库，无需每次重复敲击主密码。

**Why this priority**: 头部文件名加密与生物识别是高安全要求商务与个人用户的刚需（对标 7-Zip -mhe 与 BetterZip 密码库）。

**Independent Test**: 使用加密文件名创建 7Z 包，未输入密码前验证任何解析器均无法枚举目录树；在打开密码库时触发 Touch ID 认证，验证指纹识别通过后自动解密密码。

**Acceptance Scenarios**:
1. **Given** 用户勾选“加密文件名”并输入密码，**When** 执行 7Z 压缩，**Then** 生成的 7Z 归档中央目录头被 AES-256 加密。
2. **Given** 头部加密的 7Z 归档，**When** 在 TTZip 或第三方工具中打开，**Then** 在用户输入正确密码之前，完全不展示文件列表与大小元数据。
3. **Given** 用户在系统偏好中启用了 Touch ID，**When** 进入密码保险库或自动填充密码，**Then** 弹出系统级 Touch ID 验证弹窗，认证成功后解密凭据。

---

### User Story 4 - 外部应用程序就地编辑与双向热回写 (External Editor Seamless Round-Trip & Hot Patching) (Priority: P2)

作为开发人员或文档编辑者，我希望在 TTZip 归档浏览器中双击任意文本/代码/配置文件时，能够使用系统默认编辑器（如 VSCode、Xcode、TextEdit）直接打开编辑；当我在外部编辑器中保存文件时，TTZip 能够实时侦测到文件变更并自动或确认后将修改原子写回压缩包内，无需手动解压再重新压缩。

**Why this priority**: 就地编辑是 BetterZip 区别于普通解压软件的最核心生产力功能，极大提升修改小配置文件的流转效率。

**Independent Test**: 在 TTZip 中浏览 `app.zip`，双击 `config.json` 用外部编辑器修改保存，验证 TTZip 捕获保存事件并自动更新归档包内条目，重新解压后确认内容为修改后的最新版本。

**Acceptance Scenarios**:
1. **Given** 归档内某个条目，**When** 用户选择“在外部应用中编辑”，**Then** 文件安全提取至沙盒临时区并拉起系统关联应用。
2. **Given** 临时文件被外部编辑器保存修改，**When** 文件系统事件触发，**Then** TTZip 浮现保存确认提示或按预设静默增量替换压缩包内对应条目。
3. **Given** 编辑会话结束且归档完成更新，**When** 用户关闭归档窗口，**Then** 沙盒临时文件被彻底安全清理。

---

### User Story 5 - 灾难自愈 Reed-Solomon 恢复记录与前向纠错 (Disaster Resilience Reed-Solomon Recovery Record) (Priority: P2)

作为进行长期冷备份与数据归档的科研/企业用户，我希望在创建关键归档时能够附加 1% ~ 10% 的 Reed-Solomon 恢复记录（Recovery Record / RR），使得当磁盘发生扇区损坏、传输比特翻转或文件微量截断时，TTZip 能够自动计算校验子并 100% 修复受损数据。

**Why this priority**: 恢复记录是 WinRAR 屹立不倒的核心护城河技术，为重要数据提供硬件介质衰退下的容灾保障。

**Independent Test**: 压缩 50MB 数据并附加 5% 恢复记录，使用脚本在归档中间人为涂黑 1MB 随机坏块，运行 TTZip 修复引擎，验证数据 100% 完美修复并通过 SHA-256 校验。

**Acceptance Scenarios**:
1. **Given** 压缩配置面板，**When** 用户开启“添加恢复记录”并设定比例为 5%，**Then** 压缩引擎在归档尾部追加标准 RS-FEC 校验块元数据。
2. **Given** 包含恢复记录但被破坏了部分扇区的归档，**When** 用户点击“体检与修复”，**Then** 系统定位坏块扇区，利用 RS 纠错码成功重建损坏数据。
3. **Given** 损坏程度超出恢复记录纠错上限（如损坏 20% 而恢复记录仅 5%），**When** 执行修复，**Then** 系统给出清晰诊断报告并尽可能挽救未损坏条目。

---

### User Story 6 - GUI 原生多核算力能效基准仪表盘与系统级服务集成 (GUI MIPS Benchmark & System QuickLook/Finder Integration) (Priority: P3)

作为关注硬件性能的 Apple Silicon 用户，我希望在 GUI 中拥有一个美观实时的 Benchmark 仪表盘（对标 7-Zip Benchmark），能够测试并展示当前 Mac 的压缩/解压 MIPS、吞吐量（GB/s）与能效表现；同时在 macOS 访达中按下空格键即可通过 QuickLook 瞬时预览压缩包内容。

**Why this priority**: 强化专业极客形象与系统深度融合度（对标 7-Zip Benchmark 与 BetterZip QuickLook）。

**Independent Test**: 在 GUI 中启动 Benchmark，实时观察多核负载与 MIPS 输出；在 Finder 中选中 zip 文件按空格键验证 QuickLook 正常加载条目结构。

**Acceptance Scenarios**:
1. **Given** TTZip 算力仪表盘，**When** 用户点击“开始基准测试”，**Then** 实时绘制多核压缩与解压吞吐折线图并给出最终综合 MIPS 评分。
2. **Given** Finder 中的归档文件，**When** 用户按下空格键，**Then** 弹出 QuickLook 预览窗口展示包内目录树结构与文件基本属性。

---

## 3. Edge Cases & Defensive Invariants

1. **智能解压同名冲突防覆盖**：当智能解压的目标文件夹已存在时，系统必须自动附加序号（如 `Folder (1)`）或弹出交互对话框（覆盖 / 跳过 / 保留两者），严禁静默覆盖已有文件。
2. **分卷压缩磁盘空间不足**：在分卷写入过程中若检测到目标卷剩余空间不足，必须立即挂起或安全回滚，清理已写入的不完整当前分卷切片，记录精确错误日志。
3. **外部编辑文件锁定与并发修改**：若用户在外部编辑器尚未关闭时关闭了 TTZip，系统必须提示“仍有外部编辑会话进行中”，防止临时文件孤儿化或丢失修改。
4. **Touch ID 多次认证失败优雅降级**：当连续 3 次指纹认证不通过或硬件不支持时，必须无缝回退至主密码输入框，不阻断正常业务。
5. **恢复记录对第三方工具的透明性**：附加的恢复记录必须存放在归档标准末尾或元数据扩展槽内，确保标准 7-Zip / WinRAR / 系统解压工具在忽略恢复记录时仍能正常解压该文件。

---

## 4. Functional Requirements *(mandatory)*

- **FR-001**: 系统 MUST 实现智能解压启发式算法（Smart Extraction Heuristic），根据包内根级条目数量（单根目录 vs 多根文件）自动决定是否创建外部包裹文件夹。
- **FR-002**: 系统 MUST 提供操作后自动化配置项（Post-Operation Actions），包括“自动移入废纸篓”、“在 Finder 中高亮显示”、“播放完成提示音”与“发送系统通知”。
- **FR-003**: 系统 MUST 支持 7Z、ZIP、TAR 等格式的分卷归档创建，提供 CD(700MB)、DVD(4.7GB)、FAT32(4095MB)、Web(25MB/100MB) 及自定义尺寸选项，生成的切片命名遵循标准规范。
- **FR-004**: 系统 MUST 支持多卷切片的连续读取与自动识别，当用户打开 `.001` 切片时自动加载全卷目录树并支持完整提取。
- **FR-005**: 系统 MUST 在 7Z 格式创建时提供“加密文件名（Encrypt Header / -mhe）”选项，使用 AES-256 对 Central Directory 头部元数据进行完整加密。
- **FR-006**: 系统 MUST 集成 macOS `LocalAuthentication` 框架，在访问密码保险库或自动填充时支持 Touch ID 与 Apple Watch 生物认证。
- **FR-007**: 系统 MUST 提供外部应用就地编辑桥接（External Editor Bridge），将选定条目安全提取至临时沙盒并使用 `NSWorkspace.shared.open` 启动外部关联程序。
- **FR-008**: 系统 MUST 结合 `FSEvents` 或 `DispatchSourceFileSystemObject` 侦听外部临时文件变更，在文件保存时触发增量原子重压缩回写。
- **FR-009**: 系统 MUST 实现 Reed-Solomon (RS-FEC) 纠错码生成器，支持在归档末尾附加 1% ~ 10% 可配置比例的恢复记录（Recovery Record）。
- **FR-010**: 系统 MUST 在归档体检与修复引擎中集成 RS 解码器，在检测到扇区级数据破坏时自动执行数学纠错与原文件重建。
- **FR-011**: 系统 MUST 在 GUI 中提供独立的 Benchmark View 算力仪表盘，支持多线程与不同字典尺寸下的实时吞吐绘制与 MIPS 评分输出。
- **FR-012**: 所有新增算法与数据结构改动 MUST 遵循热路径零内存分配、无锁并发与 Swift 6 Actor 线程安全规范。

---

## 5. Key Entities

- **SmartExtractStrategy**: 智能解压决策实体，包含 `isSingleRootDirectory`, `rootFolderName`, `targetExtractionURL` 与 `conflictPolicy`。
- **SplitVolumeDescriptor**: 分卷描述符，包含 `baseArchiveName`, `volumeSize`, `formatExtension`, `currentVolumeIndex`, `totalVolumes`。
- **ExternalEditSession**: 外部编辑会话，包含 `archiveURL`, `entryPath`, `sandboxTemporaryURL`, `originalMTime`, `watcherSource`, `state`。
- **RecoveryRecordBlock**: 恢复记录数据块，包含 `recoveryPercent`, `parityBlockSize`, `dataShards`, `parityShards`, `eccPayloadOffset`。
- **BenchmarkExecutionState**: 算力基准状态，包含 `threads`, `dictionarySize`, `compressThroughput`, `decompressThroughput`, `mipsScore`。

---

## 6. Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 智能解压对于 100% 的常见归档样本（单目录压缩包与散列多文件压缩包）产生符合预期的解压目录结构，零冗余文件夹生成。
- **SC-002**: 创建的 7Z/ZIP 分卷归档在主流解压工具（Windows 7-Zip, WinRAR, macOS The Unarchiver）中实现 100% 互通解压。
- **SC-003**: 启用“加密文件名”的 7Z 归档在输入密码前，任何第三方工具无法探测出包内任意一个文件名。
- **SC-004**: 外部编辑器修改保存后，TTZip 侦听响应延迟 <= 50ms，回写归档完整性 100%。
- **SC-005**: 附加 5% 恢复记录的 50MB 归档在遭受 <= 1MB 随机坏块注入时，自愈修复成功率达到 100%。
- **SC-006**: GUI Benchmark 测试过程中 `@MainActor` 帧率保持在 60fps，算力 MIPS 偏差与真实硬件能力 <= 2%。

---

## 7. Clarifications Log

### Session 2026-08-18
- Q: 智能解压在遇到 `__MACOSX` 或 `.DS_Store` 等系统隐藏文件时如何判定单根目录？ → A: 自动清洗/忽略 Apple 元数据隐藏文件，仅以用户可见的实体文件与目录判定是否为单一顶层根目录。
- Q: 7Z 头部文件名加密（-mhe）在 GUI 上的用户交互流转？ → A: 打开归档时若检测到 7Z 头部加密标志，立即挂起目录树构建，自动弹出密码验证面板（支持 Touch ID），解锁后方才渲染目录树。
- Q: 外部应用程序就地编辑的沙盒生命周期与安全性？ → A: 文件提取至沙盒隔离区（`NSTemporaryDirectory/TTZip_Edit_<uuid>`），通过 `DispatchSource` 侦听文件保存事件；归档更新后或窗口关闭时强制销毁临时文件。
- Q: 恢复记录 (Reed-Solomon) 与标准解压工具的兼容性？ → A: 恢复记录作为独立元数据附着在归档尾部，第三方标准解压工具直接读取主数据段并正常解压，TTZip 体检修复引擎可自动识别尾部纠错段并实施数学重建。
- Q: 分卷归档的命名与分块规则？ → A: 严格遵循行业通用的 `.7z.001`, `.zip.001`, `.tar.001` 与 `.z01` 命名规范，支持 Windows 7-Zip, WinRAR 及 macOS Keka 跨平台无缝识别。

