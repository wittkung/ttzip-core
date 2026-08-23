# Quickstart: 151-multilingual-readme-matrix

## Validation Scenarios

### Scenario 1: Verify All 4 Localized README Files Exist & Have Language Bar
- **Command**: `head -n 5 README.md README_zh.md README_ja.md README_ko.md`
- **Expected Output**: Every file starts with the language switcher navigation bar.

### Scenario 2: Full Local CI Verification
- **Command**: `./scripts/local-ci.sh`
- **Expected Output**: `ALL LOCAL CI CHECKS PASSED SUCCESSFULLY (0 Quota)`
