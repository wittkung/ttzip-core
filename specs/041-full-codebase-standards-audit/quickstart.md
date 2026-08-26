# Quickstart: 全代码库规范审计与契约验证指南 (Quickstart & Validation Guide)

**Feature Directory**: `specs/041-full-codebase-standards-audit`  
**Created**: 2026-08-17  
**Status**: Ready

---

## 1. 契约 Schema 校验 (JSON Schema Validation)

验证审计规范契约是否符合 Draft-07 标准并无裸通配类型。

### 场景 1: 验证 `codebase_audit_spec.json` 语法与元规范
- **Command**:
  ```bash
  python3 -c "import json; schema = json.load(open('specs/041-full-codebase-standards-audit/contracts/codebase_audit_spec.json')); print('Schema Title:', schema['title']); assert schema['\$schema'] == 'http://json-schema.org/draft-07/schema#'; assert 'properties' in schema; print('Validation: PASS')"
  ```
- **Expected Output**:
  ```text
  Schema Title: CodebaseAuditReportSpec
  Validation: PASS
  ```
- **Failure Diagnostic**: 若报错 `AssertionError` 或 `KeyError`，检查 JSON 文件格式是否缺少 Draft-07 `$schema` 字段或顶层属性未按规范定义。

---

## 2. 缺陷全景报告完整性核验 (Audit Report Integrity)

验证综合审计报告是否完整包含 41 项缺陷条目、涵盖 P0/P1/P2/P3 四级矩阵与四阶段路线图。

### 场景 2: 验证全景报告存在性与缺陷结构
- **Command**:
  ```bash
  python3 -c "import os, re; path = 'docs/architecture/comprehensive_systemic_audit_report.md'; assert os.path.exists(path), 'Report missing'; content = open(path).read(); assert 'P0-01' in content, 'P0 missing'; assert 'P1-01' in content, 'P1 missing'; print('Audit Report Grounded: PASS')"
  ```
- **Expected Output**:
  ```text
  Audit Report Grounded: PASS
  ```
- **Failure Diagnostic**: 若文件不存在，检查 `docs/architecture/comprehensive_systemic_audit_report.md` 是否已正确落盘。

---

## 3. 全量测试与性能门禁回归验证 (Regression & Performance Gate)

### 场景 3: 运行全量单元测试断言基线
- **Command**:
  ```bash
  swift test
  ```
- **Expected Output**:
  ```text
  Executed 525+ tests, with 0 failures
  ```
- **Failure Diagnostic**: 若测试失败，查看具体失败测试套件名（如 `ArchiveReaderTests`），定位是否由未解冻代码或环境配置导致。
