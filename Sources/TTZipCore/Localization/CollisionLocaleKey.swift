// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

/// Strongly-typed localization keys for the extraction file collision resolution dialog (Feature 229).
/// Integrates with `TTZipLocalizationManager` (`TTI18n`) and provides built-in `zh-Hans`, `zh-Hant`,
/// and `en` translations with deterministic English fallback for other supported languages.
public enum CollisionLocaleKey: String, LocaleKeyProtocol, CaseIterable, Sendable {
    // MARK: - 1. Header & Conflict Classification
    case panelTitle = "collision.panel.title"
    case conflictProgressFormat = "collision.panel.progress_format"
    case sameTypeTitle = "collision.kind.same_type"
    case typeMismatchBadge = "collision.kind.type_mismatch"
    case typeMismatchHelp = "collision.kind.type_mismatch_help"

    // MARK: - 2. Comparison Cards & Objective Attribute Badges
    case incomingCardTitle = "collision.card.incoming_title"
    case existingCardTitle = "collision.card.existing_title"
    case newerBadge = "collision.badge.newer"
    case largerBadge = "collision.badge.larger"
    case mismatchBadge = "collision.badge.mismatch"
    case directoryBadge = "collision.badge.directory"
    case fileBadge = "collision.badge.file"

    // MARK: - 3. Attribute Labels & Metadata Values
    case pathLabel = "collision.attr.path"
    case sizeLabel = "collision.attr.size"
    case modifiedLabel = "collision.attr.modified"
    case typeLabel = "collision.attr.type"
    case directoryTypeValue = "collision.value.directory"
    case fileTypeValue = "collision.value.file"
    case unavailableValue = "collision.value.unavailable"
    case bytesExactFormat = "collision.value.bytes_exact_format"
    case bytesOnlyFormat = "collision.value.bytes_only_format"

    // MARK: - 4. Four Resolution Actions & Batch Control
    case actionStopTitle = "collision.action.stop.title"
    case actionStopSubtitle = "collision.action.stop.subtitle"
    case actionSkipTitle = "collision.action.skip.title"
    case actionSkipSubtitle = "collision.action.skip.subtitle"
    case actionReplaceTitle = "collision.action.replace.title"
    case actionReplaceSubtitle = "collision.action.replace.subtitle"
    case actionKeepBothTitle = "collision.action.keep_both.title"
    case actionKeepBothSubtitle = "collision.action.keep_both.subtitle"
    case applyToAllRemaining = "collision.control.apply_to_all"

    // MARK: - 5. VoiceOver Accessibility Labels & Hints
    case dialogAccessibilityLabel = "collision.a11y.dialog_label"
    case headerSummaryVoiceOverFormat = "collision.a11y.header_summary_format"
    case headerTypeMismatchClause = "collision.a11y.header_type_mismatch_clause"
    case typeMismatchAccessibilityLabel = "collision.a11y.type_mismatch_label"
    case cardAccessibilityFormat = "collision.a11y.card_format"
    case actionStopAccessibilityHint = "collision.a11y.action_stop_hint"
    case actionSkipAccessibilityHint = "collision.a11y.action_skip_hint"
    case actionReplaceAccessibilityHint = "collision.a11y.action_replace_hint"
    case actionKeepBothAccessibilityHint = "collision.a11y.action_keep_both_hint"
    case applyToAllAccessibilityHint = "collision.a11y.apply_to_all_hint"

    /// Built-in fallback translation covering Simplified Chinese, Traditional Chinese, and English.
    /// Other supported languages (`ja`, `de`, `fr`, `es`) follow the project fallback rule to English.
    public func fallbackTranslation(for language: AppLanguage) -> String {
        guard let entry = Self.translations[self] else {
            return rawKey
        }
        switch language {
        case .zhHans:
            return entry.zhHans
        case .zhHant:
            return entry.zhHant
        case .en, .ja, .de, .fr, .es:
            return entry.en
        }
    }

    /// Resolves the localized string through `TTZipLocalizationManager`, honoring any compiled catalog
    /// override before falling back to the built-in translation table.
    public func resolve(
        for language: AppLanguage? = nil,
        manager: TTZipLocalizationManager = .shared
    ) -> String {
        let targetLanguage = language ?? manager.currentLanguage
        let catalogCandidate = manager.string(for: self as any LocaleKeyProtocol, language: targetLanguage)
        if !catalogCandidate.isEmpty && catalogCandidate != rawKey {
            return catalogCandidate
        }
        return fallbackTranslation(for: targetLanguage)
    }

