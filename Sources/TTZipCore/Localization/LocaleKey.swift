// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

import Foundation

/// Unified protocol for strongly-typed localization keys.
public protocol LocaleKeyProtocol: Sendable {
    var rawKey: String { get }
}

public extension LocaleKeyProtocol where Self: RawRepresentable, RawValue == String {
    @inlinable var rawKey: String { rawValue }
}

/// Strongly-typed namespaced localization keys.
public enum L10n {
    
    // MARK: - 1. Common Actions & States
    public enum Common: String, LocaleKeyProtocol, CaseIterable {
        case cancel = "common.cancel"
        case ok = "common.ok"
        case done = "common.done"
        case save = "common.save"
        case close = "common.close"
        case retry = "common.retry"
        case delete = "common.delete"
        case apply = "common.apply"
        case search = "common.search"
        case success = "common.success"
        case failed = "common.failed"
        case loading = "common.loading"
        case warning = "common.warning"
        case error = "common.error"
        case processing = "common.processing"
        case copy = "common.copy"
        case paste = "common.paste"
        case clear = "common.clear"
        case export = "common.export"
        case importAction = "common.import"
        case selectAll = "common.select_all"
        case revealInFinder = "common.reveal_in_finder"
        case selectDestination = "common.select_destination"
        case openFiles = "common.open_files"
        case chooseFolder = "common.choose_folder"
        case browse = "common.browse"
        case refresh = "common.refresh"
    }
    
    // MARK: - 2. Sidebar Navigation & Layout
    public enum Sidebar: String, LocaleKeyProtocol, CaseIterable {
        case homeAndExtract = "sidebar.home_and_extract"
        case newArchive = "sidebar.new_archive"
        case presets = "sidebar.presets"
        case benchmark = "sidebar.benchmark"
        case vault = "sidebar.vault"
        case licensing = "sidebar.licensing"
        case settings = "sidebar.settings"
        case queue = "sidebar.queue"
        case indexHeader = "sidebar.index_header"
        case proBadge = "sidebar.pro_badge"
        case openArchiveHeader = "sidebar.open_archive_header"
        case printedInMacOS = "sidebar.printed_in_macos"
        case zeroCopyAcceleration = "sidebar.zero_copy_acceleration"
    }
    
    // MARK: - 3. File Explorer & Archive Outline
    public enum Explorer: String, LocaleKeyProtocol, CaseIterable {
        case columnsView = "explorer.columns_view"
        case gridView = "explorer.grid_view"
        case listView = "explorer.list_view"
        case sortByName = "explorer.sort_by_name"
        case sortBySize = "explorer.sort_by_size"
        case sortByDate = "explorer.sort_by_date"
        case sortByKind = "explorer.sort_by_kind"
        case emptyDirectory = "explorer.empty_directory"
        case dragDropPrompt = "explorer.drag_drop_prompt"
        case encryptedBadge = "explorer.encrypted_badge"
        case quickLook = "explorer.quick_look"
        case deleteEntryPrompt = "explorer.delete_entry_prompt"
        case folderComposition = "explorer.folder_composition"
        case extractToPrompt = "explorer.extract_to_prompt"
        case nameHeader = "explorer.name_header"
        case sizeHeader = "explorer.size_header"
        case ratioHeader = "explorer.ratio_header"
        case dateHeader = "explorer.date_header"
        case kindHeader = "explorer.kind_header"
        case crc32Header = "explorer.crc32_header"
        case compressedSizeHeader = "explorer.compressed_size_header"
        case itemsCountHeader = "explorer.items_count_header"
        case folderHeader = "explorer.folder_header"
        case archiveHeader = "explorer.archive_header"
        case passwordProtectedArchive = "explorer.password_protected_archive"
        case loadingArchiveStructure = "explorer.loading_archive_structure"
        case newFolder = "explorer.new_folder"
        case newFile = "explorer.new_file"
        case folderName = "explorer.folder_name"
        case fileName = "explorer.file_name"
        case rootAccess = "explorer.root_access"
        case rootAccessHelp = "explorer.root_access_help"
        case create = "explorer.create"
    }
    
