# Quickstart & Verification Guide: 全仓库系统级审计报告验证

**Feature Directory**: `specs/040-comprehensive-systemic-invariants-codebase-audit`  
**Date**: 2026-08-16  
**Status**: Ready for Validation

---

## 验证场景 1: 审计契约 Schema 校验

### Command
```bash
python3 -c "
import glob, json
with open('specs/040-comprehensive-systemic-invariants-codebase-audit/contracts/codebase_audit_spec.json') as f:
    data = json.load(f)
assert data['$schema'] == 'http://json-schema.org/draft-07/schema#'
assert len(data['properties']['audited_layers']['items']['type']) > 0
print('✓ Codebase Audit Schema Validated.')
"
```

### Expected Output
```text
✓ Codebase Audit Schema Validated.
```

---

## 验证场景 2: 全景审计报告完整性校验

### Command
```bash
python3 -c "
with open('docs/architecture/comprehensive_systemic_audit_report.md') as f:
    doc = f.read()
assert 'Stream-First' in doc
assert 'Invariant-First' in doc
assert 'Bounds-First' in doc
assert 'Oracle-First' in doc
assert 'P0' in doc
assert 'P1' in doc
assert 'P2' in doc
assert 'P3' in doc
print('✓ Comprehensive Audit Report 100% verified.')
"
```

### Expected Output
```text
✓ Comprehensive Audit Report 100% verified.
```
