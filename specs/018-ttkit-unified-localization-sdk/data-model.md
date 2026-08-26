# Data Model & Entity Specifications: 018 TTKit Unified Localization SDK

- **Feature Directory**: `specs/018-ttkit-unified-localization-sdk`
- **Status**: `Draft`
- **Created**: 2026-08-25

---

## 1. Core Entities & Enums

### 1.1 `AppLanguage` (Core Language Identifier)
Represents standardized IETF BCP-47 language tags supported across all SDK clients.

| Value | BCP-47 Code | English Name | Native Display Name |
| :--- | :--- | :--- | :--- |
| `En` | `en` | English | English |
| `ZhHans` | `zh-Hans` | Simplified Chinese | 简体中文 |
| `ZhHant` | `zh-Hant` | Traditional Chinese | 繁體中文 |
| `Ja` | `ja` | Japanese | 日本語 |
| `De` | `de` | German | Deutsch |
| `Fr` | `fr` | French | Français |
| `Es` | `es` | Spanish | Español |

### 1.2 `ByteSizeStandard` (Capacity Formatting Mode)
- `MetricSI`: Decimal powers ($10^3 = 1000$ B = 1 KB, 1 MB, 1 GB, 1 TB).
- `BinaryIEC`: Binary powers ($2^{10} = 1024$ B = 1 KiB, 1 MiB, 1 GiB, 1 TiB).

### 1.3 `PluralCategory` (CLDR Pluralization Rules)
Conforms to Unicode CLDR Plural Rules:
- `Zero`, `One`, `Two`, `Few`, `Many`, `Other`.

---

## 2. Catalog & Schema Data Structures

### 2.1 `CatalogContract` (Single Source of Truth)
```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "namespace": "common",
  "entries": {
    "cancel": {
      "translations": {
        "en": "Cancel",
        "zh-Hans": "取消",
        "zh-Hant": "取消",
        "ja": "キャンセル",
        "de": "Abbrechen",
        "fr": "Annuler",
        "es": "Cancelar"
      },
      "description": "Standard modal/dialog dismissal action",
      "placeholders": []
    }
  }
}
```

### 2.2 `MenuTopologyNode` (AppKit Dynamic Menu Model)
- `tag`: `Int` (Unique immutable identifier, e.g. 1001 for About, 1101 for New Archive)
- `actionSelector`: `String?` (Cocoa selector signature, e.g. `"orderFrontStandardAboutPanel:"`)
- `slotIndex`: `Int?` (Fallback structural position in `NSMenu.items`)
- `localeKey`: `String` (Contract lookup key, e.g. `"menu.about"`)

### 2.3 `LocalizationChangeEvent` (Darwin / AppGroup IPC Payload)
- `language`: `String` (BCP-47 code)
- `byteStandard`: `String` (`"MetricSI"` | `"BinaryIEC"`)
- `timestamp`: `Int64` (Unix epoch milliseconds)
- `sourceBundleId`: `String` (Originating bundle identifier, e.g. `"com.ttzip.app"`)
