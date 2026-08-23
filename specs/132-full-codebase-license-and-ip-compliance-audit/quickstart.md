# Quickstart & Validation Guide: Feature 132

## Verification Scenarios

### Scenario 1: Full-Codebase SPDX Header Compliance
- **Command**: `python3 scripts/audit_licenses.py --check-headers --dir Sources`
- **Expected Output**: `[PASS] 100% of proprietary source files have valid SPDX headers.`
- **Failure Diagnostic**: If files fail, check line 1 of the flagged file for `// SPDX-License-Identifier: LicenseRef-TTZip-Source-Available-1.0`.

### Scenario 2: Third-Party License Harvesting & Documentation
- **Command**: `python3 scripts/generate_acknowledgements.py --output docs/THIRD_PARTY_LICENSES.md`
- **Expected Output**: `[SUCCESS] Harvested 6 upstream dependency licenses into docs/THIRD_PARTY_LICENSES.md.`
- **Failure Diagnostic**: Ensure `Vendor/` directory contains clean checkouts with original `LICENSE` files.

### Scenario 3: Copyleft & GPL Viral Linkage Immunity Audit
- **Command**: `python3 scripts/audit_licenses.py --check-copyleft`
- **Expected Output**: `[PASS] 0 viral GPL/AGPL static dependencies detected. All dependencies are permissive or weak-copyleft compliant.`
- **Failure Diagnostic**: Review `Package.swift` linked libraries.