    // MARK: - 4. Compression Options & Progress
    public enum Compress: String, LocaleKeyProtocol, CaseIterable {
        case title = "compress.title"
        case startAction = "compress.start_action"
        case format = "compress.format"
        case level = "compress.level"
        case levelStore = "compress.level_store"
        case levelFastest = "compress.level_fastest"
        case levelNormal = "compress.level_normal"
        case levelMaximum = "compress.level_maximum"
        case levelUltra = "compress.level_ultra"
        case solidArchive = "compress.solid_archive"
        case splitVolume = "compress.split_volume"
        case splitVolumeCustom = "compress.split_volume_custom"
        case splitVolumeNone = "compress.split_volume_none"
        case encryption = "compress.encryption"
        case encryptFileNames = "compress.encrypt_file_names"
        case deleteSource = "compress.delete_source"
        case openFinder = "compress.open_finder"
        case filterMacJunk = "compress.filter_mac_junk"
        case cpuThreads = "compress.cpu_threads"
        case allCores = "compress.all_cores"
        case dictionarySize = "compress.dictionary_size"
        case summaryOriginal = "compress.summary_original"
        case summaryCompressed = "compress.summary_compressed"
        case summaryRatio = "compress.summary_ratio"
        case summarySpeed = "compress.summary_speed"
        case fastPresetDesc = "compress.fast_preset_desc"
        case normalPresetDesc = "compress.normal_preset_desc"
        case maximumPresetDesc = "compress.maximum_preset_desc"
        case smartStoreBypassTitle = "compress.smart_store_bypass_title"
        case smartStoreBypassDesc = "compress.smart_store_bypass_desc"
        case targetFolder = "compress.target_folder"
        case archiveNamePlaceholder = "compress.archive_name_placeholder"
        case createSeparateArchives = "compress.create_separate_archives"
        case targetParameters = "compress.target_parameters"
        case hardwareAcceleration = "compress.hardware_acceleration"
        case formatGuide = "compress.format_guide"
    }
    
    // MARK: - 5. Extraction Workflow
    public enum Extract: String, LocaleKeyProtocol, CaseIterable {
        case title = "extract.title"
        case action = "extract.action"
        case here = "extract.here"
        case toSubfolder = "extract.to_subfolder"
        case destination = "extract.destination"
        case autoOpenFolder = "extract.auto_open_folder"
        case passwordPrompt = "extract.password_prompt"
        case saveToVault = "extract.save_to_vault"
        case conflictOverwrite = "extract.conflict_overwrite"
        case conflictSkip = "extract.conflict_skip"
        case conflictRename = "extract.conflict_rename"
        case successNotice = "extract.success_notice"
        case failureNotice = "extract.failure_notice"
        case enterPasswordPlaceholder = "extract.enter_password_placeholder"
        case incorrectPasswordPrompt = "extract.incorrect_password_prompt"
        case extractingTo = "extract.extracting_to"
        case totalExtractedFiles = "extract.total_extracted_files"
        case speedMetric = "extract.speed_metric"
    }
    
    // MARK: - 6. Benchmark Metrics & Speed Test
    public enum Benchmark: String, LocaleKeyProtocol, CaseIterable {
        case throughput = "benchmark.throughput"
        case compressionRatio = "benchmark.compression_ratio"
        case duration = "benchmark.duration"
        case memoryUsage = "benchmark.memory_usage"
        case peakThroughput = "benchmark.peak_throughput"
        case speedup = "benchmark.speedup"
        case runAction = "benchmark.run_action"
        case passStatus = "benchmark.pass_status"
        case appleSiliconTopology = "benchmark.apple_silicon_topology"
        case competitorCompare = "benchmark.competitor_compare"
        case benchmarkMatrixTitle = "benchmark.benchmark_matrix_title"
        case benchmarkSuiteShortcut = "benchmark.benchmark_suite_shortcut"
        case spaceReducedSummary = "benchmark.space_reduced_summary"
        case singleCoreVsMultiCore = "benchmark.single_core_vs_multicore"
        case hardwareCoresFormat = "benchmark.hardware_cores_format"
        case hardwareMemoryFormat = "benchmark.hardware_memory_format"
        case liveDialTitle = "benchmark.live_dial_title"
        case testMode = "benchmark.test_mode"
        case dataScale = "benchmark.data_scale"
        case customCorpus = "benchmark.custom_corpus"
        case changeCorpus = "benchmark.change_corpus"
    }
    
