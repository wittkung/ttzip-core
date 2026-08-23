# Data Model: 198-modernize-multilingual-readmes-and-homebrew-formulas

## 1. Documentation & Distribution Entities
```
Multilingual Documentation:
  ├── README.md            # English (Default)
  ├── README_zh.md         # Simplified Chinese
  ├── README_ja.md         # Japanese
  └── README_ko.md         # Korean

Homebrew Distribution:
  ├── Formula/ttzip-cli.rb # Official CLI tool formula
  └── Formula/ttzip.rb     # Alias formula for `brew install ttzip`
```
