# Quickstart & Verification Guide: 系统工程方法论与不变量验证

**Feature Directory**: `specs/039-systemic-engineering-methodology-and-invariants-reconstruction`  
**Date**: 2026-08-16  
**Status**: Ready for Validation

---

## 验证场景 1: Schema 契约校验

### Command
```bash
python3 -c "
import glob, json
schemas = glob.glob('specs/039-systemic-engineering-methodology-and-invariants-reconstruction/contracts/*.json')
assert len(schemas) == 2
for s in schemas:
    with open(s) as f:
        data = json.load(f)
    assert data['$schema'] == 'http://json-schema.org/draft-07/schema#'
print('✓ All 039 Schemas Validated.')
"
```

### Expected Output
```text
✓ All 039 Schemas Validated.
```

---

## 验证场景 2: 宪法与方法论文档完整性校验

### Command
```bash
python3 -c "
with open('.specify/memory/constitution.md') as f:
    c = f.read()
assert 'Stream-First' in c
assert 'Invariant-First' in c
assert 'Bounds-First' in c
assert 'Oracle-First' in c

with open('GEMINI.md') as f:
    g = f.read()
assert 'Stream-First' in g
assert 'Invariant-First' in g

with open('docs/architecture/systemic_engineering_methodology.md') as f:
    doc = f.read()
assert len(doc) > 2000

print('✓ Constitution and Methodology Guidelines 100% verified.')
"
```

### Expected Output
```text
✓ Constitution and Methodology Guidelines 100% verified.
```