    // MARK: - 7. Custom Presets Workspace
    public enum Presets: String, LocaleKeyProtocol, CaseIterable {
        case title = "presets.title"
        case createNew = "presets.create_new"
        case duplicate = "presets.duplicate"
        case resetDefaults = "presets.reset_defaults"
        case proConfig = "presets.pro_config"
        case undo = "presets.undo"
        case redo = "presets.redo"
        case saveDraft = "presets.save_draft"
        case name = "presets.name"
        case desc = "presets.desc"
        case deletePreset = "presets.delete_preset"
        case presetNamePlaceholder = "presets.preset_name_placeholder"
        case formatSelector = "presets.format_selector"
        case compressionTier = "presets.compression_tier"
        case filterMacJunkOption = "presets.filter_mac_junk_option"
        case solidBlockOption = "presets.solid_block_option"
        case copySuffix = "presets.copy_suffix"
        case volumeSplitting = "presets.volume_splitting"
        case volumeSize = "presets.volume_size"
        case filterRules = "presets.filter_rules"
        case filterMacJunkDesc = "presets.filter_mac_junk_desc"
        case filterGitDesc = "presets.filter_git_desc"
    }
    
    // MARK: - 8. Password Keychain Vault
    public enum Vault: String, LocaleKeyProtocol, CaseIterable {
        case title = "vault.title"
        case unlockPrompt = "vault.unlock_prompt"
        case addPassword = "vault.add_password"
        case emptyVault = "vault.empty_vault"
        case labelPlaceholder = "vault.label_placeholder"
        case passwordPlaceholder = "vault.password_placeholder"
        case unlockButton = "vault.unlock_button"
        case lockVault = "vault.lock_vault"
        case deleteEntry = "vault.delete_entry"
        case copyPassword = "vault.copy_password"
        case biometricPrompt = "vault.biometric_prompt"
        case vaultSecurityHeader = "vault.vault_security_header"
        case pbkdf2Desc = "vault.pbkdf2_desc"
        case volatileZeroingDesc = "vault.volatile_zeroing_desc"
        case aesGcmStorageDesc = "vault.aes_gcm_storage_desc"
        case noPasswordsSavedPrompt = "vault.no_passwords_saved_prompt"
        case strengthVeryWeak = "vault.strength_very_weak"
        case strengthWeak = "vault.strength_weak"
        case strengthMedium = "vault.strength_medium"
        case strengthStrong = "vault.strength_strong"
        case strengthVeryStrong = "vault.strength_very_strong"
        case biometricReason = "vault.biometric_reason"
        case authCancelled = "vault.auth_cancelled"
        case touchIDNotEnrolled = "vault.touch_id_not_enrolled"
        case touchIDLockedOut = "vault.touch_id_locked_out"
    }
    
    // MARK: - 9. Preferences & Settings
    public enum Settings: String, LocaleKeyProtocol, CaseIterable {
        case title = "settings.title"
        case general = "settings.general"
        case localization = "settings.localization"
        case language = "settings.language"
        case byteUnits = "settings.byte_units"
        case unitSI = "settings.unit_si"
        case unitIEC = "settings.unit_iec"
        case licenseStatus = "settings.license_status"
        case hardwareTopology = "settings.hardware_topology"
        case defaultFormat = "settings.default_format"
        case finderIntegration = "settings.finder_integration"
        case smartStoreBypass = "settings.smart_store_bypass"
        case bypassHighEntropyDesc = "settings.bypass_high_entropy_desc"
        case instantSwitchNote = "settings.instant_switch_note"
        case proLicenseActive = "settings.pro_license_active"
        case freeEdition = "settings.free_edition"
        case macAppStoreLicenseActive = "settings.mac_app_store_license_active"
        case enterActivationKey = "settings.enter_activation_key"
        case activateProButton = "settings.activate_pro_button"
        case deactivateButton = "settings.deactivate_button"
        case licenseStatusActive = "settings.license_status_active"
        case invalidKeyError = "settings.invalid_key_error"
        case chipModel = "settings.chip_model"
        case cpuCores = "settings.cpu_cores"
        case unifiedMemory = "settings.unified_memory"
    }
    
