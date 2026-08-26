# Feature Specification: 079-professional-grade-gap-audit

**Feature Branch**: `079-professional-grade-gap-audit`

**Created**: 2026-08-18

**Status**: Draft

**Input**: User description: "全面审计 ttzip，看看距离专业软件差距在哪"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 归档内即时就地编辑与双向文件同步 (In-Place Archive Modification & Live Sync) (Priority: P1)

用户在 TTZip 归档浏览器 (Archive Explorer) 中双击任意文件（如代码源文件、配置文件、文本或图像），系统自动在用户的默认外部应用程序（如 VS Code、TextEdit、Xcode 等）中打开该文件。当用户在外部编辑器中保存更改（`Cmd+S`）时，TTZip 实时捕获文件系统变更事件，并在归档内部完成就地更新与增量重打包，无需用户手动全量解压、重新压缩和覆盖原归档。同时，用户可直接从 Finder 拖拽文件到归档浏览器中添加条目，或通过键盘 Delete 键就地删除归档内选定条目。

**Why this priority**: 就地增量编辑与外部编辑器联动是顶级专业归档工具（如 BetterZip、WinRAR、7-Zip）最核心的生产力特性。用户无需解压即可修改归档内配置，是区分“解压查看器”与“专业归档工作台”的关键分水岭。

**Independent Test**: 打开包含多个文件的 `.zip` 或 `.7z` 归档，在归档浏览器中双击文本文件并在外部编辑器中修改保存，验证原归档文件内部对应条目的内容与修改时间实时更新，且其余条目 100% 完好无损。

**Acceptance Scenarios**:

1. **Given** 用户在 TTZip 中打开包含 `config.json` 的归档文件，**When** 用户双击该条目，**Then** 系统将其安全提取至临时沙盒并唤起关联外部编辑器，启动 `FileWatcherEngine` 监听文件描述符变更。
2. **Given** 外部编辑器完成写入并触发 `.write` 事件，**When** 用户切换回 TTZip，**Then** TTZip 自动执行轻量级增量打包更新归档，并在界面中呈现同步成功的状态反馈。
3. **Given** 用户从 Finder 拖拽 1 个新文件放入当前归档浏览器的某个子目录中，**When** 用户松开鼠标，**Then** 系统自动将新文件添加到归档对应虚拟路径下并刷新目录树。
4. **Given** 用户选中归档内的 1 个或多个条目并按下 `Delete` 键，**When** 用户确认删除，**Then** 系统在原归档中剔除指定条目并安全重构 Central Directory 或归档 Header。

---

### User Story 2 - macOS 深度原生系统集成 (Native QuickLook & Finder Sync Integration) (Priority: P1)

macOS 用户在 Finder 中选中任意受支持的归档文件（ZIP, 7Z, TAR, RAR, CAB, DMG, ISO 等），按下空格键即可通过原生 Quick Look 快速预览插件即时查看归档内部完整的目录树、文件体积、压缩率与加密状态，无需启动主应用程序。在 Finder 中右键点击文件或归档时，呈现标准的 TTZip 扩展菜单（“极速压缩”、“解压到当前目录”、“解压到同名文件夹”、“去除 Mac 冗余文件压缩”），并支持在系统“打开方式”中设置为各类归档格式的默认处理工具。

**Why this priority**: macOS 原生体验与生态粘性高度依赖 QuickLook 预览和 Finder 上下文菜单。专业 macOS 压缩软件（如 Keka、BetterZip、The Unarchiver）均深度植入系统级扩展，这是达成顶级 Mac 原生体验的基石。

**Independent Test**: 在 macOS Finder 中选中 `.zip` 与 `.7z` 文件按下空格键，验证 QuickLook 预览窗口以 60fps 极速渲染出层次化文件列表；在 Finder 中右键点击文件夹，验证菜单项可直接触发后台解压或打包。

**Acceptance Scenarios**:

1. **Given** 用户在 Finder 中选中一个 5GB 的 `.7z` 归档并按下空格键，**When** QuickLook 插件触发，**Then** 插件在 <= 50ms 内完成头部流式解析，并渲染出深色/浅色自适应的交互式文件树视图。
2. **Given** 用户在 Finder 中右键选中多个文件，**When** 点击“⚡️ TTZip: 快速打包为 7Z”，**Then** 系统在后台拉起压缩任务并在通知中心派发进度与完成通知。
3. **Given** 用户在系统设置中将 TTZip 关联为 `.rar` 格式默认打开方式，**When** 双击任意 `.rar` 文件，**Then** TTZip 自动唤起并载入归档浏览视图。

---

### User Story 3 - 归档无盘内存完整性体检与损坏应急修复 (Archive Integrity Diagnostics & Recovery Console) (Priority: P2)

