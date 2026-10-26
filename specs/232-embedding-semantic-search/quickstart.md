# Quickstart: Developing & Verifying Feature 232 Hybrid Semantic Search (GGUF / Rust)

**Location**: `products/ttzip/core/specs/232-embedding-semantic-search/quickstart.md`  
**Updated**: 2026-10-18  
**Canonical Twin**: [`products/ttzip/docs/twin/semantic-search.md`](../../docs/twin/semantic-search.md)  

---

## 1. Run `ttzip-semantic` Rust Unit & Integration Tests

```bash
bazel test //products/ttzip/core/rust/crates/ttzip-semantic:ttzip_semantic_test
```

Or via Cargo once **H** applies the workspace member registration patch (`contracts/handoff_patches.md`):
```bash
cargo test --manifest-path products/ttzip/core/rust/crates/ttzip-semantic/Cargo.toml
```

---

## 2. Run Swift `OmniSearchTests` (`SemanticSearchTests.swift`)

```bash
bazel test //products/ttzip/apple:OmniSearchTests --test_filter=SemanticSearchTests
```

This verifies:
1. Reference vector parity ($\text{cosine} \ge 0.999930$) against `references/digup/Tests/LlamaRuntimeTests/Reference/vectors.json`.
2. Real GGUF model checkpoint verification (`embeddinggemma-2-Q8_0.gguf`, `309,855,456` B, SHA-256 `2188ac1d...`) — explicitly reporting `BLOCKED` if the 310 MB GGUF weight file is not installed on disk.
3. End-to-end retrieval on opaque filenames (`note_001.txt`, `memo_2026_10_14.md`, `mod_a.rs`) across English, Chinese, code, long-text chunking, and no-hit queries.
4. Exact filename and number/identifier preservation (`invoice_2026_q3.pdf`, `0x47`).
5. Cooperative cancellation, `request_id` stale-result protection, and multi-window isolation.
6. Index persistence across restarts, incremental file modify/rename/delete, and automatic rebuild on index corruption or `model_fingerprint` upgrade.
7. Authorized scope boundary enforcement, `.git`/`.ssh` exclusion, scope revocation purge, and archive `entrySubpath` open request preservation.