    // MARK: - 10. Operations Queue
    public enum Queue: String, LocaleKeyProtocol, CaseIterable {
        case title = "queue.title"
        case activeTasks = "queue.active_tasks"
        case overallThroughput = "queue.overall_throughput"
        case emptyQueue = "queue.empty_queue"
        case cancelTask = "queue.cancel_task"
        case pauseAll = "queue.pause_all"
        case resumeAll = "queue.resume_all"
        case taskCompressing = "queue.task_compressing"
        case taskExtracting = "queue.task_extracting"
        case taskWaiting = "queue.task_waiting"
        case taskCompleted = "queue.task_completed"
        case taskCancelled = "queue.task_cancelled"
        case taskFailed = "queue.task_failed"
        case clearCompleted = "queue.clear_completed"
    }
    
    // MARK: - Meta-Type Registration & Reflective Key Collection
    public static let allKeyGroups: [any (LocaleKeyProtocol & CaseIterable).Type] = [
        Common.self, Sidebar.self, Explorer.self, Compress.self, Extract.self,
        Benchmark.self, Presets.self, Vault.self, Settings.self, Queue.self,
        Preview.self, Menu.self, Dialogs.self, Errors.self, Units.self,
        CLI.self, Notification.self, Diagnostics.self, Recovery.self,
        QuickLook.self, FinderSync.self
    ]
    
    /// Returns all defined raw keys across all localization namespaces.
    public static var allRawKeys: [String] {
        var keys: [String] = []
        for group in allKeyGroups {
            if let groupCases = group.allCases as? [any LocaleKeyProtocol] {
                keys.append(contentsOf: groupCases.map(\.rawKey))
            }
        }
        return keys
    }
}

extension L10n {
    
    // MARK: - 11. Media & Document Previews
    public enum Preview: String, LocaleKeyProtocol, CaseIterable {
        case loading = "preview.loading"
        case unsupported = "preview.unsupported"
        case fullScreen = "preview.full_screen"
        case exitFullScreen = "preview.exit_full_screen"
        case pageCount = "preview.page_count"
        case dimensions = "preview.dimensions"
        case rawTextView = "preview.raw_text_view"
        case audioVisualizer = "preview.audio_visualizer"
        case documentReader = "preview.document_reader"
        case syntaxHighlighting = "preview.syntax_highlighting"
        case cannotPreviewFormat = "preview.cannot_preview_format"
        case mediaMetadata = "preview.media_metadata"
        case openInReader = "preview.open_in_reader"
        case typography = "preview.typography"
        case typographyTheme = "preview.typography_theme"
        case fontFamily = "preview.font_family"
        case fontSize = "preview.font_size"
        case noChapterContent = "preview.no_chapter_content"
        case audioSpecs = "preview.audio_specs"
        case rewind15 = "preview.rewind_15"
        case forward15 = "preview.forward_15"
        case zoomOut = "preview.zoom_out"
        case zoomIn = "preview.zoom_in"
        case resetZoom = "preview.reset_zoom"
    }
    