用户在处理关键备份、大容量归档或可疑损坏文件时，可以使用 TTZip 的“深度体检 (Test Archive)”功能，在完全不写入磁盘的纯内存流式管道中对归档全部数据块执行逐一解码与 CRC32/SHA-256 校验，精准报告发生比特翻转或校验失败的具体文件条目。若归档因网络中断或介质损坏发生截断，用户可通过“应急修复 (Repair Archive)”控制台尝试抢救未损坏的数据块并重构有效归档。

**Why this priority**: 工业级数据安全与归档可靠性（如 WinRAR / 7-Zip 的 Test 功能）是专业用户与系统管理员的核心刚需。快速定位损坏块并最大程度挽回数据是专业软件的必备能力。

**Independent Test**: 对人工注入 1 字节随机错误的 `.zip` 和 `.7z` 测试样本执行“测试归档”，验证测试引擎在 1 秒内精确捕获损坏的文件路径与校验和不匹配，并输出结构化体检报告。

**Acceptance Scenarios**:

1. **Given** 一个包含 10,000 个文件的归档文件，**When** 用户点击工具栏“🛡️ 测试归档完整性”，**Then** 系统以多核并行流式管道解码全部块（零磁盘 I/O），并在进度条结束后输出“全部通过 (100% 校验合格)”或列出错误条目清单。
2. **Given** 一个尾部截断的损坏归档文件，**When** 用户在修复控制台中选择修复，**Then** `ArchiveRepairEngine` 扫描并挽救所有有效前置数据块，生成可正常解压的 `.repaired.zip`。

---

### User Story 4 - 跨平台纯净压缩策略与 Mac 特有元数据精细控制 (Cross-Platform Sanitization & Metadata Control) (Priority: P2)

macOS 用户在创建归档用于发送给 Windows / Linux 接收方时，可一键开启“跨平台纯净模式 (Windows/Linux Clean Archive)”，自动剔除 `.DS_Store`、`__MACOSX/` 资源分支目录、`._*` AppleDouble 隐藏文件以及 macOS 独有的扩展属性（Extended Attributes），避免接收方解压时出现混乱的双重文件与系统残留垃圾。而在进行 macOS 本地备份时，支持“高保真模式”，完整保留 POSIX 权限、软/硬链接、ACL 权限列表以及 Gatekeeper 隔离标记（`com.apple.quarantine`）。

**Why this priority**: Mac 用户打包发送给 Windows 同事时出现 `__MACOSX` 与 `.DS_Store` 垃圾文件是 macOS 归档工具长期以来的头号用户痛点（Keka 的核心卖点之一）。提供智能、自适应的纯净压缩策略是衡量 Mac 压缩工具专业度的关键标尺。

**Independent Test**: 打包包含 macOS 专有图标、标签和 `.DS_Store` 的测试文件夹并启用“跨平台纯净模式”，解压后验证归档内绝无任何 `__MACOSX`、`.DS_Store` 或 `._*` 文件；切换为“macOS 全保真模式”重新打包，验证解压后文件的 POSIX 权限与扩展属性 100% 还原。

**Acceptance Scenarios**:

1. **Given** 用户在压缩设置中勾选“排除 macOS 专有系统文件 (.DS_Store, __MACOSX)”，**When** 执行打包，**Then** 压缩引擎在目录扫描阶段过滤全部系统残留，生成的 ZIP/7Z 在 Windows 资源管理器中打开干净清爽。
2. **Given** 用户解压包含 `com.apple.quarantine` 属性的外部下载归档，**When** 执行解压，**Then** 系统安全遵循 macOS 安全沙盒策略正确为解压产物继承或设置隔离标记。

---

### User Story 5 - 全局后台多任务队列与企业级批处理管理 (Global Operations Queue & Task Hub) (Priority: P3)

用户在连续解压或压缩多个大型归档时，TTZip 提供独立的“任务管理中枢 (Operations Queue & Task Hub)”，展示所有并发/排队任务的实时瞬时吞吐速率（MB/s）、剩余时间估算（ETA）、已处理字节数与当前正在处理的单文件。用户可对任务进行暂停、继续、调整执行优先级或取消，Dock 图标实时显示全局综合进度环与未完成任务徽标，任务全部完成时自动触发系统原生通知。

**Why this priority**: 批量归档与重型解压任务的队列管控是专业工具（如 Bandizip 专业版、Keka 队列管理器）的标配能力，能够有效防止多任务抢占 I/O 导致系统卡顿，提升重度用户体验。

**Independent Test**: 同时将 10 个大型归档文件拖入 TTZip 触发批量解压，验证任务管理器按照设定的最大并发数（如 2 个并发）有序调度，支持单任务暂停与取消，Dock 图标精确反映整体进度。

**Acceptance Scenarios**:

1. **Given** 用户连续发起 5 个压缩与 3 个解压任务，**When** 任务中枢接管，**Then** 系统以卡片列表形式展示每个任务的动态进度条、实时 MB/s 吞吐与当前文件名。
2. **Given** 某个任务正在占用大量磁盘带宽，**When** 用户点击该任务的“暂停”按钮，**Then** 引擎立即挂起该任务的工作线程释放 I/O 资源，点击“继续”无缝恢复。
3. **Given** 所有排队任务顺利完成，**When** 应用程序处于后台，**Then** 系统派发 macOS 通知中心通知，Dock 徽标自动清除。

