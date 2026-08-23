# Quickstart Validation Guide: Repository Hygiene and Governance Standards

**Feature Branch**: `077-repository-hygiene-and-standards`
**Date**: 2026-08-18
**Status**: Complete

This document provides runnable physical validation scenarios that verify the complete repository hygiene, GitHub issue forms, PR verification templates, gitignore/gitattributes rules, and offline pre-flight quality verification script.

---

## Scenario 1: Validate GitHub Issue Forms & PR Template Schema Compliance

Validates that all `.github/ISSUE_TEMPLATE/*.yml`, `config.yml`, and `.github/pull_request_template.md` exist and comply with the defined schema constraints.

### Command
```bash
python3 -c '
import os, yaml

templates = [
    ".github/ISSUE_TEMPLATE/bug_report.yml",
    ".github/ISSUE_TEMPLATE/performance_regression.yml",
    ".github/ISSUE_TEMPLATE/feature_request.yml",
    ".github/ISSUE_TEMPLATE/config.yml"
]

for t in templates:
    assert os.path.exists(t), f"Missing template: {t}"
    with open(t, "r") as f:
        data = yaml.safe_load(f)
        assert data is not None, f"Failed to parse YAML: {t}"
    print(f"✅ Valid YAML form: {t}")

assert os.path.exists(".github/pull_request_template.md"), "Missing PR template"
with open(".github/pull_request_template.md", "r") as f:
    pr_content = f.read()
    assert "Performance Floor & Zero-Regression Gate" in pr_content
    assert "Swift 6 Concurrency & Toolchain Gate" in pr_content
    assert "Sanitizers Security Matrix (ASan & TSan)" in pr_content
    assert "Benchmark Differential Comparison Table" in pr_content
print("✅ Valid PR template with all mandatory verification gates")
'
```

### Expected Output
```text
✅ Valid YAML form: .github/ISSUE_TEMPLATE/bug_report.yml
✅ Valid YAML form: .github/ISSUE_TEMPLATE/performance_regression.yml
✅ Valid YAML form: .github/ISSUE_TEMPLATE/feature_request.yml
✅ Valid YAML form: .github/ISSUE_TEMPLATE/config.yml
✅ Valid PR template with all mandatory verification gates
```

### Failure Diagnostic
- If YAML parsing fails, check for unescaped colons, improper indentation, or invalid characters.
- If PR template assertion fails, verify that all 5 verification gates and the benchmark table exist in `.github/pull_request_template.md`.

---

## Scenario 2: Validate Repository Cleanliness & `.gitignore` / `.gitattributes` Rules

Validates that no transient build directories (`.build_custom`, `.build_di_test`, `.build_tmp`, `dist`, `build`, `build_dist`), vendor upstream clones (`Vendor/zlib-ng-upstream/`), or `.DS_Store` are tracked or shown in `git status`.

### Command
```bash
python3 -c '
import os, subprocess

# 1. Assert .gitignore covers key paths
with open(".gitignore", "r") as f:
    gi = f.read()
    assert ".build_*/" in gi or ".build/" in gi, "Missing .build ignore"
    assert "Vendor/*-upstream/" in gi, "Missing vendor upstream ignore"
    assert ".DS_Store" in gi, "Missing .DS_Store ignore"
print("✅ .gitignore contains all necessary exclusion rules")

# 2. Assert .gitattributes has LF normalization and Linguist overrides
with open(".gitattributes", "r") as f:
    ga = f.read()
    assert "* text=auto eol=lf" in ga, "Missing EOL LF normalization"
    assert "Vendor/** linguist-vendored" in ga, "Missing Linguist vendor override"
    assert "specs/** linguist-generated" in ga, "Missing Linguist generated override"
print("✅ .gitattributes contains EOL normalization and Linguist overrides")

# 3. Check for any stray .DS_Store
res = subprocess.run(["find", ".", "-name", ".DS_Store", "-not", "-path", "./.git/*"], capture_output=True, text=True)
if res.stdout.strip():
    print(f"⚠️ Stray .DS_Store found:\n{res.stdout}")
else:
    print("✅ Zero stray .DS_Store in repository tree")
'
```

