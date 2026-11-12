// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

/// Centralized thread-safe localization manager bridging to Rust UniFFI localization engine.
public final class TTZipLocalizationManager: @unchecked Sendable {
    public static let shared = TTZipLocalizationManager()
    
    private let lock = NSLock()
    private var _currentLanguage: AppLanguage
    
    /// Current active application and CLI interaction language.
    public var currentLanguage: AppLanguage {
        get {
            lock.lock()
            defer { lock.unlock() }
            return _currentLanguage
        }
        set {
            lock.lock()
            _currentLanguage = newValue
            lock.unlock()
        }
    }
    
    private init() {
        // 1. Inspect shared AppGroup preferences store
        if let saved = TTZipPreferencesStore.getSavedLanguage() {
            self._currentLanguage = saved
            return
        }
        
        // 2. Inspect POSIX environment variables (LC_ALL / LANG)
        let env = ProcessInfo.processInfo.environment
        if let lcAll = env["LC_ALL"], let parsed = AppLanguage.from(identifier: lcAll) {
            self._currentLanguage = parsed
            return
        }
        if let lang = env["LANG"], let parsed = AppLanguage.from(identifier: lang) {
            self._currentLanguage = parsed
            return
        }
        
        // 3. Query system preferred languages
        if let preferred = Locale.preferredLanguages.first, let parsed = AppLanguage.from(identifier: preferred) {
            self._currentLanguage = parsed
            return
        }
        
        // 4. Fallback to English (en)
        self._currentLanguage = .en
    }
    
    /// Resolves localized string for a key in the target or current active language.
    public func string(for key: any LocaleKeyProtocol, language: AppLanguage? = nil) -> String {
        let targetLanguage = language ?? currentLanguage
        let rawKey = key.rawKey
        let res = ttzipI18nGetString(key: rawKey, lang: targetLanguage)
        if !res.isEmpty {
            return res
        }
        return rawKey
    }
    
    /// Maps language enum to corresponding string catalog dictionary via Rust engine.
    public func catalog(for language: AppLanguage) -> [String: String] {
        var dict: [String: String] = [:]
        for key in L10n.allRawKeys {
            let res = ttzipI18nGetString(key: key, lang: language)
            dict[key] = res.isEmpty ? key : res
        }
        return dict
    }
}

// MARK: - Formatters & Extensions

extension ByteSizeStandard: @unchecked Sendable {
    public static var metricSI: ByteSizeStandard { .metricSi }
    public static var binaryIEC: ByteSizeStandard { .binaryIec }
}

/// Zero-heap-allocation byte capacity formatting engine powered by Rust.
public enum ByteSizeFormatter {
    
    /// Formats byte count according to standard and localized decimal conventions.
    public static func format(bytes: Int64, style: ByteSizeStandard = .metricSi, language: AppLanguage = .en) -> String {
        ttzipI18nFormatBytes(bytes: bytes, standard: style, lang: language)
    }
}

/// High-performance throughput rate formatting engine powered by Rust.
public enum ThroughputFormatter {
    
    /// Formats throughput rate in MB/s with locale-sensitive decimal formatting.
    public static func format(mbPerSec: Double, language: AppLanguage = .en) -> String {
        ttzipI18nFormatThroughput(mbPerSec: mbPerSec, lang: language)
    }
}

/// Unicode CLDR plural categories.
public enum PluralCategory: Sendable {
    case zero
    case one
    case two
    case few
    case many
    case other
}

/// Evaluator engine for Unicode CLDR plural rules across supported languages.
public enum PluralRuleEngine {
    
    /// Evaluates plural category given item count and language.
    public static func evaluate(count: Int64, language: AppLanguage) -> PluralCategory {
        switch language {
        case .zhHans, .zhHant, .ja:
            return .other
            
        case .en, .de, .es:
            return (count == 1) ? .one : .other
            
        case .fr:
            return (count == 0 || count == 1) ? .one : .other
        }
    }
}

extension ArchiveError {
    /// Localized error description for the current or specified language.
    public func localizedDescription(for language: AppLanguage? = nil) -> String {
        let manager = TTZipLocalizationManager.shared
        let targetLang = language ?? manager.currentLanguage
        if case .createTransactionIncomplete(let report) = self {
            return report.summaryMessage
        }
        if let key = simpleErrorKey {
            return manager.string(for: key, language: targetLang)
        }
        if case .invalidState = self {
            return manager.string(for: L10n.Common.error, language: targetLang)
        }
        let locale = Locale(identifier: targetLang.bcp47)
        return formattedLocalizedString(manager: manager, language: targetLang, locale: locale)
    }

    private var simpleErrorKey: L10n.Errors? {
        switch self {
        case .fileNotFound: return .fileNotFound
        case .invalidFormat: return .unsupportedFormat
        case .passwordRequired: return .passwordRequired
        case .wrongPassword: return .incorrectPassword
        case .cancelled, .extractionStopped: return .operationCancelled
        case .openFailed: return .readError
        case .corruptHeader: return .corruptedHeader
        default: return nil
        }
    }

