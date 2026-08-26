# Quickstart Guide: TTZip Full Multilingual SDK Testing System

- **Feature ID**: `006-multi-language-sdk-automated-testing-framework`
- **Created**: 2026-08-24

---

## 1. Quick Test Execution (All 9 SDKs)

To run the complete automated test matrix across all 9 language ecosystems:

```bash
# Run all SDK tests with stdout diagnostics
bash core/scripts/run_sdk_test_matrix.sh

# Or run via Makefile
make test-all-sdk
```

---

## 2. Targeted Execution by Language or Category

```bash
# Run only Python and Go SDK tests
bash core/scripts/run_sdk_test_matrix.sh --sdk=python,go

# Run only Cross-Language Interoperability Matrix
bash core/scripts/run_sdk_test_matrix.sh --category=interop

# Run only Security & Zip Slip Defense Gates
bash core/scripts/run_sdk_test_matrix.sh --category=security
```

---

## 3. Exporting JSON & JUnit XML Reports for CI/CD

```bash
# Export structured JSON matrix report
bash core/scripts/run_sdk_test_matrix.sh --json /tmp/ttzip-sdk-report.json

# Export JUnit XML test report for Jenkins / GitHub Actions
bash core/scripts/run_sdk_test_matrix.sh --junit /tmp/junit-reports/
```

---

## 4. Running ASan/TSan Sanitizers Gate

```bash
# Execute native memory leak and thread race tests
bash core/scripts/run_sanitizers.sh
```
