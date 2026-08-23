# Feature Specification: 197-purge-broken-scripts-and-deduplicate-site-docs

## 1. Executive Summary & Strategic Motivation
1. Purge obsolete `scripts/run_delta_audit.sh` which fails due to legacy `delta` subcommand removal in `ttzip-bench`.
2. Deduplicate static website files across the workspace by removing duplicate directory `site/` (8 duplicate files) and removing stray duplicate files in `docs/` (`docs/index.html`, `docs/privacy.html`, `docs/terms.html`, `docs/CNAME`, `docs/appcast.xml`).
3. Maintain root directory as the single source of truth for the official website, legal pages, and Sparkle appcast.
4. Verify all tests and CI gates pass 100%.

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Clean Script Suite
- **Given** inspecting the `scripts/` directory
- **When** running any utility script
- **Then** every script executes cleanly with zero unknown subcommand errors.

### User Scenario 2: Single Source of Truth for Web Content
- **Given** updating product website pages or Sparkle `appcast.xml`
- **When** modifying root HTML / XML files
- **Then** zero duplicate copies exist in `site/` or loose in `docs/`.

---

## 3. Success Metrics
1. Delete `scripts/run_delta_audit.sh`.
2. Delete `site/` directory (8 duplicate files).
3. Remove 5 loose duplicates in `docs/` (`index.html`, `privacy.html`, `terms.html`, `CNAME`, `appcast.xml`).
4. Update `.gitattributes` to remove `site/**` rule.
5. Pass all 4 stages of local CI gate in $< 10\text{s}$.
