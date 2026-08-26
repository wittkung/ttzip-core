# Feature Specification: Full Frontend Chinese & English Bilingual Localization (全面完成前端中英完整适配)

**Feature Branch**: `152-frontend-bilingual-localization`

**Created**: 2026-08-20

**Status**: Draft

**Input**: User description: "全面完成前端中英完整适配 /speckit-specify"

## Clarifications

### Session 2026-08-20
- Q: Which languages must have 100% full key parity in this phase? → A: English (`en`), Simplified Chinese (`zh-Hans`), and Traditional Chinese (`zh-Hant`) as core targets, with Japanese, German, French, Spanish falling back cleanly to English catalogs.
- Q: How should dynamic reactive UI updates be achieved across all views? → A: Views subscribe to `@ObservedObject private var l10n = AppLocalizationState.shared` and resolve strings via `l10n.t(...)` or `l10n.format(...)`, ensuring real-time instant re-render on language toggle without reloading windows.
- Q: What is the strategy for native AppKit menu and system alerts? → A: Synchronize AppKit `NSMainMenu` via `AppKitMenuSynchronizer` immediately on language mutation, and pass localized strings to `SystemNotificationManager` and `NSAlert`.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Real-Time Zero-Restart Language Switching (Priority: P1)

As a macOS user or international professional, I want to toggle between Simplified Chinese (简体中文), Traditional Chinese (繁體中文), and English (English) in TTZip preferences and have the entire user interface — including sidebar navigation, search bars, omnibars, compression/extraction sheets, inspector sidebars, previewers, benchmark dashboards, password vaults, preset workspaces, and system notifications — update immediately in real time (<10ms) without restarting the application or dropping current operation states.

**Why this priority**: Seamless multi-language adaptation without restarting is fundamental for native macOS usability and worldwide accessibility.

**Independent Test**:
Can be verified by opening TTZip, navigating through all tabs/modals (Sidebar, Explorer, Compress Modal, Extract Modal, Benchmark, Presets, Password Vault, Settings), selecting English and Chinese in Settings, and asserting that all visible UI strings, labels, placeholders, dialogs, and tooltips update immediately without restarting or UI glitching.

**Acceptance Scenarios**:

1. **Given** TTZip is running in Simplified Chinese mode, **When** the user switches to English in Settings > Localization, **Then** all top-level tabs, toolbar buttons, column headers, modal dialogues, action buttons, progress bars, and status captions instantly display in fluent English.
2. **Given** TTZip is running in English mode, **When** the user switches to Simplified Chinese (or Traditional Chinese), **Then** all UI elements immediately reflect authentic Chinese terminology without hardcoded English fallbacks or clipped text layouts.

---

### User Story 2 - Comprehensive Component String Coverage Across All Workspaces (Priority: P1)

As a user interacting with advanced features (File Explorer, 16-Format Benchmark, Password Keychain Vault, Custom Presets, and Media Previews), I want every UI view, context menu, error alert, and tooltip to be fully translated with contextually precise vocabulary, eliminating mixed-language (Chinglish / partial localization) interfaces and raw key leaks.

**Why this priority**: Eliminates visual inconsistency and poor user experience caused by mixed English/Chinese interfaces or hardcoded strings.

**Independent Test**:
Run UI/codebase audits across all 67+ Swift views in `Sources/TTZipApp/Views/` to ensure 100% of user-facing strings are resolved through the reactive `AppLocalizationState` / `TTZipLocalizationManager` catalog system, with zero untranslated hardcoded Chinese or English string literals.

**Acceptance Scenarios**:

1. **Given** an active archive inspection in ArchiveExplorerView, **When** examining file entries, breadcrumbs, search bars, file size/ratio badges, and metadata inspectors, **Then** all labels, units, and timestamps render in the active language.
2. **Given** a benchmark execution in BenchmarkView, **When** running speed tests across ZIP, 7Z, TAR.ZST, and ZSTD algorithms, **Then** hardware topology descriptions, throughput charts, speed dial gauges, competitor comparisons, and progress indicators render fully in the chosen language.
3. **Given** an error condition (e.g. encrypted archive with wrong password, corrupted header, disk full, permission denied), **When** an alert sheet or toast notification appears, **Then** the diagnostic message and recovery action buttons are fully localized in the active language.

---

### User Story 3 - macOS Native Menu Bar & System Notification Localization (Priority: P2)

As a macOS native user, I want the system menu bar (`NSMenu`), context menus (`NSContextMenu`), Dock progress captions, Finder sync extension menus, and macOS UserNotifications to dynamically align with the chosen language.

**Why this priority**: Provides full consistency with macOS platform design guidelines and system-level integrations.

**Independent Test**:
Trigger system notifications, open the macOS top menu bar, right-click file items in the outline view, and verify that all menu titles and notification texts reflect the current language setting.

