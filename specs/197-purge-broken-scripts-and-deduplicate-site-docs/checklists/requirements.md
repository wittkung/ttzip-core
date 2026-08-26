# Specification Quality Checklist: 197-purge-broken-scripts-and-deduplicate-site-docs

## 1. Content Quality
- [x] Clear division into broken script purge, duplicate website deduplication, and gitattributes cleanup.
- [x] Grounded on exact file paths and byte-by-byte diff checks.

## 2. Requirement Completeness
- [x] Zero broken scripts remaining in `scripts/`.
- [x] 100% deduplication of web and appcast assets.

## 3. Feature Readiness
- [x] Single-file LOC $\le 800$ preserved across all modified files.
- [x] Local CI automated regression gate 100% green.
