# Research: EmbeddingGemma 2 GGUF & `llama.cpp` Semantic Search Architecture (Feature 232)

**Feature**: Hybrid Lexical-Semantic Search Engine with EmbeddingGemma 2 GGUF & `llama.cpp`  
**Location**: `products/ttzip/core/specs/232-embedding-semantic-search/research.md`  
**Updated**: 2026-10-18  
**Canonical Twin**: [`products/ttzip/docs/twin/semantic-search.md`](../../docs/twin/semantic-search.md)  

---

## 1. Why TTZip Switched from CoreML Prototype to GGUF + `llama.cpp` + Rust

1. **CoreML Export Activation Overflow & Missing Parity**:
   - `products/ttzip/scripts/export_embeddinggemma_coreml.py` exported `google/embeddinggemma-2` with `compute_precision=ct.precision.FLOAT16` and static `seq_len = 32`.
   - As verified in [`references/digup/Sources/DigUpKit/ModelFiles.swift`](../../../../references/digup/Sources/DigUpKit/ModelFiles.swift), EmbeddingGemma 2 intermediate activations overflow IEEE 754 Float16 (`65504`).
   - Meanwhile, `ggml-org/embeddinggemma-2-GGUF` (`rev bfcd298762cc34d0357ece5ebdd31791a3a374d8`, `embeddinggemma-2-Q8_0.gguf`, `309,855,456` bytes, SHA-256 `2188ac1deca4b77dffefd603c2776a9d76d9d74ec01841392982ebb840b09135`) is already verified against `mlx-vlm` BF16 reference vectors ($\text{cosine} \ge 0.999$).
2. **Built-In GGUF SentencePiece Tokenizer**:
   - The `.gguf` file embeds the complete 262,144-token SentencePiece BPE vocabulary and special token rules (`<bos>=2`, `<eos>=1`). Calling `llama_tokenize` in Rust eliminates the broken `3 + byte` UTF-8 stub in `SemanticQueryEncoder.swift` without introducing an extra Swift tokenizer dependency.
3. **Critical `llama.cpp-no-logits.patch` Memory Saving**:
   - At `llama.cpp` tag `b11461` (`4d756bc72bf00a4aacf410ae15a2d315f3db400d`), `llama_context::output_reserve` allocates `n_vocab` floats per token by default ($262{,}144 \times 4\text{ B} = 1\text{ MiB/token}$, or **$2\text{ GiB}$** for a 2,048-token batch).
   - Applying [`llama.cpp-no-logits.patch`](../../../../references/digup/scripts/llama.cpp-no-logits.patch) (SHA-256 `33c8c4812a4307aef3f398e5ba622edcc54ef421054edd98ca0f71906b9a2d7e`) sets `bool has_logits = !(cparams.embeddings && !cparams.causal_attn);`, keeping context working memory under `~65 MiB`.
4. **Canonical Prompt Templates (`Model.swift`)**:
   - General search query: `"task: search result | query: \(query)"`
   - Code search query (`code:`): `"task: code retrieval | query: \(cleanQuery)"`
   - Document / entry chunk: `"title: \(relativePath.isEmpty ? "none" : relativePath) | text: \(chunkText)"`
5. **DigUp Initial Indexing Throughput Caution ([DigUp Issue #48](https://github.com/ARahim3/DigUp/issues/48))**:
   - DigUp's own initial full-corpus indexing throughput has ongoing upstream optimization discussions. TTZip does not inherit DigUp's throughput numbers; instead, TTZip separates **Tier-1 (Filename/Path)** from **Tier-2 (UTF-8 Text/Code Chunks)** and commits checkpoints incrementally so search remains immediately responsive.
6. **Surrogate Model Experiment Disclaimer**:
   - Earlier offline experiments using `all-MiniLM-L6-v2` and macOS `NLEmbedding(.simplifiedChinese)` (`Hit@1 = 0.950`) were strictly mechanism ablations to prove the necessity of Tier-2 body indexing and subword tokenization over `entry_<id>` placeholders; they do **not** represent EmbeddingGemma 2 or TTZip production accuracy.