**Acceptance Scenarios**:

1. **Given** the app language changes to English, **When** opening the macOS top application menus (App, File, Edit, View, Window, Help), **Then** `AppKitMenuSynchronizer` updates all standard and custom menu items to English.
2. **Given** a background compression task finishes, **When** `SystemNotificationManager` posts a completion notification, **Then** the notification title, subtitle, and body text match the user's selected language.

---

### Edge Cases

- **Missing Catalog Keys**: If a newly added UI key is accidentally missing in a target language catalog, the system MUST cascade fallback gracefully to English without crashing or displaying empty strings.
- **Dynamic String Formatting**: Strings with variable placeholders (`%@`, `%d`, `%.2f`, byte sizes, elapsed times) MUST correctly preserve parameter ordering and handle localized pluralization/number formatting.
- **System Locale Auto-Detection**: On first launch without prior user preference stored in `UserDefaults`, the application MUST automatically detect and adopt the macOS system preferred language (e.g., `zh-Hans-CN`, `zh-Hant-TW`, `en-US`).
- **Layout Truncation & Text Expansion**: English text is often longer than Chinese. All buttons, capsules, tabs, and form labels MUST have flexible auto-layout frames that prevent clipping, ellipses (`...`) truncation, or misaligned badges.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide strongly-typed, comprehensive enum namespaces under `L10n` for all frontend views: `L10n.Common`, `L10n.Sidebar`, `L10n.Explorer`, `L10n.Compress`, `L10n.Extract`, `L10n.Benchmark`, `L10n.Presets`, `L10n.Vault`, `L10n.Settings`, `L10n.Queue`, `L10n.Preview`, `L10n.Menu`, `L10n.Dialogs`, `L10n.Errors`, and `L10n.Units`.
- **FR-002**: System MUST maintain 1:1 complete key parity in string catalogs for English (`LocaleCatalog+En`), Simplified Chinese (`LocaleCatalog+ZhHans`), and Traditional Chinese (`LocaleCatalog+ZhHant`), with zero missing key gaps.
- **FR-003**: System MUST update all 67+ SwiftUI views in `Sources/TTZipApp/Views/` to resolve strings via `AppLocalizationState.shared.t(...)` or `AppLocalizationState.shared.format(...)` (or `@ObservedObject l10n`), eliminating hardcoded Chinese/English text and ad-hoc `isZh ? "..." : "..."` branching.
- **FR-004**: System MUST synchronize macOS AppKit main menu bar items dynamically through `AppKitMenuSynchronizer` when language changes.
- **FR-005**: System MUST localize all background notifications emitted by `SystemNotificationManager` and `DockProgressManager`.
- **FR-006**: System MUST localize all system alert dialogs, file overwrite confirmations, and password prompt sheets.
- **FR-007**: System MUST persist the selected language in `UserDefaults` (`TTZip_AppSelectedLanguage`) and load it synchronously on application initialization.
- **FR-008**: System MUST support dynamic language switching within < 10ms with zero memory leaks and zero UI state resets.
- **FR-009**: System MUST support localized formatting for byte sizes (e.g. `1.2 GB` / `1.2 吉字节` or standard binary units), compression ratios (e.g. `45.2%`), and throughput metrics (`1,250 MB/s`).
- **FR-010**: All UI components MUST be verified for visual typography elegance and zero text clipping in both English and Chinese modes according to `ttzip-ui-design-system` principles.

### Key Entities

- **`AppLanguage`**: Supported language enum (`.en`, `.zhHans`, `.zhHant`, `.ja`, `.de`, `.fr`, `.es`).
- **`LocaleKeyProtocol` / `L10n`**: Strongly typed hierarchical key definition system ensuring compile-time key verification.
- **`TTZipLocalizationManager`**: In-memory, thread-safe string catalog resolver with O(1) dictionary lookups and English cascade fallback.
- **`AppLocalizationState`**: `@MainActor` observable object bridging SwiftUI views to the core localization engine.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Zero hardcoded user-visible text literals remaining across all `Sources/TTZipApp/Views/` files.
- **SC-002**: 100% key parity across `LocaleCatalogEn`, `LocaleCatalogZhHans`, and `LocaleCatalogZhHant` with zero missing keys in automated audit tests.
- **SC-003**: Language switching latency is under 10ms across all open windows and sheets.
- **SC-004**: 100% of existing unit and regression tests pass (`swift test`) without regression.

## Assumptions

- Target OS is macOS 14.0+ on Apple Silicon and Intel x86_64.
- High-frequency inner loop compression/decompression algorithms are zero-overhead and independent of UI localization.
- Non-UI CLI logs will use `TTLogger` with CLI localized strings when run in interactive terminal environments.