    private struct TranslationTriple: Sendable {
        let en: String
        let zhHans: String
        let zhHant: String

        init(_ en: String, _ zhHans: String, _ zhHant: String) {
            self.en = en
            self.zhHans = zhHans
            self.zhHant = zhHant
        }
    }

    private static let translations: [CollisionLocaleKey: TranslationTriple] = [
        .panelTitle: TranslationTriple(
            "File Collision Resolution",
            "发现同名项目冲突",
            "發現同名項目衝突"
        ),
        .conflictProgressFormat: TranslationTriple(
            "Conflict %1$@ of %2$@ — \"%3$@\"",
            "第 %1$@ 项，共 %2$@ 项冲突 — “%3$@”",
            "第 %1$@ 項，共 %2$@ 項衝突 — 「%3$@」"
        ),
        .sameTypeTitle: TranslationTriple(
            "Same Type",
            "相同类型",
            "相同類型"
        ),
        .typeMismatchBadge: TranslationTriple(
            "Type Mismatch",
            "类型不同",
            "類型不同"
        ),
        .typeMismatchHelp: TranslationTriple(
            "The incoming item and existing item have different types (file vs. folder)",
            "待解压项目与目标已有项目类型不同（例如一方为文件，另一方为文件夹）",
            "待解壓項目與目標已有項目類型不同（例如一方為檔案，另一方為資料夾）"
        ),
        .incomingCardTitle: TranslationTriple(
            "Incoming Item (From Archive)",
            "待解压项目（来自归档）",
            "待解壓項目（來自歸檔）"
        ),
        .existingCardTitle: TranslationTriple(
            "Existing Item (At Destination)",
            "目标已有项目（当前位置）",
            "目標已有項目（目前位置）"
        ),
        .newerBadge: TranslationTriple(
            "Newer",
            "时间较新",
            "時間較新"
        ),
        .largerBadge: TranslationTriple(
            "Larger",
            "体积较大",
            "體積較大"
        ),
        .mismatchBadge: TranslationTriple(
            "Mismatch",
            "类型冲突",
            "類型衝突"
        ),
        .directoryBadge: TranslationTriple(
            "Folder",
            "文件夹",
            "資料夾"
        ),
        .fileBadge: TranslationTriple(
            "File",
            "文件",
            "檔案"
        ),
        .pathLabel: TranslationTriple(
            "Path",
            "路径",
            "路徑"
        ),
        .sizeLabel: TranslationTriple(
            "Size",
            "大小",
            "大小"
        ),
        .modifiedLabel: TranslationTriple(
            "Modified",
            "修改时间",
            "修改時間"
        ),
        .typeLabel: TranslationTriple(
            "Type",
            "类型",
            "類型"
        ),
        .directoryTypeValue: TranslationTriple(
            "Folder",
            "文件夹",
            "資料夾"
        ),
        .fileTypeValue: TranslationTriple(
            "File",
            "文件",
            "檔案"
        ),
        .unavailableValue: TranslationTriple(
            "Unavailable",
            "未提供",
            "未提供"
        ),
        .bytesExactFormat: TranslationTriple(
            "%1$@ (%2$@ B)",
            "%1$@（%2$@ 字节）",
            "%1$@（%2$@ 位元組）"
        ),
        .bytesOnlyFormat: TranslationTriple(
            "%1$@ B",
            "%1$@ 字节",
            "%1$@ 位元組"
        ),
        .actionStopTitle: TranslationTriple(
            "Stop",
            "停止",
            "停止"
        ),
        .actionStopSubtitle: TranslationTriple(
            "Stop remaining extraction",
            "停止后续解压处理",
            "停止後續解壓處理"
        ),
        .actionSkipTitle: TranslationTriple(
            "Skip",
            "跳过",
            "略過"
        ),
        .actionSkipSubtitle: TranslationTriple(
            "Keep existing item unchanged",
            "保留目标已有项目",
            "保留目標已有項目"
        ),
        .actionReplaceTitle: TranslationTriple(
            "Replace",
            "替换",
            "取代"
        ),
        .actionReplaceSubtitle: TranslationTriple(
            "Overwrite with incoming item",
            "用待解压项目覆盖",
            "以待解壓項目覆蓋"
        ),
        .actionKeepBothTitle: TranslationTriple(
            "Keep Both",
            "保留两者",
            "保留兩者"
        ),
        .actionKeepBothSubtitle: TranslationTriple(
            "Rename incoming to keep both",
            "待解压项目重命名共存",
            "待解壓項目重新命名共存"
        ),
        .applyToAllRemaining: TranslationTriple(
            "Apply this resolution to all remaining conflicts",
            "对其余冲突使用相同操作",
            "對其餘衝突使用相同操作"
        ),
        .dialogAccessibilityLabel: TranslationTriple(
            "File collision resolution dialog",
            "同名文件冲突处理对话框",
            "同名項目衝突處理對話框"
        ),
        .headerSummaryVoiceOverFormat: TranslationTriple(
            "File collision resolution. Conflict %1$@ of %2$@. Item name: %3$@%4$@. Incoming item path: %5$@, size: %6$@, modified: %7$@. Existing item path: %8$@, size: %9$@, modified: %10$@.",
            "发现同名项目冲突，第 %1$@ 项，共 %2$@ 项。项目名称：%3$@%4$@。待解压项目路径：%5$@，大小：%6$@，修改时间：%7$@。目标已有项目路径：%8$@，大小：%9$@，修改时间：%10$@。",
            "發現同名項目衝突，第 %1$@ 項，共 %2$@ 項。項目名稱：%3$@%4$@。待解壓項目路徑：%5$@，大小：%6$@，修改時間：%7$@。目標已有項目路徑：%8$@，大小：%9$@，修改時間：%10$@。"
        ),
        .headerTypeMismatchClause: TranslationTriple(
            ", type mismatch between incoming and existing items",
            "，待解压项目与目标已有项目类型不同",
            "，待解壓項目與目標已有項目類型不同"
        ),
        .typeMismatchAccessibilityLabel: TranslationTriple(
            "Type mismatch between incoming item and existing item",
            "待解压项目与目标已有项目类型不同",
            "待解壓項目與目標已有項目類型不同"
        ),
        .cardAccessibilityFormat: TranslationTriple(
            "%1$@: %2$@, type: %3$@, path: %4$@, size: %5$@, modified: %6$@%7$@",
            "%1$@：%2$@，类型：%3$@，路径：%4$@，大小：%5$@，修改时间：%6$@%7$@",
            "%1$@：%2$@，類型：%3$@，路徑：%4$@，大小：%5$@，修改時間：%6$@%7$@"
        ),
        .actionStopAccessibilityHint: TranslationTriple(
            "Stop remaining conflict and extraction processing. Press Escape.",
            "停止后续冲突与解压处理。快捷键 Escape",
            "停止後續衝突與解壓處理。快速鍵 Escape"
        ),
        .actionSkipAccessibilityHint: TranslationTriple(
            "Preserve existing item at destination and skip incoming item.",
            "跳过当前待解压项目，保留目标位置已有项目不变",
            "略過目前待解壓項目，保留目標位置已有項目不變"
        ),
        .actionReplaceAccessibilityHint: TranslationTriple(
            "Overwrite existing item at destination with incoming item.",
            "用待解压项目覆盖目标位置的已有项目",
            "以待解壓項目覆蓋目標位置的已有項目"
        ),
        .actionKeepBothAccessibilityHint: TranslationTriple(
            "Save incoming item with a numbered name alongside existing item. Default action, press Return.",
            "保留目标已有项目，将待解压项目自动重命名后一并保存。默认操作，快捷键 Return",
            "保留目標已有項目，將待解壓項目自動重新命名後一併儲存。預設操作，快速鍵 Return"
        ),
        .applyToAllAccessibilityHint: TranslationTriple(
            "Applies the selected action to all remaining conflicts in this extraction.",
            "勾选后，所选操作将应用于本次解压其余全部同名冲突",
            "勾選後，所選操作將套用於本次解壓其餘全部同名衝突"
        )
    ]
}

public extension L10n {
    typealias Collision = CollisionLocaleKey
}

public extension TTZipLocalizationManager {
    /// Resolves a `CollisionLocaleKey` in the target or active language with catalog override + built-in fallback.
    func string(for key: CollisionLocaleKey, language: AppLanguage? = nil) -> String {
        key.resolve(for: language, manager: self)
    }

    /// Formats a `CollisionLocaleKey` template using the target or active language's `Locale`.
    func format(_ key: CollisionLocaleKey, language: AppLanguage? = nil, _ args: CVarArg...) -> String {
        let targetLanguage = language ?? currentLanguage
        let template = key.resolve(for: targetLanguage, manager: self)
        return String(format: template, locale: Locale(identifier: targetLanguage.bcp47), arguments: args)
    }
}