---

### Edge Cases

- **外部编辑器编辑中的临时文件并发冲突**：若用户在外部编辑器中未保存直接关闭归档浏览器窗口，系统检测未提交的修改并弹出确认对话框（“丢弃更改”或“保存并更新归档”），防止用户编辑内容意外丢失。
- **加密归档多密码批量解压**：当用户批量解压多个受保护归档时，系统优先遍历 `PasswordVaultManager` 中的已知密码库进行无感预检，仅对未能匹配的归档弹出统一密码输入框，避免连续弹窗阻塞用户流程。
- **只读或只写介质上的就地修改**：若归档文件所在卷处于只读状态（如挂载的只读 DMG 或无写权限目录），在用户尝试就地修改或删除条目时，系统明确提示“归档处于只读介质，请先导出副本”，并提供“另存为新归档”路径。

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: 系统必须支持在归档浏览器中双击条目将其提取到沙盒临时目录，并通过 `NSWorkspace.shared.open` 唤起系统关联外部程序。
- **FR-002**: `FileWatcherEngine` 必须在外部程序修改并保存文件后 500ms 内捕获文件系统变更，并调用归档引擎执行增量或原子替换重打包。
- **FR-003**: 归档浏览器必须支持从 Finder 拖拽外部文件/文件夹并将其添加至当前浏览的归档子目录中。
- **FR-004**: 归档浏览器必须支持键盘 `Delete` / `Backspace` 快捷键，对选中的一个或多个条目执行就地删除操作。
- **FR-005**: QuickLook 预览插件必须支持全部 16 种归档格式，在 <= 50ms 内完成头部解析并在 Finder 预览面板中渲染富文本/HTML5 文件树。
- **FR-006**: 系统必须提供 Finder Sync / Context Menu 扩展集成，支持“快速压缩为 ZIP/7Z”、“解压到当前目录”、“解压到子文件夹”等标准上下文动作。
- **FR-007**: 系统必须提供独立的“测试归档完整性 (Test Archive)”引擎与 UI 报告面板，在内存中流式解码并校验所有条目的 CRC32/SHA-256。
- **FR-008**: 压缩设置中必须提供“跨平台纯净归档”选项，支持自动过滤 `.DS_Store`、`__MACOSX`、`._*` 及 Mac 专有扩展属性。
- **FR-009**: 压缩设置中必须提供“macOS 全保真归档”选项，严格保留 POSIX 权限、扩展属性与软硬链接。
- **FR-010**: 系统必须提供全局任务管理器 (Operations Queue)，支持并发控制（最大并发数 1~8 可调）、任务暂停/继续/取消，以及 Dock 进度和系统通知派发。
- **FR-011**: 归档浏览器必须支持选择性部分解压，支持选中条目通过拖拽直接导出到 Finder 目标路径。

### Key Entities

- **InPlaceEditSession**: 记录当前处于外部编辑状态的临时文件路径、归档路径、归档内相对路径、原始修改时间与监听 DispatchSource。
- **ArchiveIntegrityReport**: 包含体检归档总条目数、总字节数、校验通过数、失败条目列表（包含错误类型、预期校验和与实际校验和）、耗时与整体状态。
- **OperationTask**: 封装单个压缩/解压/测试/修复作业的唯一 ID、作业类型、源路径、目标路径、进度、实时速率、暂停状态与错误信息。
- **SanitizationProfile**: 归档清洗策略配置实体（包含排除规则集合：`.DS_Store`、`__MACOSX`、扩展属性、POSIX 权限掩码等）。

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 归档内就地文件编辑（双击 -> 外部修改保存 -> 归档更新闭环）在 1 秒内完成无感保存与界面刷新。
- **SC-002**: Finder QuickLook 预览在按下空格键后 <= 50ms 内呈现首屏文件树结构。
- **SC-003**: 归档完整性体检（Test Archive）处理吞吐达到 >= 5000 MB/s（内存纯解码校验，不产生磁盘写入开销）。
- **SC-004**: 跨平台纯净压缩生成的 ZIP/7Z 在 Windows 系统中解压后，0 出现 `__MACOSX` 或 `.DS_Store` 干扰文件。
- **SC-005**: 批量任务管理支持同时排队 >= 100 个归档作业，系统 UI 维持 60fps 流畅无卡顿。

## Assumptions

- 用户的操作系统为 macOS 14.0 (Sonoma) 或更高版本。
- 外部编辑器遵循标准 POSIX 文件系统写入原子语义或常规写事件（`.write` / `.extend` / `.attrib`）。
- QuickLook 插件在 App 沙盒与系统 QuickLook 扩展框架内运行。
- 在 MAS 沙盒构建模式下，外部应用唤起与文件监听遵循 macOS App Sandbox Security-Scoped Bookmark 规范。
