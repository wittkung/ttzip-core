# Quickstart Guide: 018 TTKit Unified Localization SDK

- **Feature Directory**: `specs/018-ttkit-unified-localization-sdk`
- **Status**: `Ready`
- **Created**: 2026-08-25

---

## 1. Quickstart Validation Scenarios

### Scenario A: Rust Core String Lookup & Formatting
```rust
use tt_i18n::{TTLocalizationEngine, AppLanguage, ByteSizeStandard};

fn main() {
    let engine = TTLocalizationEngine::new();
    
    // 1. O(1) Zero-alloc string lookup with fallback
    let ok_zh = engine.get_string("common.ok", AppLanguage::ZhHans);
    assert_eq!(ok_zh, "好");
    
    // 2. Localized byte formatting (SI with German comma delimiter)
    let formatted = engine.format_bytes(1_500_000, ByteSizeStandard::MetricSI, AppLanguage::De);
    assert_eq!(formatted, "1,5 MB");
    
    println!("Rust i18n Core validated successfully.");
}
```

### Scenario B: Swift 6 SwiftUI Presentation & Menu Sync
```swift
import SwiftUI
import TTLocalizationKit

@main
struct DemoApp: App {
    @State private var l10n = LocalizationState.shared
    
    var body: some Scene {
        WindowGroup {
            VStack(spacing: 16) {
                // Reactive Text Primitive
                L10nText(L10n.Common.ok)
                
                // Formatted Capacity
                Text(l10n.formatBytes(1_500_000_000))
                
                Button("Switch to Deutsch") {
                    l10n.setLanguage(.de)
                }
            }
            .padding()
        }
        .commands {
            TTLocalizationCommands()
        }
    }
}
```

### Scenario C: CI Automated Quality Gate
```bash
# 1. Verify 100% key parity across all 7 languages
cargo run -p tt-l10n-tools -- validate-parity --schema specs/018-ttkit-unified-localization-sdk/contracts/catalog-entry-contract.json

# 2. Check anti-fake translation thresholds (< 15% duplicate strings)
cargo run -p tt-l10n-tools -- validate-anti-fake --threshold 0.15

# 3. Fuzz format specifiers for positional parameter crash safety
cargo run -p tt-l10n-tools -- validate-format-specifiers
```