    private func formattedLocalizedString(
        manager: TTZipLocalizationManager,
        language: AppLanguage,
        locale: Locale
    ) -> String {
        let lookup = { (key: L10n.Errors) in manager.string(for: key, language: language) }
        switch self {
        case .readFailed(let code):
            return String(format: lookup(.readError), locale: locale, code)
        case .passwordRequiredDetailed(let archivePath, let tier):
            let fileName = (archivePath as NSString).lastPathComponent
            let key: L10n.Errors = (tier == .headerAndData)
                ? .passwordRequiredHeaderAndData
                : .passwordRequiredPayload
            return String(format: lookup(key), locale: locale, fileName)
        case .unsupportedEncryptionMethod(let path, let method):
            let fileName = (path as NSString).lastPathComponent
            return String(format: lookup(.unsupportedEncryption), locale: locale, method, fileName)
        case .corruptedData(let archivePath, let entryPath):
            let fileName = (archivePath as NSString).lastPathComponent
            return "\(lookup(.corruptData)) (\(fileName): \(entryPath))"
        case .partialExtractionFailure(let outcome):
            return Self.formatPartialExtractionFailure(outcome)
        case .engineFailure(let code, let message):
            return String(format: lookup(.engineFailure), locale: locale, code, message)
        default:
            return manager.string(for: L10n.Common.error, language: language)
        }
    }

    private static func formatPartialExtractionFailure(_ outcome: ExtractionOutcome) -> String {
        let firstPath = outcome.failed.first?.relativePath ?? ""
        let suffix = firstPath.isEmpty ? "" : " (\(firstPath))"
        return "Extraction completed with \(outcome.failed.count) failed item(s)\(suffix)"
    }
}

public extension L10n {
    // MARK: - 22. Inspector Panel & Directory Canvas
    enum Inspector: String, LocaleKeyProtocol, CaseIterable {
        case title = "inspector.title"
        case contentBreakdown = "inspector.content_breakdown"
        case currentDirectory = "inspector.current_directory"
        case directoryCanvas = "inspector.directory_canvas"
        case emptyDirectory = "inspector.empty_directory"
        case emptyDirectoryDesc = "inspector.empty_directory_desc"
        case fileSystem = "inspector.file_system"
        case items = "inspector.items"
        case modified = "inspector.modified"
        case overviewFs = "inspector.overview_fs"
        case ownerGroup = "inspector.owner_group"
        case permissions = "inspector.permissions"
        case size = "inspector.size"
        case exposure = "inspector.exposure"
        case overviewAttributes = "inspector.overview_attributes"
        case whereLocation = "inspector.where_location"
        case quickLookHelp = "inspector.quick_look_help"
        case revealInFinderHelp = "inspector.reveal_in_finder_help"
        case extractArchiveHelp = "inspector.extract_archive_help"
        case compressFileHelp = "inspector.compress_file_help"
        case appleFileSystemApfs = "inspector.apple_file_system_apfs"
        case owner = "inspector.owner"
        case ownerGroupFormat = "inspector.owner_group_format"
        case videoSpecs = "inspector.video_specs"
        case imageSpecs = "inspector.image_specs"
        case resolution = "inspector.resolution"
        case colorSpace = "inspector.color_space"
        case bitDepth = "inspector.bit_depth"
        case cameraExif = "inspector.camera_exif"
        case camera = "inspector.camera"
        case toggleInspectorPanel = "inspector.toggle_inspector_panel"
        case multiItemsFormat = "inspector.multi_items_format"
        case filesLabel = "inspector.files_label"
        case foldersLabel = "inspector.folders_label"
        case largestItem = "inspector.largest_item"
        case fileTypes = "inspector.file_types"
        case selectToInspect = "inspector.select_to_inspect"
        case storageActive = "inspector.storage_active"
        case multiSelectionTitle = "inspector.multi_selection_title"
        case quickLookShortcut = "inspector.quick_look_shortcut"
        case itemActionsHelp = "inspector.item_actions_help"
    }

    // MARK: - 23. Spotlight & Global Search Overlay
    enum Search: String, LocaleKeyProtocol, CaseIterable {
        case placeholder = "search.placeholder"
        case actionOpen = "search.action_open"
        case actionReveal = "search.action_reveal"
        case actionQuickLook = "search.action_quick_look"
        case actionDismiss = "search.action_dismiss"
        case badgeActiveArchive = "search.badge_active_archive"
        case badgeFolder = "search.badge_folder"
        case badgeFile = "search.badge_file"
        case badgeRecent = "search.badge_recent"
        case badgeAction = "search.badge_action"
        case noResults = "search.no_results"
        case searchInFolder = "search.search_in_folder"
    }

    // MARK: - 24. Peripheral Device & ADB / MTP Pairing
    enum Device: String, LocaleKeyProtocol, CaseIterable {
        case pairingInProgress = "device.pairing_in_progress"
        case pairingSuccess = "device.pairing_success"
        case pairingSuccessDesc = "device.pairing_success_desc"
        case pairingFailed = "device.pairing_failed"
        case pairingFailedDesc = "device.pairing_failed_desc"
        case phoneStepsTitle = "device.phone_steps_title"
        case phoneStepsDesc = "device.phone_steps_desc"
        case pinCodeTitle = "device.pin_code_title"
        case invalidPortError = "device.invalid_port_error"
        case pinDigitsError = "device.pin_digits_error"
        case specifyIpError = "device.specify_ip_error"
        case handshakeStep = "device.handshake_step"
        case verifyPinStep = "device.verify_pin_step"
        case timeoutMsg = "device.timeout_msg"
        case guideTitle = "device.guide_title"
        case guideStep1Title = "device.guide_step1_title"
        case guideStep1Desc = "device.guide_step1_desc"
        case guideStep2Title = "device.guide_step2_title"
        case guideStep2Desc = "device.guide_step2_desc"
        case guideStep3Title = "device.guide_step3_title"
        case guideStep3Desc = "device.guide_step3_desc"
        case guideStep4Title = "device.guide_step4_title"
        case guideStep4Desc = "device.guide_step4_desc"
        case adbActive = "device.adb_active"
        case adbNotFound = "device.adb_not_found"
        case recheck = "device.recheck"
        case switchAdbMode = "device.switch_adb_mode"
        case switchAdbModeDesc = "device.switch_adb_mode_desc"
    }
}
