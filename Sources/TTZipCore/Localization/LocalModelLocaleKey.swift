// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

/// Strongly-typed localization keys for the Local Models Settings page and on-demand semantic model lifecycle.
/// Integrates with `TTZipLocalizationManager` (`TTI18n`) and provides built-in `zh-Hans`, `zh-Hant`,
/// and `en` translations with deterministic English fallback for other supported languages.
public enum LocalModelLocaleKey: String, LocaleKeyProtocol, CaseIterable, Sendable {
    // MARK: - 1. Tab & Section Headers
    case tabTitle = "settings.local_models.tab_title"
    case sectionTitle = "settings.local_models.section_title"
    case sectionSubtitle = "settings.local_models.section_subtitle"
    case capabilityHeader = "settings.local_models.capability_header"
    case metadataHeader = "settings.local_models.metadata_header"

    // MARK: - 2. Purpose & Search Capability Notices
    case purposeDescription = "settings.local_models.purpose_description"
    case purposeValue = "settings.local_models.purpose_value"
    case basicSearchFallbackNotice = "settings.local_models.basic_search_fallback_notice"
    case capabilitySemanticAndLexical = "settings.local_models.capability_semantic_and_lexical"
    case capabilityBasicLexicalOnly = "settings.local_models.capability_basic_lexical_only"

    // MARK: - 3. Field Labels & Values
    case modelNameLabel = "settings.local_models.label.model_name"
    case purposeLabel = "settings.local_models.label.purpose"
    case downloadSizeLabel = "settings.local_models.label.download_size"
    case diskUsageLabel = "settings.local_models.label.disk_usage"
    case statusLabel = "settings.local_models.label.status"
    case diskUsageZeroValue = "settings.local_models.value.disk_usage_zero"

    // MARK: - 4. Lifecycle Status Badges
    case statusNotInstalled = "settings.local_models.status.not_installed"
    case statusDownloading = "settings.local_models.status.downloading"
    case statusVerifying = "settings.local_models.status.verifying"
    case statusAvailable = "settings.local_models.status.available"
    case statusFailed = "settings.local_models.status.failed"

    // MARK: - 5. Action Buttons & Progress Feedback
    case actionDownload = "settings.local_models.action.download"
    case actionCancel = "settings.local_models.action.cancel"
    case actionRetry = "settings.local_models.action.retry"
    case actionDelete = "settings.local_models.action.delete"
    case verifyingProgressNotice = "settings.local_models.progress.verifying_notice"
    case downloadingProgressFormat = "settings.local_models.progress.downloading_format"
    case failureBannerTitle = "settings.local_models.error.banner_title"

    /// Resolves the localized string for the current active language.
    public var localized: String {
        resolve()
    }

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

