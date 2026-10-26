# Subtitle Font Selection Design & Verification Contract (2026-10-17)

**Feature Directory**: `products/ttzip/core/specs/230-unified-avsamplebuffer-media-pipeline`

**Status**: 实现完成；上一轮测试通过，本轮受并行修改影响，专项回归与 GUI 待验收

**Scope**:
- `infra/ttmedia/Sources/TTMediaKit/UI/Video/SubtitleStyleConfiguration.swift`
- `infra/ttmedia/Sources/TTMediaKit/UI/Video/SubtitleFontPicker.swift`
- `infra/ttmedia/Sources/TTMediaKit/UI/MediaLocalization.swift`
- `infra/ttmedia/Tests/TTMediaKitTests/SubtitleFontConfigurationTests.swift`

---

## 1. Problem Statement & Goals

Previously, `SubtitleOverlayView` hardcoded `Font.system(size: size, weight: weight, design: .rounded)`, providing only font size/scale adjustments while locking the typeface to the system rounded font. Users could not choose a local system font (e.g., PingFang SC, Hiragino Sans GB, Songti SC, Kaiti SC, Helvetica Neue, Menlo) as supported in IINA (`QuickSettingViewController` + `FontPickerWindowController`).

This design introduces native macOS subtitle font selection in `TTMediaKit` with the following guarantees:
1. **Explicit Separation of "Font" and "Font Size"**: Add a dedicated "Font" entry in `SubtitleQuickSettingsPopover` distinct from the "Font Size" / scale controls.
2. **IINA-Inspired Font Picker**: Provide a searchable family + typeface (member) dual-column picker with a quick-return "System Default" entry and a live bilingual (Chinese + English) preview card.
3. **Zero-Interruption Live Updates**: Selecting a font immediately updates primary and secondary (bilingual) subtitle overlays while preserving the secondary subtitle `0.82x` font size ratio, compact viewport scaling (`width < 380pt`), bold/italic traits, outline stroke, background box, and vertical position—without reloading the video, rescanning subtitles, or recreating `VideoPlaybackSession`.
4. **Backward-Compatible Persistence & Safe Fallback**: Persist `fontFamilyName` in `SubtitleStyleConfiguration` (`UserDefaults`). Legacy JSON payloads lacking `fontFamilyName` decode cleanly without resetting existing `fontSize`, `scale`, `textColor`, `verticalPosition`, or `delay`. Uninstalled, empty, or invalid font names fall back deterministically to the system rounded font with CJK cascade protection so Chinese characters never render as missing glyphs (`.notdef` tofu boxes).
5. **Controls Visibility Coordination**: Opening the font picker increments `MediaControlsVisibility` interaction depth (`beginInteraction()` / `endInteraction()`), preventing the floating control bar or popover from auto-hiding while the user browses or filters fonts.
6. **Shared `TTI18n` Localization**: All new UI strings resolve through `MediaLocalization` (`SubtitleFontLocaleKey` conforming to `FallbackLocaleKeyProtocol`) with built-in English, Simplified Chinese (`zh-Hans`), and Traditional Chinese (`zh-Hant`) translations.

---

## 2. Data Model & Backward-Compatible Persistence

### 2.1 `SubtitleStyleConfiguration` Schema Evolution

`SubtitleStyleConfiguration` gains an optional `fontFamilyName: String?` property (where `nil` represents the System Default rounded font):

```swift
public struct SubtitleStyleConfiguration: Equatable, Codable, Sendable {
    public static let defaultsKey = "com.metastudyline.ttmedia.subtitleStyleConfiguration"

    public var fontFamilyName: String?
    public var fontSize: Double
    public var scale: Double
    public var isBold: Bool
    public var isItalic: Bool
    public var textColor: SubtitleColorPreset
    public var borderWidth: Double
    public var backgroundOpacity: Double
    public var verticalPosition: Double
    public var delay: Double
}
```

### 2.2 Normalization & Legacy JSON Migration

- **Normalization**: `SubtitleFontResolver.normalizeFontIdentifier(_:)` trims leading/trailing whitespace and maps `nil`, `""`, `"system"`, `"default"`, or `"__system_default__"` (case-insensitive) to `nil` (System Default).
- **Backward-Compatible `init(from decoder: Decoder)`**:
  - Uses `decodeIfPresent` for every key so that legacy JSON payloads saved before `fontFamilyName` existed decode all existing fields (`fontSize`, `scale`, `isBold`, `isItalic`, `textColor`, `borderWidth`, `backgroundOpacity`, `verticalPosition`, `delay`) intact, defaulting only `fontFamilyName` to `nil`.
  - Passes decoded numeric values through the same clamping rules as `init(...)` so out-of-range values in edited or legacy payloads remain bounded.

---

## 3. Font Catalog, Resolution & CJK Fallback (`SubtitleFontPicker.swift`)

### 3.1 Catalog Discovery (`SubtitleFontCatalog`)

Modeled after IINA's `FontPickerWindowController`:
- Queries `NSFontManager.shared.availableFontFamilies`, filtering out hidden system dot-prefixed families (`hasPrefix(".")`) and blank strings.
- Resolves each family's localized display name via `NSFontManager.shared.localizedName(forFamily:face: nil)` and enumerates typeface members via `NSFontManager.shared.availableMembers(ofFontFamily:)`, capturing each member's PostScript name (`member[0]`), face name (`member[1]`), weight, and traits.
- Prepends a synthetic **System Default** option at the top of the catalog (with localized family label `subtitleFontSystemDefault` and localized typeface label `subtitleFontSystemDefaultFace`) so users can switch back to the system rounded font with a single click.
- Supports case-insensitive filtering across family name, localized family name, and member PostScript/face names.