### Expected Output
```text
✅ .gitignore contains all necessary exclusion rules
✅ .gitattributes contains EOL normalization and Linguist overrides
✅ Zero stray .DS_Store in repository tree
```

### Failure Diagnostic
- If `Vendor/*-upstream/` is not ignored, ensure `.gitignore` has the glob pattern `Vendor/*-upstream/`.
- If stray `.DS_Store` files are detected, remove them using `find . -name ".DS_Store" -delete`.

---

## Scenario 3: Validate CI/CD Workflow Quota Protection

Validates that `.github/workflows/ci-cd.yml` has automated `push` and `pull_request` triggers disabled (commented out or omitted) and only retains `workflow_dispatch`.

### Command
```bash
python3 -c '
import yaml

with open(".github/workflows/ci-cd.yml", "r") as f:
    data = yaml.safe_load(f)

# Extract triggers (handling PyYAML boolean parsing for unquoted `on`)
on_triggers = data.get("on") or data.get(True) or {}
assert "workflow_dispatch" in on_triggers, "workflow_dispatch trigger is missing"
assert "push" not in on_triggers, "Automated push trigger is enabled! Quota risk!"
assert "pull_request" not in on_triggers, "Automated pull_request trigger is enabled! Quota risk!"
print("✅ CI/CD trigger is strictly workflow_dispatch (0 automatic quota minutes consumed)")
'
```

### Expected Output
```text
✅ CI/CD trigger is strictly workflow_dispatch (0 automatic quota minutes consumed)
```

### Failure Diagnostic
- If `push` or `pull_request` triggers are active in `.github/workflows/ci-cd.yml`, comment them out or remove them under the `on:` block.

---

## Scenario 4: Execute Full Local Pre-Flight Verification Script

Executes the standalone, single-command offline verification script to validate repository hygiene, invariant linting, parallel unit tests, and performance gates.

### Command
```bash
./scripts/pre_flight_check.sh
```

### Expected Output
```text
================================================================================
TTZip Local Pre-Flight Quality Gate & Repository Hygiene Verification
================================================================================
[Stage 1/4] Checking Repository Cleanliness & Git Hygiene...
✅ Repository Cleanliness Gate: PASSED (0.05s)

[Stage 2/4] Running Codebase Invariant & Formatting Lint...
✅ Codebase Invariant Gate: PASSED (0.42s)

[Stage 3/4] Running Fast Parallel Unit & Pattern Test Suite...
✅ Unit Test Suite Gate: PASSED (584+ tests, 8.12s)

[Stage 4/4] Verifying Core Engine Performance Floor Gates...
✅ Core Performance Floor Gate: PASSED (11.34s)

================================================================================
PRE-FLIGHT QUALITY GATE SUMMARY
================================================================================
Stage 1: Repository Hygiene           [ PASS ] (0.05s)
Stage 2: Codebase Invariant Lint      [ PASS ] (0.42s)
Stage 3: Unit & Pattern Test Suite    [ PASS ] (8.12s)
Stage 4: Core Performance Floor Gate  [ PASS ] (11.34s)
--------------------------------------------------------------------------------
Overall Status: PASSED (Total Duration: 19.93s)
================================================================================
```

### Failure Diagnostic
- If Stage 1 fails: check for unstaged untracked temporary files or `.DS_Store`.
- If Stage 2 fails: inspect invariant violations output by `scripts/lint_codebase_invariants.py`.
- If Stage 3 fails: run `swift test` to identify the failing unit test.
- If Stage 4 fails: run `swift test --filter XCTestPerformanceMeasureTests` to check if a performance threshold fell below the historical minimum floor.
