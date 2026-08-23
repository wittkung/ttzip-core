# Implementation Plan: 197-purge-broken-scripts-and-deduplicate-site-docs

## Technical Context
- Purge obsolete `scripts/run_delta_audit.sh`.
- Deduplicate static site files by removing `site/` and stray `docs/*.html|xml|CNAME`.
- Update `.gitattributes` to remove `site/**`.

---

## Constitution Check
- [x] Zero Cloud Quota.
- [x] Single-file LOC $\le 800$.

---

## Phase 0: Research Items
- R001 [SUBAGENT:research] 《无效基准子脚本清理方案》: Completed.
- R002 [SUBAGENT:research] 《静态站点与发布清单单点真相源方案》: Completed.

---

## Phase 1: Broken Script Purge
- Delete `scripts/run_delta_audit.sh`.

## Phase 2: Site Deduplication & Repository Hygiene
- Delete `site/` directory.
- Delete loose files in `docs/` (`index.html`, `privacy.html`, `terms.html`, `CNAME`, `appcast.xml`).
- Update `.gitattributes` to remove `site/**` rule.

## Phase 3: Verification & Gate
- Run `./scripts/lint_loc_gate.sh`.
- Run `swift test` and `cargo test --workspace`.
- Run `./scripts/run_local_ci_gate.sh`.