    // MARK: - 12. AppKit Menu & Finder Extensions
    public enum Menu: String, LocaleKeyProtocol, CaseIterable {
        case about = "menu.about"
        case hide = "menu.hide"
        case hideOthers = "menu.hide_others"
        case showAll = "menu.show_all"
        case quit = "menu.quit"
        case closeWindow = "menu.close_window"
        case minimize = "menu.minimize"
        case zoom = "menu.zoom"
        case fileMenu = "menu.file_menu"
        case editMenu = "menu.edit_menu"
        case viewMenu = "menu.view_menu"
        case windowMenu = "menu.window_menu"
        case helpMenu = "menu.help_menu"
        case checkForUpdates = "menu.check_for_updates"
        case preferences = "menu.preferences"
        case openArchive = "menu.open_archive"
        case newArchiveMenu = "menu.new_archive_menu"
        case selectAllMenu = "menu.select_all_menu"
        case toggleFullScreen = "menu.toggle_full_screen"
        case undo = "menu.undo"
        case redo = "menu.redo"
        case cut = "menu.cut"
        case copy = "menu.copy"
        case paste = "menu.paste"
        case pasteAndMatchStyle = "menu.paste_and_match_style"
        case delete = "menu.delete"
        case bringAllToFront = "menu.bring_all_to_front"
        case services = "menu.services"
        case openRecent = "menu.open_recent"
        case clearMenu = "menu.clear_menu"
        case finderExtractHere = "menu.finder_extract_here"
        case finderExtractSubfolder = "menu.finder_extract_subfolder"
        case finderInspect = "menu.finder_inspect"
        case finderAutofillVault = "menu.finder_autofill_vault"
        case finderComputeHash = "menu.finder_compute_hash"
        case finderCompress7z = "menu.finder_compress_7z"
        case finderCompressZip = "menu.finder_compress_zip"
        case finderCompressSeparate = "menu.finder_compress_separate"
        case finderCompressDeleteSource = "menu.finder_compress_delete_source"
        case finderCompressAdvanced = "menu.finder_compress_advanced"
    }
    
    // MARK: - 13. System Dialogs & Confirmations
    public enum Dialogs: String, LocaleKeyProtocol, CaseIterable {
        case confirmDeleteTitle = "dialogs.confirm_delete_title"
        case confirmDeleteMessage = "dialogs.confirm_delete_message"
        case overwriteTitle = "dialogs.overwrite_title"
        case overwriteMessage = "dialogs.overwrite_message"
        case unsavedChangesTitle = "dialogs.unsaved_changes_title"
        case unsavedChangesMessage = "dialogs.unsaved_changes_message"
        case wrongPasswordTitle = "dialogs.wrong_password_title"
        case wrongPasswordMessage = "dialogs.wrong_password_message"
        case operationErrorTitle = "dialogs.operation_error_title"
        case alertOk = "dialogs.alert_ok"
        case alertCancel = "dialogs.alert_cancel"
        case alertOverwrite = "dialogs.alert_overwrite"
        case alertSkip = "dialogs.alert_skip"
    }
    
    // MARK: - 14. Error Diagnostics & Defense
    public enum Errors: String, LocaleKeyProtocol, CaseIterable {
        case fileNotFound = "error.file_not_found"
        case permissionDenied = "error.permission_denied"
        case diskFull = "error.disk_full"
        case zipSlipDetected = "error.zip_slip_detected"
        case corruptedHeader = "error.corrupted_header"
        case crcMismatch = "error.crc_mismatch"
        case outOfMemory = "error.out_of_memory"
        case operationCancelled = "error.operation_cancelled"
        case passwordRequired = "error.password_required"
        case incorrectPassword = "error.incorrect_password"
        case unsupportedFormat = "error.unsupported_format"
        case corruptData = "error.corrupt_data"
        case readError = "error.read_error"
        case writeError = "error.write_error"
        case passwordRequiredHeaderAndData = "error.password_required_header_and_data"
        case passwordRequiredPayload = "error.password_required_payload"
        case engineFailure = "error.engine_failure"
        case unsupportedEncryption = "error.unsupported_encryption"
    }
    
    // MARK: - 15. Units of Measurement & Counters
    public enum Units: String, LocaleKeyProtocol, CaseIterable {
        case bytes = "units.bytes"
        case kb = "units.kb"
        case mb = "units.mb"
        case gb = "units.gb"
        case tb = "units.tb"
        case mbPerSec = "units.mb_per_sec"
        case seconds = "units.seconds"
        case percent = "units.percent"
        case itemsCount = "units.items_count"
        case coresCount = "units.cores_count"
        case unifiedMemoryGB = "units.unified_memory_gb"
    }
    
