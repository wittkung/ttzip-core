# Quickstart & Verification Guide: 防御性安全与零分配加固验证

**Feature Directory**: `specs/038-libarchive-defensive-security-and-zero-allocation-hardening`  
**Date**: 2026-08-16  
**Status**: Ready for Validation

---

## 验证场景 1: 契约与 Schema 校验

### Command
```bash
python3 -c "
import glob, json
with open('specs/038-libarchive-defensive-security-and-zero-allocation-hardening/contracts/hardening_spec.json') as f:
    schema = json.load(f)
assert schema['properties']['spec_version']['enum'] == ['1.0.0']
assert len(schema['properties']['security_flags']['items']['enum']) == 4
assert schema['properties']['zero_allocation_enabled']['enum'] == [True]
assert schema['properties']['password_wipe_function']['enum'] == ['memset_s']
print('✓ Hardening Spec Contract Validated.')
"
```


### Expected Output
```text
✓ Hardening Spec Contract Validated.
```

---

## 验证场景 2: 代码加固项核对

### Command
```bash
python3 -c "
with open('Sources/CTTZipBridge/CTTZipBridge_Archive.c') as f:
    c_content = f.read()
assert 'ARCHIVE_EXTRACT_SECURE_SYMLINKS' in c_content
assert 'ARCHIVE_EXTRACT_SECURE_NOABSOLUTEPATHS' in c_content

with open('Sources/TTZipCore/SecurityScanner.swift') as f:
    swift_content = f.read()
assert 'sanitizePath' in swift_content

with open('Sources/TTZipCore/Adapters/LibdeflateCAdapter.swift') as f:
    adapter_content = f.read()
assert 'allocate(capacity:' in adapter_content

print('✓ All hardening implementation checkpoints validated.')
"
```

### Expected Output
```text
✓ All hardening implementation checkpoints validated.
```
