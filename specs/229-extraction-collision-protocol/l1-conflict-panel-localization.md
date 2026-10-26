# L1 设计与交接记录：同名文件冲突弹窗本地化与可读性修复

**所属特性**：`229-extraction-collision-protocol`  
**负责单元**：L1（同名文件冲突弹窗展示、本地化与可读性修复）  
**状态**：组件实现及专项验证完成，应用接线、集成编译与 GUI 待验收

---

## 1. 背景与已确认问题

在解压同名冲突交互弹窗 [`ForensicCollisionPanel`](file://../../infra/ttkit/Sources/TTKitUI/Omnibar/ForensicCollisionPanel.swift) 中，存在以下已确认问题：

1. **硬编码英文漏翻**：
   - 弹窗主标题（`File Collision Resolution`）、进度副标题（`Conflict X / Y`）、类型冲突胶囊（`Type Mismatch`）。
   - 左右对比卡片标题（`Incoming Source` / `Existing Destination`）、卡片顶部角标（`NEWER` / `DIR` / `FILE`）、属性行标签（`Path` / `Size` / `Modified` / `Type`）、属性值（`Directory` / `Folder` / `File`）及比较角标（`Larger` / `Latest` / `Mismatch`）。
   - 四个决议按钮标题与说明（`Stop` / `Skip` / `Replace` / `Keep Both` 及其副标题）、底部复选框（`Apply this resolution to all remaining conflicts`）。
   - 全部 VoiceOver 无障碍标签（`accessibilityLabel`）与操作提示（`accessibilityHint`）。
2. **日期、数字与字节大小未跟随应用语言**：
   - `ConflictFileDescriptor.formattedSize` 使用无 `Locale` 的 `NumberFormatter` 与系统默认 `ByteCountFormatter`，目录硬编码为 `"Directory"`，未跟随应用当前语言与字节单位标准（SI / IEC），且对负数等无效字节数无防御展示。
   - `ConflictFileDescriptor.formattedTimestamp` 使用无 `Locale` 的固定格式，对缺失或无效时间戳（如 `epoch <= 0` 或非有限值）无明确提示。
3. **按钮说明截断与可读性不足**：
   - 弹窗固定 `width: 580`，四个操作按钮单行挤在 `HStack` 中，每个按钮净宽仅约 `95pt`，且受 macOS `.buttonStyle(.bordered)` 原生单行 `NSButton` 边框裁剪限制，副标题在英文及长译文下被严重截断。
   - 卡片左列属性标签固定 `width: 52`，在中文或较长译文下易拥挤；未选中卡片边框使用 `Color.white.opacity(0.15)`，在浅色主题下对比度不足。
   - 旧比较标签混用 `NEWER` 与 `Latest`，容易产生“应当保留某一方”的倾向性暗示；“停止”旧文案 `Cancel extraction` 未准确区分“停止后续冲突与解压处理”与已提交阶段边界。

> **范围外已记录问题（本轮不扩大修改范围）**：
> - 侧栏收藏与固定分组标题使用了 `String(localized:)` 而非 `TTI18n`（[`SidebarFavoritesSectionView.swift:48`](file://../apple/Sources/TTZipApp/Views/Sidebar/SidebarFavoritesSectionView.swift#L48) 的 `String(localized: "FAVORITES")`、[`SidebarPinnedSectionView.swift:99`](file://../apple/Sources/TTZipApp/Views/Sidebar/SidebarPinnedSectionView.swift#L99) 的 `String(localized: "PINNED")`），由于主 Bundle 无对应 `.lproj` 词条目录而在中文界面下显示英文。归入侧栏本地化问题清单，不在 L1 修改范围内触碰。

---

## 2. 全部可见文案、提示与 VoiceOver 文案清单

所有词条统一定义在 [`CollisionLocaleKey.swift`](file://Sources/TTZipCore/Localization/CollisionLocaleKey.swift)（命名空间 `collision.*`，可通过 `CollisionLocaleKey` 或 `L10n.Collision` 访问），并通过 [`CollisionPanelLocalization.swift`](file://../apple/Sources/TTZipApp/Views/Localization/CollisionPanelLocalization.swift) 注入 [`ForensicCollisionLocalization.swift`](file://../../infra/ttkit/Sources/TTKitUI/Omnibar/ForensicCollisionLocalization.swift)。

左右卡片明确采用 **“待解压项目（来自归档）”** 与 **“目标已有项目（当前位置）”** 对照表达，避免用户混淆来源与目标。

### 2.1 标题栏与冲突类型词条

| Key (`CollisionLocaleKey`) | Raw Key | 简体中文 (`zh-Hans`) | 繁体中文 (`zh-Hant`) | 英文 (`en` 及其他语言后备) | 用途 |
|---|---|---|---|---|---|
| `.panelTitle` | `collision.panel.title` | `发现同名项目冲突` | `發現同名項目衝突` | `File Collision Resolution` | 弹窗主标题 |
| `.conflictProgressFormat` | `collision.panel.progress_format` | `第 %1$@ 项，共 %2$@ 项冲突 — “%3$@”` | `第 %1$@ 項，共 %2$@ 項衝突 — 「%3$@」` | `Conflict %1$@ of %2$@ — "%3$@"` | 冲突序号与当前文件名 |
| `.sameTypeTitle` | `collision.kind.same_type` | `相同类型` | `相同類型` | `Same Type` | `ConflictKindDisplay.sameType` 显示名 |
| `.typeMismatchBadge` | `collision.kind.type_mismatch` | `类型不同` | `類型不同` | `Type Mismatch` | 顶部警示胶囊与 `ConflictKindDisplay.typeMismatch` |
| `.typeMismatchHelp` | `collision.kind.type_mismatch_help` | `待解压项目与目标已有项目类型不同（例如一方为文件，另一方为文件夹）` | `待解壓項目與目標已有項目類型不同（例如一方為檔案，另一方為資料夾）` | `The incoming item and existing item have different types (file vs. folder)` | 类型冲突悬浮提示 |

### 2.2 左右对比卡片、属性标签与客观比较角标

> **客观比较原则**：角标仅使用“时间较新／较新（`Newer`）”与“体积较大／较大（`Larger`）”描述客观元数据差异，不使用 `Latest` / `Recommended` 等暗示“应当保留某一方”的诱导性词汇。

| Key (`CollisionLocaleKey`) | Raw Key | 简体中文 (`zh-Hans`) | 繁体中文 (`zh-Hant`) | 英文 (`en` 及其他语言后备) | 用途 |
|---|---|---|---|---|---|
| `.incomingCardTitle` | `collision.card.incoming_title` | `待解压项目（来自归档）` | `待解壓項目（來自歸檔）` | `Incoming Item (From Archive)` | 左侧卡片标题 |
| `.existingCardTitle` | `collision.card.existing_title` | `目标已有项目（当前位置）` | `目標已有項目（目前位置）` | `Existing Item (At Destination)` | 右侧卡片标题 |
| `.newerBadge` | `collision.badge.newer` | `时间较新` | `時間較新` | `Newer` | 卡片顶部与修改时间行比较角标（僅当双方时间均有效且一方严格较新时显示） |
| `.largerBadge` | `collision.badge.larger` | `体积较大` | `體積較大` | `Larger` | 大小行比较角标（仅当双方均为普通文件、字节数有效且一方严格较大时显示） |
| `.mismatchBadge` | `collision.badge.mismatch` | `类型冲突` | `類型衝突` | `Mismatch` | 类型行冲突角标 |
| `.directoryBadge` | `collision.badge.directory` | `文件夹` | `資料夾` | `Folder` | 类型冲突时卡片角标（目录） |
| `.fileBadge` | `collision.badge.file` | `文件` | `檔案` | `File` | 类型冲突时卡片角标（文件） |
| `.pathLabel` | `collision.attr.path` | `路径` | `路徑` | `Path` | 属性标签：相对路径 |
| `.sizeLabel` | `collision.attr.size` | `大小` | `大小` | `Size` | 属性标签：字节大小 |
| `.modifiedLabel` | `collision.attr.modified` | `修改时间` | `修改時間` | `Modified` | 属性标签：修改时间 |
| `.typeLabel` | `collision.attr.type` | `类型` | `類型` | `Type` | 属性标签：项目类型 |
| `.directoryTypeValue` | `collision.value.directory` | `文件夹` | `資料夾` | `Folder` | 目录类型值及目录大小栏展示文案 |
| `.fileTypeValue` | `collision.value.file` | `文件` | `檔案` | `File` | 文件类型值 |
| `.unavailableValue` | `collision.value.unavailable` | `未提供` | `未提供` | `Unavailable` | 缺失或无效大小／时间戳的明确占位展示 |
| `.bytesExactFormat` | `collision.value.bytes_exact_format` | `%1$@（%2$@ 字节）` | `%1$@（%2$@ 位元組）` | `%1$@ (%2$@ B)` | 可读大小 + 精确字节数复合格式 |
| `.bytesOnlyFormat` | `collision.value.bytes_only_format` | `%1$@ 字节` | `%1$@ 位元組` | `%1$@ B` | 小于 1 KB 或纯字节展示格式 |

### 2.3 四个操作按钮与底部复选框

| Key (`CollisionLocaleKey`) | Raw Key | 简体中文 (`zh-Hans`) | 繁体中文 (`zh-Hant`) | 英文 (`en` 及其他语言后备) | 用途 |
|---|---|---|---|---|---|
| `.actionStopTitle` | `collision.action.stop.title` | `停止` | `停止` | `Stop` | `.stop` 按钮主标题 |
| `.actionStopSubtitle` | `collision.action.stop.subtitle` | `停止后续解压处理` | `停止後續解壓處理` | `Stop remaining extraction` | `.stop` 按钮副标题（准确说明停止后续处理，不宣称任意提交阶段可回滚） |
| `.actionSkipTitle` | `collision.action.skip.title` | `跳过` | `略過` | `Skip` | `.skip` 按钮主标题 |
| `.actionSkipSubtitle` | `collision.action.skip.subtitle` | `保留目标已有项目` | `保留目標已有項目` | `Keep existing item unchanged` | `.skip` 按钮副标题 |
| `.actionReplaceTitle` | `collision.action.replace.title` | `替换` | `取代` | `Replace` | `.replace` 按钮主标题 |
| `.actionReplaceSubtitle` | `collision.action.replace.subtitle` | `用待解压项目覆盖` | `以待解壓項目覆蓋` | `Overwrite with incoming item` | `.replace` 按钮副标题 |
| `.actionKeepBothTitle` | `collision.action.keep_both.title` | `保留两者` | `保留兩者` | `Keep Both` | `.keepBoth` 按钮主标题 |
| `.actionKeepBothSubtitle` | `collision.action.keep_both.subtitle` | `待解压项目重命名共存` | `待解壓項目重新命名共存` | `Rename incoming to keep both` | `.keepBoth` 按钮副标题 |
| `.applyToAllRemaining` | `collision.control.apply_to_all` | `对其余冲突使用相同操作` | `對其餘衝突使用相同操作` | `Apply this resolution to all remaining conflicts` | 底部批量应用复选框标题 |

### 2.4 VoiceOver 无障碍与辅助提示词条

| Key (`CollisionLocaleKey`) | Raw Key | 简体中文 (`zh-Hans`) | 繁体中文 (`zh-Hant`) | 英文 (`en` 及其他语言后备) | 用途 |
|---|---|---|---|---|---|
| `.dialogAccessibilityLabel` | `collision.a11y.dialog_label` | `同名文件冲突处理对话框` | `同名項目衝突處理對話框` | `File collision resolution dialog` | 弹窗容器 `accessibilityLabel` |
| `.headerSummaryVoiceOverFormat` | `collision.a11y.header_summary_format` | `发现同名项目冲突，第 %1$@ 项，共 %2$@ 项。项目名称：%3$@%4$@。待解压项目路径：%5$@，大小：%6$@，修改时间：%7$@。目标已有项目路径：%8$@，大小：%9$@，修改时间：%10$@。` | `發現同名項目衝突，第 %1$@ 項，共 %2$@ 項。項目名稱：%3$@%4$@。待解壓項目路徑：%5$@，大小：%6$@，修改時間：%7$@。目標已有項目路徑：%8$@，大小：%9$@，修改時間：%10$@。` | `File collision resolution. Conflict %1$@ of %2$@. Item name: %3$@%4$@. Incoming item path: %5$@, size: %6$@, modified: %7$@. Existing item path: %8$@, size: %9$@, modified: %10$@.` | 顶部综合无障碍摘要 |
| `.headerTypeMismatchClause` | `collision.a11y.header_type_mismatch_clause` | `，待解压项目与目标已有项目类型不同` | `，待解壓項目與目標已有項目類型不同` | `, type mismatch between incoming and existing items` | 顶部摘要中的类型不同子句 |
| `.typeMismatchAccessibilityLabel` | `collision.a11y.type_mismatch_label` | `待解压项目与目标已有项目类型不同` | `待解壓項目與目標已有項目類型不同` | `Type mismatch between incoming item and existing item` | 类型不同胶囊 `accessibilityLabel` |
| `.cardAccessibilityFormat` | `collision.a11y.card_format` | `%1$@：%2$@，类型：%3$@，路径：%4$@，大小：%5$@，修改时间：%6$@%7$@` | `%1$@：%2$@，類型：%3$@，路徑：%4$@，大小：%5$@，修改時間：%6$@%7$@` | `%1$@: %2$@, type: %3$@, path: %4$@, size: %5$@, modified: %6$@%7$@` | 左右卡片无障碍摘要 |
| `.actionStopAccessibilityHint` | `collision.a11y.action_stop_hint` | `停止后续冲突与解压处理。快捷键 Escape` | `停止後續衝突與解壓處理。快速鍵 Escape` | `Stop remaining conflict and extraction processing. Press Escape.` | `.stop` 无障碍提示与悬浮说明 |
| `.actionSkipAccessibilityHint` | `collision.a11y.action_skip_hint` | `跳过当前待解压项目，保留目标位置已有项目不变` | `略過目前待解壓項目，保留目標位置已有項目不變` | `Preserve existing item at destination and skip incoming item.` | `.skip` 无障碍提示与悬浮说明 |
| `.actionReplaceAccessibilityHint` | `collision.a11y.action_replace_hint` | `用待解压项目覆盖目标位置的已有项目` | `以待解壓項目覆蓋目標位置的已有項目` | `Overwrite existing item at destination with incoming item.` | `.replace` 无障碍提示与悬浮说明 |
| `.actionKeepBothAccessibilityHint` | `collision.a11y.action_keep_both_hint` | `保留目标已有项目，将待解压项目自动重命名后一并保存。默认操作，快捷键 Return` | `保留目標已有項目，將待解壓項目自動重新命名後一併儲存。預設操作，快速鍵 Return` | `Save incoming item with a numbered name alongside existing item. Default action, press Return.` | `.keepBoth` 无障碍提示与悬浮说明 |
| `.applyToAllAccessibilityHint` | `collision.a11y.apply_to_all_hint` | `勾选后，所选操作将应用于本次解压其余全部同名冲突` | `勾選後，所選操作將套用於本次解壓其餘全部同名衝突` | `Applies the selected action to all remaining conflicts in this extraction.` | 复选框 `accessibilityHint` 与 `.help` |

---

## 3. TTI18n 语言来源及实时切换方式

1. **单一语言状态源**：
   - TTZip 应用通过既存の [`AppLocalizationState.shared`](file://../apple/Sources/TTZipUI/Services/AppLocalizationState.swift)（`@Observable @MainActor`）管理当前语言 `currentLanguage: TTZipCore.AppLanguage` 与字节单位标准 `byteUnitStandard: ByteSizeStandard`，并在初始化和 `setLanguage(_:)` 时自动同步至 `TTZipLocalizationManager.shared.currentLanguage` 与 `TTI18nUI.LocalizationState.shared.setLanguage(...)`。
   - 不新建任何语言设置存储、不新建语言单例、不引入主 Bundle `String(localized:)`。
2. **词条解析与覆盖／后备优先级**：
   - `CollisionLocaleKey.resolve(for:manager:)` 先调用 `TTZipLocalizationManager.shared.string(for: self, language: targetLanguage)` 查询底层 Rust UniFFI 静态词典（`ttzipI18nGetString`）：
     - 若底层词典返回非空且不等于 `rawKey` 的覆盖译文，优先采用词典覆盖值；
     - 若底层词典尚未编译该新增 key，则走 `CollisionLocaleKey.fallbackTranslation(for: targetLanguage)`：
       - `.zhHans` 返回简体中文；
       - `.zhHant` 返回繁体中文；
       - `.en` 返回英文；
       - `.ja`、`.de`、`.fr`、`.es` 按项目既有后备规则显式回退至英文译文（绝不返回 `rawKey` 或空字符串）。
3. **弹窗打开期间实时切换语言的状态保持机制（显式注入方案）**：
   - 采用**显式注入本地化**的接线方案：[`CollisionPanelLocalization.makeLocalization(from:)`](file://../apple/Sources/TTZipApp/Views/Localization/CollisionPanelLocalization.swift) 在 `@MainActor` 视图求值周期内读取 `@Observable` 的 `AppLocalizationState.shared.currentLanguage` 与 `byteUnitStandard`，构造不可变的 `ForensicCollisionLocalization` 值模型显式传入 `ForensicCollisionPanel`。
   - 说明：虽然代码中定义了 `registerFallbackProvider()` 辅助方法，但**当前应用入口并未调用 `registerFallbackProvider()`**（定义函数不代表已注册，且不为绕开 W1 文件所有权另加隐式初始化）。因此旧调用点在未显式传入 `localization` 时仅保证兼容编译，尚未跟随应用语言，必须由 H 合入 §8.2 的显式注入补丁后才在应用内生效。
   - `ForensicCollisionPanel` 内部不包含任何因 `localization` 变化而触发 `onResolve` 或重置 `@Binding var applyToAllRemaining: Bool` 的副作用：切语言仅触发 SwiftUI 文本与格式重绘，当前冲突记录（`sourceFile`、`targetFile`、`conflictIndex`、`totalConflicts`）、复选框勾选状态与 `CheckedContinuation` 待决回调保持原状。

---

## 4. 原始数据与可翻译文案的边界

| 数据字段 | 处理规则 | 是否翻译／改写 |
|---|---|---|
| `descriptor.fileName` | 原样显示（保留大小写、空格、emoji、CJK、罕见符号、组合字符 NFC/NFD） | **绝不翻译或改写** |
| `descriptor.relativePath` | 原样显示归档内／目标相对路径 | **绝不翻译或改写** |
| `descriptor.filePath` | 在悬浮 `.help` 提示中提供完整路径参考 | **绝不翻译或改写** |
| `descriptor.modificationDate` | 按应用 `Locale` 格式化展示；不猜测、不偏移、不修正 ZIP 时间戳（ZIP 时间解析由 R3 负责） | **只做 Locale 格式化，不篡改时间戳值** |
| `descriptor.byteSize` | 按应用 `Locale` 与字节单位标准格式化展示 | **只做 Locale 数值与单位格式化** |
| 卡片标题、属性名、比较标签、按钮文案、复选框、无障碍说明 | 随应用语言通过 `TTI18n` 完整本地化 | **可翻译 UI 文案** |

---

## 5. 日期、数量、字节大小的格式化规则与缺失数据防御

1. **应用 Locale 绑定**：
   - 所有格式化器均显式绑定 `Locale(identifier: currentLanguage.bcp47)`（例如 `zh-Hans`、`zh-Hant`、`en`、`ja`、`de`、`fr`、`es`）。
2. **数量（冲突序号与总数）**：
   - 使用绑定 `locale` 的 `NumberFormatter`（`numberStyle = .decimal`）格式化 `conflictIndex` 与 `totalConflicts`（如 `10,000` 在英文/中文为 `10,000`，德文为 `10.000`）。
   - 若传入非正数（`<= 0`），安全钳位至 `max(1, value)` 显示，避免出现 `0 / 0` 破坏文案可读性。
3. **字节大小（`formatSize`）**：
   - **目录（`isDirectory == true`）**：返回当前语言的目录类型文案（`文件夹` / `資料夾` / `Folder`），不显示无意义的 `0 B`。
   - **缺失或无效大小（`byteSize < 0`）**：返回当前语言的未提供占位文案（`未提供` / `Unavailable`），且不参与“体积较大”角标比较。
   - **有效文件大小（`byteSize >= 0`）**：
     - `< 1000` 字节时直接显示精确字节数（如 `512 字节` / `512 位元組` / `512 B`），避免出现重复的 `512 B (512 B)`。
     - `>= 1000` 字节时显示“可读单位 + 精确字节数”（如 `2.0 KB（2,048 字节）` / `2.0 KB（2,048 位元組）` / `2 KB (2,048 B)`）。在 TTZip 宿主中可读单位部分遵循 `AppLocalizationState.shared.byteUnitStandard`（调用 `ByteSizeFormatter.format`），精确字节数使用绑定应用 `Locale` 的十进制分组 `NumberFormatter`。
4. **修改时间（`formatTimestamp`）**：
   - **缺失或无效时间戳判定**：当 `descriptor.hasValidModificationDate == false`、`!modificationDate.timeIntervalSince1970.isFinite`、`modificationDate.timeIntervalSince1970 <= 0`（对应 `mtime_unix_ns <= 0` 缺失元数据）、`modificationDate == .distantPast` 或 `modificationDate == .distantFuture` 时，判定为缺失或无效时间戳。
   - **缺失或无效展示**：返回当前语言的未提供占位文案（`未提供` / `Unavailable`），且当任一侧时间戳无效时，`isSourceNewer` 与 `isTargetNewer` 均为 `false`（不显示“时间较新”角标）。
   - **有效时间戳展示**：使用绑定应用 `Locale` 的 `DateFormatter`（`dateStyle = .medium`, `timeStyle = .medium`）格式化，原样反映真实时间点。

---

## 6. 四个操作的用户语义与快捷键映射

`ConflictResolutionAction` 的 `case`（`.replace`、`.keepBoth`、`.skip`、`.stop`）、`rawValue`（`"REPLACE"`、`"`KEEP_BOTH`"`、`"`SKIP`"`、`"`STOP`"`）、按钮排列顺序（左起：`停止`、`跳过`、`替换`、`保留两者`）与 `onResolve(action, applyToAllRemaining)` 回调签名保持 100% 不变：

| 枚举 Case | RawValue | 用户语义 | 键盘快捷键 | 视觉层级 (`ButtonProminence`) |
|---|---|---|---|---|
| `.stop` | `"STOP"` | **停止后续处理**：停止当前冲突判决与后续解压流程（在未进入 commit 前不改动目标目录；文案不宣称任意已提交阶段可回滚） | `Escape` (`.cancelAction`) | `.secondaryDestructive`（朱砂红次要警示） |
| `.skip` | `"SKIP"` | **跳过**：不解压当前冲突条目，完整保留目标位置已有项目 | 无 | `.standard`（中性次要） |
| `.replace` | `"REPLACE"` | **替换**：使用归档中的待解压项目覆盖目标位置的已有项目 | 无 | `.destructive`（朱砂红高危警示） |
| `.keepBoth` | `"KEEP_BOTH"` | **保留两者（默认安全操作）**：保留目标已有项目不变，将待解压项目按 `<name> N<.ext>` 自动重命名后共存保存 | `Return` (`.defaultAction`) | `.prominent`（金缮黄主推强调） |

---

## 7. 窄空间、长文件名、长译文的布局规则

为彻底解决截图中按钮副标题被截断、长路径不可辨识及浅色主题边框对比度问题，同时保持 TTKit 第四层光学悬浮面板（Layer 3 Overlay Glass Plate）视觉风格：

1. **面板宽度与响应式容器**：
   - 面板宽度由固定 `580pt` 调整为 `minWidth: 600, idealWidth: 640, maxWidth: 680`，为双卡片与四按钮提供充足横向余量。
2. **四按钮防截断自适应布局（`actionButtonsGrid`）**：
   - 移除会导致 macOS `NSButton` 单行裁剪的原生 `.buttonStyle(.bordered)`，改为符合 TTKit 规范的玻璃描边按钮容器（保留原有的按钮点击态、键盘焦点环、快捷键修饰符与色板语义）。
   - 按钮标题与副标题均开启 `.multilineTextAlignment(.center)` 与 `.fixedSize(horizontal: false, vertical: true)`（副标题支持最多 3 行自然折行，绝不截断关键语义），同时为每个按钮挂载完整 `.help("\(title) — \(subtitle)")` 悬浮提示。
   - 使用 `ViewThatFits(in: .horizontal)`：常规宽度与中英文下优先以 4 列等宽卡片按钮横排展示；当译文超长（如德语／法语长句）或容器变窄导致单行 4 列放不下时，自动切换为 2×2 双列网格布局（第一行：`替换` + `保留两者`；第二行：`停止` + `跳过`，或按原序 2×2 排列），确保任何语言下均不截断按钮说明。
3. **长文件名与长路径展示**：
   - 文件名（`cardFileRow`）：`.lineLimit(2)` + `.truncationMode(.middle)`，保留文件名前缀与扩展名尾部供肉眼辨识，并挂载 `.help(descriptor.fileName)` 悬浮提示显示完整文件名。
   - 相对路径（`attributeRow`）：`.lineLimit(2)` + `.truncationMode(.middle)`，挂载 `.help(descriptor.filePath.isEmpty ? descriptor.relativePath : "\(descriptor.relativePath)\n\(descriptor.filePath)")`，鼠标悬停即可查看完整相对路径与目标绝对路径。
   - 属性标签列宽：由固定 `52pt` 改为 `minWidth: 58, idealWidth: 64, alignment: .leading`，确保中文“修改时间”与多语种属性名不折行、不挤压属性值。
4. **深浅主题光学一致性**：
   - 卡片默认描边由 `Color.white.opacity(0.15)` 改为自适应主题的 `Color.primary.opacity(0.12)`，较新一方描边保持 `TTKitTheme.bambooGreen.opacity(0.60)`；按钮底色与文字对比度在浅色（Light）和深色（Dark）主题下均清晰可读。

---

## 8. 与 W1 的窗口弹窗调用点交接方式

[`MainRootOverlaysView.swift`](file://../apple/Sources/TTZipApp/Views/Main/MainRootOverlaysView.swift) 归 W1 独占修改，L1 **不直接修改该文件**，也不为绕开 W1 文件所有权另加隐式初始化。

### 8.1 编译兼容边界与显式注入要求
1. `ForensicCollisionPanel.init` 新增可选参数 `localization: ForensicCollisionLocalization? = nil`（默认值为 `nil`），使 W1 现有的 [`MainRootOverlaysView.swift`](file://../apple/Sources/TTZipApp/Views/Main/MainRootOverlaysView.swift) 旧调用点保持编译兼容。
2. **重要边界说明**：当前应用入口**没有调用** `CollisionPanelLocalization.registerFallbackProvider()`（仅定义函数不代表已注册）。因此旧调用点虽然兼容编译，但**尚未跟随应用语言**。
3. 正式接线统一采用 **§8.2 显式注入 `localization`** 的方案，待 W1 收口后由 H 合入 `MainRootOverlaysView.swift`。

### 8.2 给 H / W1 的 `MainRootOverlaysView.swift` 显式注入补丁

在 W1 收口后，由 H 在 [`MainRootOverlaysView.swift`](file://../apple/Sources/TTZipApp/Views/Main/MainRootOverlaysView.swift) 中显式注入 `CollisionPanelLocalization.makeLocalization(from: l10n)`：

```diff
--- a/products/ttzip/apple/Sources/TTZipApp/Views/Main/MainRootOverlaysView.swift
+++ b/products/ttzip/apple/Sources/TTZipApp/Views/Main/MainRootOverlaysView.swift
@@ -12,6 +12,7 @@ import TTKitDesign
 @MainActor
 public struct MainRootOverlaysView: View {
     @Environment(AppViewState.self) private var viewModel
+    private var l10n = AppLocalizationState.shared
     @State private var applyCollisionToAll: Bool = false
     
     public init() {}
@@ -113,6 +114,7 @@ public struct MainRootOverlaysView: View {
                     conflictIndex: prompt.conflictIndex,
                     totalConflicts: prompt.totalConflicts,
                     applyToAllRemaining: $applyCollisionToAll,
+                    localization: CollisionPanelLocalization.makeLocalization(from: l10n),
                     onResolve: { action, applyAll in
                         Task { @MainActor in
                             withAnimation(.spring(response: 0.32, dampingFraction: 0.95)) {
```

### 8.3 共享注册文件可选合并说明（供 H 集中合并）
- `products/ttzip/core/BUILD.bazel`（`glob(["Sources/TTZipCore/Localization/**/*.swift"])`）、`products/ttzip/apple/BUILD.bazel`（`glob(["Sources/TTZipApp/Views/**/*.swift"])`）与 `infra/ttkit/BUILD.bazel`（`glob(["Sources/TTKitUI/**/*.swift"])`、`glob(["Tests/TTKitTests/**/*.swift"])`）均已通过通配符自动收录本次新增的 4 个源码／测试文件，**无需修改任何 `BUILD.bazel` 或 `Package.swift`**。
- 若后续 H 希望将 `CollisionLocaleKey` 一并加入 `L10n.allKeyGroups` 反射列表（`products/ttzip/core/Sources/TTZipCore/Localization/LocaleKey.swift:374`）及 `infra/tti18n/contracts/ttzip-catalog.json` 静态生成表，只需在 `L10n.allKeyGroups` 末尾追加 `CollisionLocaleKey.self` 即可；在此之前，`CollisionLocaleKey` 已内置完整译文与 `TTZipLocalizationManager` 扩展重载，可独立工作。

---

## 9. 关联问题登记（超出本轮 L1 修改范围，供后续跟进）

根据用户指令要求，截图与周边排查中发现的非冲突弹窗漏翻问题登记如下，**本轮不扩大修改范围**：

| 序号 | 所属模块 / 文件 | 现象描述 | 建议归属 |
|---|---|---|---|
| 1 | 侧栏分组标题（`SidebarFavoritesSectionView.swift:48` / `SidebarPinnedSectionView.swift:99`） | 截图中侧栏分组标题 `PINNED`、`FAVORITES` 使用 `String(localized:)` 而非 `TTI18n`，未随 `AppLocalizationState` 切换为“已固定／收藏夹” | 侧栏视图模块（非 L1 范围） |
| 2 | ZIP 归档时间戳时区／DOS 纪元解析 | 极少数无扩展时间戳的旧 ZIP 条目可能出现 DOS 默认时间或 `1970-01-01`；L1 在展示层已对 `<= 0` 及非有限值做“未提供”防御，不猜测或篡改真实时间点 | R3（归档时间元数据解析） |

---

## 10. 验证记录、文件哈希与待验事项划分

### 10.1 L1 交付代码与测试文件 SHA-256

| 角色 | 文件路径 | SHA-256 |
|---|---|---|
| TTKit 本地化与格式化模型（新增） | `infra/ttkit/Sources/TTKitUI/Omnibar/ForensicCollisionLocalization.swift` | `f993f0f2ee20e9638519b6854da598bb7e439d08894394edd6c90fa7e3433b6d` |
| TTKit 冲突裁决面板（修改） | `infra/ttkit/Sources/TTKitUI/Omnibar/ForensicCollisionPanel.swift` | `0c507bf04da0730dfc2e1d4a47708093df1a7665702c76db4602168c4d9594fd` |
| TTZip 冲突弹窗词条定义（新增） | `products/ttzip/core/Sources/TTZipCore/Localization/CollisionLocaleKey.swift` | `ac6c49328541651204c750cb4b748cfcf8c14085f66d401cfe477a37e54366ee` |
| TTZip 冲突弹窗本地化适配器（新增） | `products/ttzip/apple/Sources/TTZipApp/Views/Localization/CollisionPanelLocalization.swift` | `3d753b05ef083e9fa790de5a15a1c731f33618fc2eaacf74c02fcc7849790bc9` |
| 冲突弹窗本地化专项测试（新增） | `infra/ttkit/Tests/TTKitTests/ForensicCollisionLocalizationTests.swift` | `3179c808e8590630bb5f3627d3b7d7474fece735fe19cfd216220b5a34ddcbc3` |

### 10.2 已完成组件验证 vs. 应用接线、集成编译与 GUI 待验收项

#### A. 已完成的组件实现与专项验证（L1 独立完成）
1. **专项单元测试（SwiftPM & Bazel）**：
   - `swift test --package-path infra/ttkit --filter ForensicCollisionLocalizationTests`：**7/7 通过**（0 警告）。
   - `bazel test //infra/ttkit:TTKitTests --test_filter=ForensicCollisionLocalizationTests`：**PASSED**。
   - 覆盖项：
     - `zh-Hans`、`zh-Hant`、`en` 三语全部 37 个可见文案、比较标签、按钮说明、复选框、提示与 VoiceOver 完整非空且无原始 key 泄漏；
     - 项目其余 11 种已支持语言（`ja`、`ko`、`de`、`fr`、`es`、`pt-BR`、`ru`、`ar`、`hi`、`it`、`nl`）后备解析无空串、无 `collision.*` 原始 key；
     - 显式传入不同 `ForensicCollisionLocalization` 时，当前冲突项与 `applyToAllRemaining` 勾选状态保持不变，且 `onResolve` 回调零触发；
     - 文件名、路径、Emoji、CJK 括号与特殊字符在双卡片及 VoiceOver 摘要中原样保留；
     - 应用 `Locale` 下的日期、数字、SI/IEC 字节大小格式化，以及缺失时间戳（`epoch <= 0`、`distantPast`、`hasValidModificationDate == false`）和无效字节大小（`< 0`）的“未提供 / Unavailable”防御与角标抑制；
     - 四个操作枚举（`REPLACE`、`KEEP_BOTH`、`SKIP`、`STOP`）与回调语义完整性；
     - 深浅主题、长译文（德语长说明）与超长文件名下的多行自适应渲染（`NSHostingView` 实测无错误）。
2. **向后兼容回归测试**：
   - `swift test --package-path infra/ttkit --filter TTKitUITests`：**12/12 通过**（旧调用签名不传 `localization:` 仍可正常编译并通过既有组件测试）。
3. **代码质量与仓库门禁**：
   - `python3 tools/ci/lint_loc_gate.py`：**PASSED**（全部修改／新增文件均远低于 800 行上限）。
   - `python3 tools/ci/lint_code_quality.py`：**PASSED**（单函数 ≤ 80 行、圈复杂度 ≤ 15，0 违规）。
   - `python3 tools/ci/lint_i18n_gate.py`：**PASSED**（`CollisionLocaleKey` 全词条非空且格式占位符一致）。
   - `python3 tools/ci/normalize_headers.py --check`：**PASSED**（专有版权头合规）。

#### B. 应用接线、集成编译与 GUI 待验收项（待 H 接线与 Q1 复验）
1. **应用显式接线与集成编译（H 负责）**：
   - 在 W1 收口后，由 H 将本文档 §8.2 的显式注入补丁（`localization: CollisionPanelLocalization.makeLocalization(from: l10n)`）合入 [`MainRootOverlaysView.swift`](file://../apple/Sources/TTZipApp/Views/Main/MainRootOverlaysView.swift)，并完成 `TTZipCore` / `TTZipApp` 全量集成编译验证。
2. **GUI 与真实解压冲突端到端复验（Q1 负责）**：
   - 在真实归档解压至已有同名文件目录时触发冲突弹窗，核验简体中文、繁体中文、英文下标题、左右卡片（“待解压项目” vs “目标已有项目”）、属性、比较角标、四个操作按钮说明、复选框与悬浮提示的真实窗口渲染效果；
   - 在弹窗开启状态下通过菜单／设置实时切换语言，确认弹窗文案即时刷新、勾选状态不丢失、解压任务不重入；
   - 在深色与浅色外观、长文件名归档条目下核验按钮副标题多行折行无截断，以及 `Return`（保留两者）、`Escape`（停止）与“对其余冲突使用相同操作”在真实解压流中的端到端行为。