    private static let translations: [LocalModelLocaleKey: TranslationTriple] = [
        .tabTitle: TranslationTriple(
            "Local Models",
            "本地模型",
            "本地模型"
        ),
        .sectionTitle: TranslationTriple(
            "Local Semantic Search Model",
            "本地 AI 语义搜索模型",
            "本地 AI 語義搜尋模型"
        ),
        .sectionSubtitle: TranslationTriple(
            "Downloaded on demand and stored locally on device without bloating the main application bundle.",
            "按需下载并保存在本地设备中，不占用主安装包体积。",
            "按需下載並儲存在本地裝置中，不佔用主安裝包體積。"
        ),
        .capabilityHeader: TranslationTriple(
            "Search Capability Overview",
            "搜索能力概览",
            "搜尋能力概覽"
        ),
        .metadataHeader: TranslationTriple(
            "Model Specifications & Storage",
            "模型信息与存储状态",
            "模型資訊與儲存狀態"
        ),
        .purposeDescription: TranslationTriple(
            "Enables natural-language and cross-lingual semantic search across archives and filenames completely offline on your Mac.",
            "在本地离线提供跨归档与文件名的自然语言理解及跨语种语义搜索能力，所有计算均在本机完成。",
            "在本地離線提供跨歸檔與檔名的自然語言理解及跨語種語義搜尋能力，所有計算均在本機完成。"
        ),
        .purposeValue: TranslationTriple(
            "Offline natural-language & cross-lingual semantic search across archives and files",
            "离线归档与文件自然语言及跨语种语义检索",
            "離線歸檔與檔案自然語言及跨語種語義檢索"
        ),
        .basicSearchFallbackNotice: TranslationTriple(
            "Basic filename and keyword search remains fully functional even without downloading the semantic model.",
            "未安装本地语义模型时，基础文件名与关键词词法搜索仍可正常完整使用。",
            "未安裝本地語義模型時，基礎檔名與關鍵字詞法搜尋仍可正常完整使用。"
        ),
        .capabilitySemanticAndLexical: TranslationTriple(
            "Local Semantic Search & Basic Lexical Search Enabled",
            "已启用本地语义搜索与基础词法搜索",
            "已啟用本地語義搜尋與基礎詞法搜尋"
        ),
        .capabilityBasicLexicalOnly: TranslationTriple(
            "Using Basic Lexical Search (Semantic Expansion Inactive)",
            "当前使用基础词法搜索（未启用语义扩展）",
            "目前使用基礎詞法搜尋（未啟用語義擴充）"
        ),
        .modelNameLabel: TranslationTriple(
            "Model Name",
            "模型名称",
            "模型名稱"
        ),
        .purposeLabel: TranslationTriple(
            "Purpose",
            "功能用途",
            "功能用途"
        ),
        .downloadSizeLabel: TranslationTriple(
            "Download Size",
            "下载大小",
            "下載大小"
        ),
        .diskUsageLabel: TranslationTriple(
            "Disk Usage",
            "磁盘占用",
            "磁碟佔用"
        ),
        .statusLabel: TranslationTriple(
            "Status",
            "当前状态",
            "目前狀態"
        ),
        .diskUsageZeroValue: TranslationTriple(
            "0 MB (Not Occupied)",
            "0 MB（未占用）",
            "0 MB（未佔用）"
        ),
        .statusNotInstalled: TranslationTriple(
            "Not Installed",
            "未安装",
            "未安裝"
        ),
        .statusDownloading: TranslationTriple(
            "Downloading",
            "下载中",
            "下載中"
        ),
        .statusVerifying: TranslationTriple(
            "Verifying & Loading",
            "校验与加载中",
            "校驗與載入中"
        ),
        .statusAvailable: TranslationTriple(
            "Available",
            "可用",
            "可用"
        ),
        .statusFailed: TranslationTriple(
            "Unavailable / Failed",
            "不可用 / 失败",
            "不可用 / 失敗"
        ),
        .actionDownload: TranslationTriple(
            "Download Model",
            "下载模型",
            "下載模型"
        ),
        .actionCancel: TranslationTriple(
            "Cancel Download",
            "取消下载",
            "取消下載"
        ),
        .actionRetry: TranslationTriple(
            "Retry Download",
            "重试下载",
            "重試下載"
        ),
        .actionDelete: TranslationTriple(
            "Delete Model",
            "删除模型",
            "刪除模型"
        ),
        .verifyingProgressNotice: TranslationTriple(
            "Verifying model integrity and initializing local inference engine...",
            "正在校验模型完整性并加载本地推理引擎…",
            "正在校驗模型完整性並載入本地推理引擎…"
        ),
        .downloadingProgressFormat: TranslationTriple(
            "Downloading model (%1$@)",
            "正在下载模型（%1$@）",
            "正在下載模型（%1$@）"
        ),
        .failureBannerTitle: TranslationTriple(
            "Model Setup Issue",
            "模型准备未完成",
            "模型準備未完成"
        )
    ]

    // MARK: - Convenience Static String Accessors

    public static var sectionTitleString: String { sectionTitle.resolve() }
    public static var sectionSubtitleString: String { sectionSubtitle.resolve() }
    public static var purposeDescriptionString: String { purposeDescription.resolve() }
    public static var purposeValueString: String { purposeValue.resolve() }
    public static var basicSearchFallbackNoticeString: String { basicSearchFallbackNotice.resolve() }
    public static var capabilitySemanticAndLexicalString: String { capabilitySemanticAndLexical.resolve() }
    public static var capabilityBasicLexicalOnlyString: String { capabilityBasicLexicalOnly.resolve() }
}

public extension L10n {
    typealias LocalModel = LocalModelLocaleKey
}

public extension TTZipLocalizationManager {
    /// Resolves a `LocalModelLocaleKey` in the target or active language with catalog override + built-in fallback.
    func string(for key: LocalModelLocaleKey, language: AppLanguage? = nil) -> String {
        key.resolve(for: language, manager: self)
    }

    /// Formats a `LocalModelLocaleKey` template using the target or active language's `Locale`.
    func format(_ key: LocalModelLocaleKey, language: AppLanguage? = nil, _ args: CVarArg...) -> String {
        let targetLanguage = language ?? currentLanguage
        let template = key.resolve(for: targetLanguage, manager: self)
        return String(format: template, locale: Locale(identifier: targetLanguage.bcp47), arguments: args)
    }
}
