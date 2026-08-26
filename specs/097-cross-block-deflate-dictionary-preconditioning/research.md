# Research: 097-cross-block-deflate-dictionary-preconditioning

## Phase 0: Research Synthesis

### R001 [SUBAGENT:research] RFC 1951 Sliding Window History Injection
- **Decision**: Use `deflateSetDictionary(strm, dict_start, dict_size)` with up to 32,768 bytes before compressing subsequent blocks.
- **Rationale**: Deflate's LZ77 sliding window is strictly 32KB. By preloading the trailing 32KB of the previous uncompressed chunk, the compressor can immediately match repeat substrings across chunk boundaries without altering the bitstream format. Standard RFC 1951 decompressors will naturally maintain the 32KB history from the preceding block without needing explicit dictionary metadata.
- **Alternatives Considered**: Independent raw block compression (rejected due to 1-3% ratio loss at boundaries); Pigz-style process spawning (rejected due to IPC fork/exec overhead vs 100% in-process C).
- **Source**: RFC 1951 Section 3.2.5, Mark Adler's `pigz` architecture notes.

### R002 [SUBAGENT:research] Thread-Local z_stream Recycling
- **Decision**: Maintain a `_Thread_local z_stream s_tls_raw_deflate_strm[13]` array and reuse instances via `deflateReset()`.
- **Rationale**: `deflateInit2()` allocates internal sliding window hash tables on the heap (~256KB per stream). Performing `deflateReset()` avoids repeated `malloc()`/`free()` cycles across hundreds of parallel chunks.
- **Alternatives Considered**: Mutex-protected object pool (rejected due to lock contention in `concurrentPerform`).
- **Source**: zlib-ng `deflateReset` specifications.