    // MARK: - 16. Standalone CLI Output
    public enum CLI: String, LocaleKeyProtocol, CaseIterable {
        case usageHeader = "cli.usage_header"
        case subcommands = "cli.subcommands"
        case globalOptions = "cli.global_options"
        case errorMissingArg = "cli.error_missing_arg"
        case errorFileNotFound = "cli.error_file_not_found"
        case errorInvalidFormat = "cli.error_invalid_format"
        case dryRunPrefix = "cli.dry_run_prefix"
        case benchRunning = "cli.bench_running"
        case testSummary = "cli.test_summary"
    }
    
    // MARK: - 17. System Notifications
    public enum Notification: String, LocaleKeyProtocol, CaseIterable {
        case taskCompletedTitle = "notification.task_completed_title"
        case taskCompletedBody = "notification.task_completed_body"
        case taskFailedTitle = "notification.task_failed_title"
        case taskFailedBody = "notification.task_failed_body"
        case threatInterceptedTitle = "notification.threat_intercepted_title"
        case threatInterceptedBody = "notification.threat_intercepted_body"
    }
    
    // MARK: - 18. Archive Diagnostics & Standards Inspector
    public enum Diagnostics: String, LocaleKeyProtocol, CaseIterable {
        case title = "diagnostics.title"
        case scanning = "diagnostics.scanning"
        case parseError = "diagnostics.parse_error"
        case citations = "diagnostics.citations"
        case signatures = "diagnostics.signatures"
        case noSignatures = "diagnostics.no_signatures"
        case verified = "diagnostics.verified"
        case signatureBytes = "diagnostics.signature_bytes"
        case zipTlv = "diagnostics.zip_tlv"
        case noExtraFields = "diagnostics.no_extra_fields"
        case validatedHeaders = "diagnostics.validated_headers"
        case deviationsWarnings = "diagnostics.deviations_warnings"
        case noReport = "diagnostics.no_report"
        case diagnosticTime = "diagnostics.diagnostic_time"
        case compliant = "diagnostics.compliant"
        case deviationsDetected = "diagnostics.deviations_detected"
        case damageBreakdown = "diagnostics.damage_breakdown"
    }
    
    // MARK: - 19. Multi-Core Password Recovery
    public enum Recovery: String, LocaleKeyProtocol, CaseIterable {
        case title = "recovery.title"
        case subtitle = "recovery.subtitle"
        case targetArchive = "recovery.target_archive"
        case archivePlaceholder = "recovery.archive_placeholder"
        case candidateWords = "recovery.candidate_words"
        case saveToVault = "recovery.save_to_vault"
        case recovering = "recovery.recovering"
        case foundPassword = "recovery.found_password"
    }
    
    // MARK: - 20. QuickLook Preview HTML
    public enum QuickLook: String, LocaleKeyProtocol, CaseIterable {
        case tableHeaderName = "quicklook.table_header_name"
        case tableHeaderSize = "quicklook.table_header_size"
        case renderedFooter = "quicklook.rendered_footer"
        case compressedFormat = "quicklook.compressed_format"
        case itemsOmittedFormat = "quicklook.items_omitted_format"
        case encryptedBadge = "quicklook.encrypted_badge"
        case cannotPreview = "quicklook.cannot_preview"
        case protectedWithPassword = "quicklook.protected_with_password"
    }
    
    // MARK: - 21. FinderSync Integration
    public enum FinderSync: String, LocaleKeyProtocol, CaseIterable {
        case extractHereTitle = "findersync.extract_here_title"
        case extractSubfolderTitle = "findersync.extract_subfolder_title"
        case inspectTitle = "findersync.inspect_title"
        case autofillTitle = "findersync.autofill_title"
        case computeHashTitle = "findersync.compute_hash_title"
        case compress7zTitle = "findersync.compress_7z_title"
        case compressZipTitle = "findersync.compress_zip_title"
        case compressSeparateTitle = "findersync.compress_separate_title"
        case compressDeleteSourceTitle = "findersync.compress_delete_source_title"
        case compressAdvancedTitle = "findersync.compress_advanced_title"
    }
}
