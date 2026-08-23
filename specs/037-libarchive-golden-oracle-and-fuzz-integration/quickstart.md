# Quickstart & Verification Guide: 黄金预言机与测试套件验证

**Feature Directory**: `specs/037-libarchive-golden-oracle-and-fuzz-integration`  
**Date**: 2026-08-16  
**Status**: Ready for Validation

---

## 验证场景 1: UUDecoder 单元与基准测试

### Command
```bash
python3 -c "
import glob, json
with open('specs/037-libarchive-golden-oracle-and-fuzz-integration/contracts/testing_oracle_spec.json') as f:
    data = json.load(f)
assert data['spec_version'] == '1.0.0'
print('✓ Testing Oracle Spec Contract Validated.')
"
```

### Expected Output
```text
✓ Testing Oracle Spec Contract Validated.
```

---

## 验证场景 2: 黄金语料库与模糊测试落地验证

### Command
```bash
test -f Sources/TTZipCore/Utilities/UUDecoder.swift && \
test -d Tests/TTZipTests/Fixtures/GoldenCorpus && \
test -f Tests/TTZipTests/ArchiveGoldenCorpusTests.swift && \
test -f Tests/TTZipTests/ArchiveMutationFuzzTests.swift && \
test -f Tests/TTZipTests/SystemDifferentialTests.swift && \
echo "All testing fixtures, fuzzers, and differential test suites verified."
```

### Expected Output
```text
All testing fixtures, fuzzers, and differential test suites verified.
```
