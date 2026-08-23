# Specification Quality Checklist: 198-modernize-multilingual-readmes-and-homebrew-formulas

## 1. Content Quality
- [x] Clear division into root file deduplication, Homebrew Formula synchronization, and 4-language README modernization.
- [x] Aligned with user's explicit preference: only `Install-TTZip.command` retained.

## 2. Requirement Completeness
- [x] Zero CMake mentions remaining in README instructions.
- [x] Both `ttzip` and `ttzip-cli` Homebrew formulas updated.

## 3. Feature Readiness
- [x] Single-file LOC $\le 800$ preserved across all modified files.
- [x] Local CI automated regression gate 100% green.
