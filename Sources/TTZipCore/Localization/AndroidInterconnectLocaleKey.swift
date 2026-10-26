// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

/// Strongly-typed localization keys for Android USB MTP, USB ADB, wireless pairing,
/// and bidirectional file transfer flows (Q3 Android Interconnect).
/// Integrates with `TTZipLocalizationManager` (`TTI18n`) and provides built-in
/// `zh-Hans`, `zh-Hant`, and `en` translations with deterministic English fallback.
public enum AndroidInterconnectLocaleKey: String, LocaleKeyProtocol, CaseIterable, Sendable {
    // MARK: - 1. Wireless Pairing Sheet & PIN Handshake
    case pairingSheetTitle = "android.pairing.sheet_title"
    case pairingSheetSubtitleFormat = "android.pairing.sheet_subtitle_format"
    case pairingHeaderTitle = "android.pairing.header_title"
    case pairingHeaderSubtitle = "android.pairing.header_subtitle"
    case pairingIpPortLabel = "android.pairing.ip_port_label"
    case pairingPortPlaceholder = "android.pairing.port_placeholder"
    case pairingActionPairConnect = "android.pairing.action_pair_connect"
    case pairingActionCancelHandshake = "android.pairing.action_cancel_handshake"
    case pairingInvalidParameters = "android.pairing.invalid_parameters"
    case pairingInstructions = "android.pairing.instructions"
    case pairingPortEndpointFormat = "android.pairing.port_endpoint_format"
    case pairingQrCapabilityNotice = "android.pairing.qr_capability_notice"
    case pairingSecurityBadge = "android.pairing.security_badge"
    case pairingPinFieldFormat = "android.pairing.pin_field_format"
    case pairingClearPin = "android.pairing.clear_pin"
    case pairingCancel = "android.pairing.cancel"
    case pairingActionPair = "android.pairing.action_pair"
    case pairingActionPairing = "android.pairing.action_pairing"
    case pairingStatusSuccess = "android.pairing.status_success"
    case wirelessStatusReady = "android.wireless.status_ready"
    case wirelessStatusPairing = "android.wireless.status_pairing"
    case wirelessStatusFailed = "android.wireless.status_failed"
    case wirelessStatusIdle = "android.wireless.status_idle"

    // MARK: - 2. Pairing & Connection Errors
    case noActiveDevice = "android.error.no_active_device"
    case errorInvalidPinLength = "android.error.invalid_pin_length"
    case errorInvalidEndpoint = "android.error.invalid_endpoint"
    case errorDaemonUnreachable = "android.error.daemon_unreachable"
    case errorPinRejectedFormat = "android.error.pin_rejected_format"
    case errorConnectRejectedFormat = "android.error.connect_rejected_format"
    case errorDeviceUnauthorized = "android.error.device_unauthorized"
    case errorDeviceOffline = "android.error.device_offline"
    case errorConnectPortUnresolved = "android.error.connect_port_unresolved"
    case errorTransferDeviceDisconnected = "android.error.transfer_device_disconnected"
    case errorTransferIncompleteBytes = "android.error.transfer_incomplete_bytes"

    // MARK: - 3. ADB Enable Guide Sheet
    case guideStandardMtpTitle = "android.guide.standard_mtp_title"
    case guideStandardMtpSpeed = "android.guide.standard_mtp_speed"
    case guideStandardMtpDesc = "android.guide.standard_mtp_desc"
    case guideHighSpeedAdbTitle = "android.guide.high_speed_adb_title"
    case guideHighSpeedAdbSpeed = "android.guide.high_speed_adb_speed"
    case guideHighSpeedAdbDesc = "android.guide.high_speed_adb_desc"
    case guideHeaderTitle = "android.guide.header_title"
    case guideHeaderSubtitle = "android.guide.header_subtitle"
    case guideStep1Title = "android.guide.step1_title"
    case guideStep1Body = "android.guide.step1_body"
    case guideStep2Title = "android.guide.step2_title"
    case guideStep2Body = "android.guide.step2_body"
    case guideStep3Title = "android.guide.step3_title"
    case guideStep3Body = "android.guide.step3_body"
    case guideStep4Title = "android.guide.step4_title"
    case guideStep4Body = "android.guide.step4_body"
    case guideProbeIdle = "android.guide.probe_idle"
    case guideProbeProbing = "android.guide.probe_probing"
    case guideProbeActive = "android.guide.probe_active"
    case guideProbeDaemonOnlyNoDevice = "android.guide.probe_daemon_only_no_device"
    case guideProbeNotDetected = "android.guide.probe_not_detected"
    case guideContinueMtp = "android.guide.continue_mtp"
    case guideVerifyRetry = "android.guide.verify_retry"
    case guideVerifyAction = "android.guide.verify_action"
    case guideDoneAction = "android.guide.done_action"

