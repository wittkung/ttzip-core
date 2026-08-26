# Data Model: Full Frontend Chinese & English Bilingual Localization

## Overview
This document defines the strict data models and invariants for TTZip's frontend localization architecture across TTZipCore and TTZipApp.

---

## 1. Core Enumerations & Protocols

### 1.1 AppLanguage
Supported language identifiers with BCP-47 mappings and POSIX environment parsing.

| Field / Property | Type | Description | Invariants & Constraints |
| :--- | :--- | :--- | :--- |
| `rawValue` | `String` | Unique identifier key | Must be one of: `en`, `zh-Hans`, `zh-Hant`, `ja`, `de`, `fr`, `es` |
| `displayName` | `String` | End-user localized language label | Native script (e.g. "English", "简体中文", "繁體中文", "日本語", "Deutsch", "Français", "Español") |
| `bcp47` | `String` | RFC 5646 / BCP-47 language tag | Standard tag (e.g. "en-US", "zh-Hans-CN", "zh-Hant-TW") |
| `locale` | `Locale` | Foundation `Locale` instance | Instantiated directly from `bcp47` identifier |

### 1.2 LocaleKeyProtocol & Namespaces
Strongly-typed hierarchical keys for compile-time verified string resolution.

| Property | Type | Description | Invariants & Constraints |
| :--- | :--- | :--- | :--- |
| `rawKey` | `String` | Dot-separated canonical key identifier | Non-empty, format `^[a-z]+(\.[a-z0-9_]+)+$` (e.g. `sidebar.home_and_extract`) |

#### Namespaces
1. `L10n.Common`: Shared verbs, confirmations, and status indicators (`ok`, `cancel`, `save`, `delete`, `processing`, etc.)
2. `L10n.Sidebar`: Navigation items, headers, and badge labels (`homeAndExtract`, `newArchive`, `presets`, `benchmark`, `vault`, `settings`, `queue`, etc.)
3. `L10n.Explorer`: Directory browser, column headers, sorting, QuickLook, and file metadata (`columnsView`, `sortByName`, `sortBySize`, etc.)
4. `L10n.Compress`: Compression modal options, algorithms, levels, solid archive, CPU threads, and summary stats (`format`, `level`, `splitVolume`, etc.)
5. `L10n.Extract`: Extraction destination, password prompts, overwrite conflicts, and success notifications (`extractHere`, `toSubfolder`, etc.)
6. `L10n.Benchmark`: Throughput metrics, compression ratios, hardware topology, competitor comparisons, and progress passes (`throughput`, `speedup`, etc.)
7. `L10n.Presets`: Custom preset cards, creation, duplicate, reset defaults, and pro configurations (`title`, `createNew`, `resetDefaults`, etc.)
8. `L10n.Vault`: Password keychain entries, PBKDF2 iterations, biometric unlocks, and vault status (`unlockPrompt`, `addPassword`, etc.)
9. `L10n.Settings`: General preferences, smart store bypass, byte units (SI vs IEC), licensing status, and Apple Silicon topology (`general`, `language`, etc.)
10. `L10n.Queue`: Concurrent task queue, pause/resume all, individual cancel, and live progress (`activeTasks`, `pauseAll`, etc.)
11. `L10n.Preview`: Media, text, code syntax, EPUB, PDF, and audio waveform inspector (`loading`, `fullScreen`, `pageCount`, etc.)
12. `L10n.Menu`: AppKit macOS top-level menu bar, submenus, and Finder context actions (`about`, `quit`, `finderExtractHere`, etc.)
13. `L10n.Dialogs`: System confirmation dialogs, overwrite warnings, and destructive action sheets (`confirmDeleteTitle`, `overwriteMessage`, etc.)
14. `L10n.Errors`: Detailed POSIX, format corruption, CRC mismatch, Zip Slip vulnerability, and encryption errors (`corruptedHeader`, `crcMismatch`, etc.)
15. `L10n.Units`: Units of measurement, time units, throughput rates, and plural counters (`bytes`, `mbPerSec`, `itemsCount`, etc.)
16. `L10n.CLI`: Standalone command line usage, flag descriptions, and terminal test summaries (`usageHeader`, `subcommands`, etc.)

---

## 2. In-Memory Catalog Models

### 2.1 StringCatalogDictionary
Pure Swift in-memory key-value dictionary compiled into the Mach-O data segment.

| Field | Type | Description | Invariants & Constraints |
| :--- | :--- | :--- | :--- |
| `strings` | `[String: String]` | Raw key to localized string mapping | 100% key parity with `L10n.allRawKeys`; zero empty or nil values |

---

## 3. Formatting Models & Options

### 3.1 ByteSizeStyle
| Option | Base Radix | Unit Names | Standard Reference |
| :--- | :--- | :--- | :--- |
| `.metricSI` | 1000 | KB, MB, GB, TB | IEEE 1541 / macOS standard |
| `.binaryIEC` | 1024 | KiB, MiB, GiB, TiB | IEC 80000-13 |

### 3.2 PluralCategory
| Value | Unicode CLDR Name | Applicable Languages |
| :--- | :--- | :--- |
| `.zero` | zero | Arabic, Latvian |
| `.one` | one | English, German, French, Spanish |
| `.two` | two | Arabic, Welsh |
| `.few` | few | Polish, Russian, Czech |
| `.many` | many | Polish, Russian, Hebrew |
| `.other` | other | Chinese, Japanese, Korean (universal), English (count != 1) |

---

## 4. AppKit & System Event Models

### 4.1 MenuSyncItem
| Field | Type | Description |
| :--- | :--- | :--- |
| `selectorName` | `String` | AppKit Objective-C action selector (e.g. `orderFrontStandardAboutPanel:`) |
| `targetKey` | `LocaleKeyProtocol` | Strongly-typed localization key |
| `preserveShortcut` | `Bool` | True to guarantee `keyEquivalent` remains untouched |

### 4.2 LocalizedNotificationPayload
| Field | Type | Description |
| :--- | :--- | :--- |
| `titleKey` | `LocaleKeyProtocol` | Notification title key |
| `bodyTemplateKey` | `LocaleKeyProtocol` | Notification body template key with format specifiers |
| `arguments` | `[CVarArg]` | Positional formatting arguments |