### 3.2 Deterministic Font Resolution & CJK Cascade Protection (`SubtitleFontResolver`)

`SubtitleFontResolver.resolve(style:size:)` returns a `SubtitleResolvedFont` descriptor containing both the resolved `NSFont` (for metrics, glyph testing, and CoreText inspection) and the SwiftUI `Font` (for `SubtitleOverlayView`):
1. **System Default (`fontFamilyName == nil`)**:
   - Constructs the system rounded `NSFont` at `size` with `.bold` or `.medium` weight (and italic trait when `style.isItalic` is enabled) alongside `Font.system(size: size, weight: weight, design: .rounded)` (with `.italic()` if `style.isItalic` is true).
2. **Installed Custom Font (`fontFamilyName != nil`)**:
   - Attempts exact PostScript/font name lookup via `NSFont(name: identifier, size: size)`, falling back to the preferred member of `NSFontManager.shared.availableMembers(ofFontFamily: identifier)`.
   - Converts the resolved `NSFont` through `NSFontManager.shared.convert(_:toHaveTrait:)` for `.boldFontMask` (when `style.isBold`) and `.italicFontMask` (when `style.isItalic`).
   - Attaches the preferred CJK + Latin cascade descriptor list (`CTFontCopyDefaultCascadeListForLanguages` for `["zh-Hans", "zh-Hant", "ja", "en"]`), ensuring Latin-only fonts (e.g., `Menlo`, `Courier`, `Helvetica`) seamlessly fall back to system Chinese glyphs without rendering `.notdef` (`glyph == 0`) boxes.
   - Wraps the resulting `CTFont` into SwiftUI `Font(ctFont)` and applies `.bold()` / `.italic()` modifiers when the underlying font family lacks a dedicated bold or italic face (such as `PingFang SC` or `Kaiti SC` lacking a native oblique face), preserving Bold and Italic behavior across all fonts.
3. **Invalid / Uninstalled Font Fallback**:
   - If neither `NSFont(name:size:)` nor `availableMembers(ofFontFamily:)` matches `identifier`, `isFallbackToSystem` is marked `true` and resolution safely falls back to the System Default rounded font without throwing or mutating other style properties.

---

## 4. UI & Interaction Flow

### 4.1 `SubtitleQuickSettingsPopover` Font Entry

In `appearanceSection` of `SubtitleQuickSettingsPopover`, above the **Font Size** (`subtitleFontSize`) slider:
- Renders a dedicated **Font** (`subtitleFont`) row with the currently active font's localized display name (or "System Default (Rounded)" / "系统默认（圆体）" / "系統預設（圓體）" when `fontFamilyName == nil` or fallback is active) and a button that opens the font selection popover.
- Clearly separates **Font** (typeface family/member selection) from **Font Size** (point size + scale slider).

### 4.2 `SubtitleFontPickerView` (IINA-Style Picker with Bilingual Preview)

When the user clicks the Font button in `SubtitleQuickSettingsPopover`:
- Opens `SubtitleFontPickerView` (presented as a popover while holding `MediaControlsVisibility.beginInteraction()` until dismissal `endInteraction()`).
- **Search bar**: Filters font families by localized name, English family name, or PostScript name.
- **Two-column browser**:
  - Left column: **Family** (`字体集` / `字體集` / `Family`), starting with `System Default` followed by all installed families.
  - Right column: **Typeface** (`字型` / `樣式` / `Typeface`), listing localized `subtitleFontSystemDefaultFace` (`常规／圆体` / `常規／圓體` / `Regular / Rounded`) for System Default or available faces (`Regular`, `Medium`, `Bold`, etc.) for the selected installed family.
- **Bilingual Live Preview Card**:
  - Renders both Chinese and English sample text (`"中文字幕预览 · Subtitle Font Preview"`) using the selected font, current bold/italic/color/outline/background settings, and primary + secondary (`0.82x`) dual-track preview sizing.
- **Immediate Application (No Playback Session Mutation or Extra OSD)**:
  - Selecting a family or face immediately invokes `onUpdateStyle(updated)` -> `VideoPlaybackSession.updateSubtitleStyle(_:)`, updating `playback.subtitleStyle`, saving to `UserDefaults`, and refreshing `SubtitleOverlayView`, the quick settings font label, and the picker's bilingual preview card in real time without touching `activeController`, `currentTime`, `isPlaying`, or subtitle track caches.
  - `VideoPlaybackSession.updateSubtitleStyle(_:)` remains unmodified and continues to show OSD badges only for `delay` and `scale` changes; font changes provide direct visual feedback in the bilingual preview card and live video subtitle overlay.

---

## 5. Non-Goals & Guardrails

- Does **not** download or install external fonts.
- Does **not** implement a full `libass` vector layout engine.
- Does **not** modify Rust (`infra/ttmedia/src/*`), XPC Worker, `VideoPlaybackSession` lifecycle, archive engine, or `BUILD.bazel`.