    /// Resolves the localized string for the current active language.
    public var localized: String {
        resolve()
    }

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

    private static let translations: [AndroidInterconnectLocaleKey: TranslationTriple] = [
        .pairingSheetTitle: TranslationTriple(
            "Pair with Android Device",
            "配对 Android 设备",
            "配對 Android 裝置"
        ),
        .pairingSheetSubtitleFormat: TranslationTriple(
            "Enter the 6-digit pairing code shown on %1$@",
            "输入 %1$@ 上显示的 6 位配对码",
            "輸入 %1$@ 上顯示的 6 位配對碼"
        ),
        .pairingHeaderTitle: TranslationTriple(
            "Pair Android Device via Wi-Fi",
            "通过 Wi-Fi 配对 Android 设备",
            "透過 Wi-Fi 配對 Android 裝置"
        ),
        .pairingHeaderSubtitle: TranslationTriple(
            "Cable-free high-speed management using Android 11+ Wireless Debugging",
            "基于 Android 11+ 无线调试的免线缆高速文件传输",
            "基於 Android 11+ 無線偵錯的免線纜高速檔案傳輸"
        ),
        .pairingIpPortLabel: TranslationTriple(
            "Device IP & Pairing Port",
            "设备 IP 与临时配对端口",
            "裝置 IP 與臨時配對連接埠"
        ),
        .pairingPortPlaceholder: TranslationTriple(
            "Port",
            "端口",
            "連接埠"
        ),
        .pairingActionPairConnect: TranslationTriple(
            "Pair & Connect",
            "配对并连接",
            "配對並連線"
        ),
        .pairingActionCancelHandshake: TranslationTriple(
            "Cancel Handshake",
            "取消握手",
            "取消交握"
        ),
        .pairingInvalidParameters: TranslationTriple(
            "Invalid pairing parameters. A 6-digit numeric PIN and valid host:port are required.",
            "配对参数无效。请输入 6 位纯数字配对码与有效的主机地址及端口。",
            "配對參數無效。請輸入 6 位純數字配對碼與有效的主機位址及連接埠。"
        ),
        .pairingInstructions: TranslationTriple(
            "On your Android 11+ device, go to Settings > Developer options > Wireless debugging > Pair device with pairing code.",
            "在 Android 11+ 手机上打开“设置 > 开发者选项 > 无线调试 > 使用配对码配对设备”。",
            "在 Android 11+ 手機上開啟「設定 > 開發人員選項 > 無線偵錯 > 使用配對碼配對裝置」。"
        ),
        .pairingPortEndpointFormat: TranslationTriple(
            "Pairing Endpoint: %1$@:%2$d (Ephemeral Pairing Port)",
            "配对端点：%1$@:%2$d（临时配对端口）",
            "配對端點：%1$@:%2$d（臨時配對連接埠）"
        ),
        .pairingQrCapabilityNotice: TranslationTriple(
            "Note: Use the 6-digit PIN pairing port shown in the pairing dialog, not the wireless debugging connect port. QR code camera scanning is not supported in-app.",
            "注意：请使用配对弹窗中的 6 位配对码与临时配对端口（非无线调试主界面的连接端口）。应用内暂不支持二维码相机扫描配对。",
            "注意：請使用配對彈窗中的 6 位配對碼與臨時配對連接埠（非無線偵錯主畫面的連線連接埠）。應用內暫不支援 QR 圖碼相機掃描配對。"
        ),
        .pairingSecurityBadge: TranslationTriple(
            "ADB Smart Socket · host:pair Handshake",
            "ADB Smart Socket · host:pair 配对握手",
            "ADB Smart Socket · host:pair 配對交握"
        ),
        .pairingPinFieldFormat: TranslationTriple(
            "Pairing digit %1$d of 6",
            "第 %1$d 位配对码（共 6 位）",
            "第 %1$d 位配對碼（共 6 位）"
        ),
        .pairingClearPin: TranslationTriple(
            "Clear PIN",
            "清空配对码",
            "清除配對碼"
        ),
        .pairingCancel: TranslationTriple(
            "Cancel",
            "取消",
            "取消"
        ),
        .pairingActionPair: TranslationTriple(
            "Pair Device",
            "配对设备",
            "配對裝置"
        ),
        .pairingActionPairing: TranslationTriple(
            "Pairing...",
            "正在配对...",
            "正在配對..."
        ),
        .pairingStatusSuccess: TranslationTriple(
            "Paired & Authorized",
            "已完成配对与授权",
            "已完成配對與授權"
        ),
        .wirelessStatusReady: TranslationTriple(
            "Ready",
            "就绪",
            "就緒"
        ),
        .wirelessStatusPairing: TranslationTriple(
            "Pairing",
            "待配对",
            "待配對"
        ),
        .wirelessStatusFailed: TranslationTriple(
            "Failed",
            "失败",
            "失敗"
        ),
        .wirelessStatusIdle: TranslationTriple(
            "Idle",
            "空闲",
            "閒置"
        ),
        .noActiveDevice: TranslationTriple(
            "No active Android device connected.",
            "未连接可用的 Android 设备。",
            "未連線可用的 Android 裝置。"
        ),
        .guideStandardMtpTitle: TranslationTriple(
            "Standard USB (MTP)",
            "标准 USB (MTP)",
            "標準 USB (MTP)"
        ),
        .guideStandardMtpSpeed: TranslationTriple(
            "15 MB/s · No /data Access",
            "15 MB/s · 无 /data 访问权限",
            "15 MB/s · 無 /data 存取權限"
        ),
        .guideStandardMtpDesc: TranslationTriple(
            "Subject to Android 11+ Scoped Storage blocks",
            "受限于 Android 11+ 分区存储隔离",
            "受限於 Android 11+ 分區儲存隔離"
        ),
        .guideHighSpeedAdbTitle: TranslationTriple(
            "High-Speed Channel (ADB)",
            "高速通道 (ADB)",
            "高速通道 (ADB)"
        ),
        .guideHighSpeedAdbSpeed: TranslationTriple(
            "80~120 MB/s · Full Access",
            "80~120 MB/s · 完整读写",
            "80~120 MB/s · 完整讀寫"
        ),
        .guideHighSpeedAdbDesc: TranslationTriple(
            "Complete read/write to all app packages without root",
            "免 Root 完整读写应用数据包",
            "免 Root 完整讀寫應用資料包"
        ),
        .errorInvalidPinLength: TranslationTriple(
            "Pairing PIN must be exactly 6 numeric digits (0-9).",
            "配对码必须为 6 位纯数字（0-9）。",
            "配對碼必須為 6 位純數字（0-9）。"
        ),
        .errorInvalidEndpoint: TranslationTriple(
            "Invalid wireless pairing host or port.",
            "无线配对主机地址或端口无效。",
            "無線配對主機位址或連接埠無效。"
        ),
        .errorDaemonUnreachable: TranslationTriple(
            "Local ADB daemon (127.0.0.1:5037) is unreachable. Start 'adb start-server' and retry.",
            "无法连接本地 ADB 守护进程 (127.0.0.1:5037)，请确认已启动 adb 服务后重试。",
            "無法連線本地 ADB 守護行程 (127.0.0.1:5037)，請確認已啟動 adb 服務後重試。"
        ),
        .errorPinRejectedFormat: TranslationTriple(
            "Wireless pairing rejected by device (%1$@). Verify the 6-digit PIN and pairing port on your phone.",
            "设备拒绝无线配对（%1$@）。请核对手机弹窗上的 6 位配对码与配对端口是否过期或输入有误。",
            "裝置拒絕無線配對（%1$@）。請核對手機彈窗上的 6 位配對碼與配對連接埠是否過期或輸入有誤。"
        ),
        .errorConnectRejectedFormat: TranslationTriple(
            "Wireless connection failed after pairing (%1$@). Confirm Wireless Debugging remains enabled.",
            "无线连接建立失败（%1$@）。请确认手机“无线调试”保持开启。",
            "無線連線建立失敗（%1$@）。請確認手機「無線偵錯」保持開啟。"
        ),
        .errorDeviceUnauthorized: TranslationTriple(
            "Device is unauthorized. Please tap 'Allow' on your Android phone's debugging authorization prompt.",
            "设备尚未授权。请在 Android 手机弹出的调试授权窗口中点击“允许”。",
            "裝置尚未授權。請在 Android 手機彈出的偵錯授權視窗中點擊「允許」。"
        ),
        .errorDeviceOffline: TranslationTriple(
            "Device session is offline or disconnected. Unlock your phone and reconnect.",
            "设备会话处于离线或断开状态，请解锁手机并重新连接。",
            "裝置工作階段處於離線或中斷狀態，請解鎖手機並重新連線。"
        ),
        .errorConnectPortUnresolved: TranslationTriple(
            "Pairing succeeded, but no active wireless debugging session was found on the device.",
            "配对已完成，但未检测到可用的无线调试连接会话，请检查手机无线调试连接端口。",
            "配對已完成，但未偵測到可用的無線偵錯連線工作階段，請檢查手機無線偵錯連線連接埠。"
        ),
        .errorTransferDeviceDisconnected: TranslationTriple(
            "Device disconnected or transport unavailable during file transfer.",
            "文件传输过程中设备断开连接或传输通道不可用。",
            "檔案傳輸過程中裝置中斷連線或傳輸通道不可用。"
        ),
        .errorTransferIncompleteBytes: TranslationTriple(
            "Transfer interrupted before all bytes were verified. Partial file was cleaned up.",
            "传输在完成全部字节校验前中断，已清理未完成的临时文件。",
            "傳輸在完成全部位元組校驗前中斷，已清理未完成的暫存檔案。"
        ),
        .guideHeaderTitle: TranslationTriple(
            "Enable High-Speed ADB Channel",
            "开启高速 ADB 通道",
            "開啟高速 ADB 通道"
        ),
        .guideHeaderSubtitle: TranslationTriple(
            "Follow these steps on your Android phone to unlock full-speed parallel file transfers.",
            "目前处于标准 MTP 模式。在手机上完成以下步骤即可开启高速并发传输：",
            "目前處於標準 MTP 模式。在手機上完成以下步驟即可開啟高速並發傳輸："
        ),
        .guideStep1Title: TranslationTriple(
            "Open About Phone",
            "进入“关于手机”",
            "進入「關於手機」"
        ),
        .guideStep1Body: TranslationTriple(
            "Open Settings on your Android device and scroll to About phone.",
            "打开 Android 手机的“设置”，滑动到底部并点击“关于手机”。",
            "開啟 Android 手機的「設定」，滑動到底部並點擊「關於手機」。"
        ),
        .guideStep2Title: TranslationTriple(
            "Tap Build Number 7 Times",
            "连续点击“版本号” 7 次",
            "連續點擊「版本號碼」 7 次"
        ),
        .guideStep2Body: TranslationTriple(
            "Locate Build number (or OS version) and tap it 7 times until 'You are now a developer!' appears.",
            "找到“版本号”（或 MIUI/HyperOS/ColorOS 版本），连续点击 7 次直到提示“您已处于开发者模式”。",
            "找到「版本號碼」（或系統版本），連續點擊 7 次直到提示「您已成為開發人員」。"
        ),
        .guideStep3Title: TranslationTriple(
            "Enable USB Debugging",
            "开启“USB 调试”",
            "開啟「USB 偵錯」"
        ),
        .guideStep3Body: TranslationTriple(
            "Return to Settings > System > Developer options and turn on USB debugging.",
            "返回“设置 > 系统和更新 > 开发者选项”，找到并开启“USB 调试”开关。",
            "返回「設定 > 系統 > 開發人員選項」，找到並開啟「USB 偵錯」開關。"
        ),
        .guideStep4Title: TranslationTriple(
            "Authorize This Mac",
            "在手机弹窗中点击“允许”",
            "在手機彈窗中點擊「允許」"
        ),
        .guideStep4Body: TranslationTriple(
            "Check 'Always allow from this computer' on your phone's RSA key prompt and tap Allow.",
            "手机屏幕会弹出“允许 USB 调试吗？”对话框，勾选“始终允许使用这台计算机进行调试”并点击“允许”。",
            "手機畫面會彈出「允許 USB 偵錯嗎？」對話框，勾選「一律允許透過這台電腦進行偵錯」並點擊「允許」。"
        ),
        .guideProbeIdle: TranslationTriple(
            "Waiting for USB debugging authorization on your phone...",
            "正在等待手机端开启 USB 调试并完成授权...",
            "正在等待手機端開啟 USB 偵錯並完成授權..."
        ),
        .guideProbeProbing: TranslationTriple(
            "Checking authorized ADB device session...",
            "正在检测已授权的 ADB 设备会话...",
            "正在偵測已授權的 ADB 裝置工作階段..."
        ),
        .guideProbeActive: TranslationTriple(
            "Authorized ADB device connected! Ready for high-speed transfer.",
            "已检测到授权的 ADB 设备会话！可进行高速传输。",
            "已偵測到授權的 ADB 裝置工作階段！可進行高速傳輸。"
        ),
        .guideProbeDaemonOnlyNoDevice: TranslationTriple(
            "ADB service is running on Mac, but no authorized Android device session was detected. Check cable and phone prompt.",
            "Mac 本地 ADB 服务已运行，但未检测到已授权的 Android 设备会话。请检查数据线并在手机端点击“允许”。",
            "Mac 本地 ADB 服務已執行，但未偵測到已授權的 Android 裝置工作階段。請檢查傳輸線並在手機端點擊「允許」。"
        ),
        .guideProbeNotDetected: TranslationTriple(
            "ADB service or device not detected yet. Please verify steps 3 and 4 on your phone.",
            "暂未检测到 ADB 服务或已授权设备，请确认已完成步骤 3 和步骤 4。",
            "暫未偵測到 ADB 服務或已授權裝置，請確認已完成步驟 3 和步驟 4。"
        ),
        .guideContinueMtp: TranslationTriple(
            "Continue with MTP Mode",
            "继续使用标准 MTP 模式",
            "繼續使用標準 MTP 模式"
        ),
        .guideVerifyRetry: TranslationTriple(
            "Retry Detection",
            "重新检测",
            "重新偵測"
        ),
        .guideVerifyAction: TranslationTriple(
            "I've Enabled It, Verify Connection",
            "我已开启，验证连接",
            "我已開啟，驗證連線"
        ),
        .guideDoneAction: TranslationTriple(
            "Start High-Speed Transfer",
            "开始高速传输",
            "開始高速傳輸"
        )
    ]
}

public extension L10n {
    typealias AndroidInterconnect = AndroidInterconnectLocaleKey
}

public extension TTZipLocalizationManager {
    /// Resolves an `AndroidInterconnectLocaleKey` in the target or active language with catalog override + built-in fallback.
    func string(for key: AndroidInterconnectLocaleKey, language: AppLanguage? = nil) -> String {
        key.resolve(for: language, manager: self)
    }

    /// Formats an `AndroidInterconnectLocaleKey` template using the target or active language's `Locale`.
    func format(_ key: AndroidInterconnectLocaleKey, language: AppLanguage? = nil, _ args: CVarArg...) -> String {
        let targetLanguage = language ?? currentLanguage
        let template = key.resolve(for: targetLanguage, manager: self)
        return String(format: template, locale: Locale(identifier: targetLanguage.bcp47), arguments: args)
    }
}
