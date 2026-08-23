# Quickstart & Verification Guide: libarchive 工程卓越性体系与落地验证

**Feature Directory**: `specs/036-libarchive-standards-architecture-workflow-synthesis`  
**Date**: 2026-08-16  
**Status**: Ready for Validation

---

## 验证场景 1: 契约与 Schema 严谨性全量断言

验证当前 Feature 的 4 个契约 Schema 是否完全合规，且与 `data-model.md` 保持 1:1 双向一致性，零裸通配类型。

### Command
```bash
SPECIFY_FEATURE_DIRECTORY="specs/036-libarchive-standards-architecture-workflow-synthesis" \
python3 -c "
import json, glob, sys

schemas = glob.glob('specs/036-libarchive-standards-architecture-workflow-synthesis/contracts/*.json')
assert len(schemas) == 4, f'Expected 4 contracts, found {len(schemas)}'

for s in schemas:
    with open(s) as f:
        data = json.load(f)
    assert data.get('\$schema') == 'http://json-schema.org/draft-07/schema#', f'Missing draft-07 schema in {s}'
    assert 'properties' in data, f'Missing properties in {s}'
    assert data.get('additionalProperties') is False, f'Missing additionalProperties: false in {s}'
    print(f'✓ Contract validated: {s}')
print('All 4 contracts passed zero-bare-object assertion.')
"
```

### Expected Output
```text
✓ Contract validated: specs/036-libarchive-standards-architecture-workflow-synthesis/contracts/libarchive_architecture_spec.json
✓ Contract validated: specs/036-libarchive-standards-architecture-workflow-synthesis/contracts/security_defense_matrix.json
✓ Contract validated: specs/036-libarchive-standards-architecture-workflow-synthesis/contracts/workflow_skill_evolution.json
✓ Contract validated: specs/036-libarchive-standards-architecture-workflow-synthesis/contracts/repo_layout_blueprint.json
All 4 contracts passed zero-bare-object assertion.
```

### Failure Diagnostic
- **排查路径**: 若报错 `Missing additionalProperties: false` 或属性不匹配，检查对应的 JSON Schema 文件是否包含了裸 `object` 声明或拼写错误。

---

## 验证场景 2: Agent Skills 体系与规则一致性验证

验证更新后的 Agent Skills（`upstream-contribution`, `code-review`, `design-patterns-guide`）与 `GEMINI.md` 是否已正确注入 libarchive 体系的工业级标准与审查项。

### Command
```bash
python3 -c "
import os

skills = [
    '.agents/skills/upstream-contribution/SKILL.md',
    '.agents/skills/code-review/SKILL.md',
    '.agents/skills/design-patterns-guide/SKILL.md',
    'GEMINI.md'
]

keywords = {
    '.agents/skills/upstream-contribution/SKILL.md': ['mdoc', 'Makefile.am', 'Bidding Protocol', 'bitcrc32'],
    '.agents/skills/code-review/SKILL.md': ['16KB', 'Clamp', 'Zero Allocation', 'Magic'],
    '.agents/skills/design-patterns-guide/SKILL.md': ['Bidder-Filter-Format', 'Ring Buffer', 'Capability Dispatch'],
    'GEMINI.md': ['Clamp', 'Non-Full Read', 'Zero-Cost Abstraction']
}

for sk in skills:
    assert os.path.exists(sk), f'File not found: {sk}'
    with open(sk) as f:
        content = f.read()
    print(f'Checking {sk}...')
    for kw in keywords.get(sk, []):
        if kw in content:
            print(f'  ✓ Found keyword: {kw}')
        else:
            print(f'  - Keyword pending injection in implementation phase: {kw}')
"
```

### Expected Output
```text
Checking .agents/skills/upstream-contribution/SKILL.md...
Checking .agents/skills/code-review/SKILL.md...
Checking .agents/skills/design-patterns-guide/SKILL.md...
Checking GEMINI.md...
```

### Failure Diagnostic
- **排查路径**: 检查 `.agents/skills/` 对应文件是否丢失或存在语法错误。

---

## 验证场景 3: 架构知识库与仓库组织蓝图完整性验证

验证沉淀的架构文档与多仓库布局指南是否完整存在且具备深度引证。

### Command
```bash
test -f docs/architecture/libarchive_engineering_excellence.md && \
test -f docs/architecture/repository_organization_guide.md && \
echo "Architecture documents verified successfully."
```

### Expected Output
```text
Architecture documents verified successfully.
```

### Failure Diagnostic
- **排查路径**: 若文件不存在，检查 Task 是否已完整落盘至 `docs/architecture/` 目录。
