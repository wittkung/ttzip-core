# Quickstart: Verifying the Unified AVSampleBuffer Media Pipeline (Feature 230)

**Feature**: `230-unified-avsamplebuffer-media-pipeline`

All commands run from the repository root.

---

## 1. Automated Gates & Linkage Audit

### 1.1 Zero Homebrew Linkage Audit (SC-001)
Verify that neither the main application binary, the worker daemon, nor any helper extension links against `/opt/homebrew` or `/usr/local`:

```bash
# Build desktop app and worker binaries
bazel build //products/ttzip/apple:TTZipAppLib //infra/ttmedia:ttmedia-worker

# Assert zero Homebrew dynamic library linkages
otool -L bazel-bin/products/ttzip/apple/TTZipAppLib.a 2>/dev/null | grep -E "/opt/homebrew|/usr/local" || true
# -> Output MUST be empty
```

### 1.2 Monorepo Architecture & Code Quality Gates
```bash
python3 tools/ci/lint_loc_gate.py
python3 tools/ci/lint_code_quality.py
```

### 1.3 Contract Schema Verification
```bash
bash products/ttzip/core/.specify/scripts/bash/lint-contracts.sh \
  products/ttzip/core/specs/230-unified-avsamplebuffer-media-pipeline/contracts
```

---

## 2. Complete MPV Decommission Static Audit (SC-005)

Ensure all legacy MPV symbols and headers have been deleted from active source code:

```bash
# Assert no references to libmpv in products/ttzip or infra/ttmedia
git grep -n "libmpv" -- products/ttzip infra/ttmedia ':!*.md' ':!*specs*' || true
# -> Output MUST be empty

# Assert infra/ttmpv and third_party/mpv are completely removed
ls -d infra/ttmpv third_party/mpv 2>/dev/null || true
# -> Output: No such file or directory
```

---

## 3. Media Playback & Lip-Sync Verification (SC-002, SC-003)

1. Launch `products/ttzip/apple/dist/TTZip.app` on a clean macOS environment (no Homebrew).
2. Open an archive containing `sample_1080p.mkv` (H.264 + AAC) and `sample_4k.webm` (VP9 + Opus).
3. Select the file and press `Space` to open media preview:
   - Expected: Video renders within $\le 200\text{ ms}$; audio plays through CoreAudio hardware; Picture-in-Picture icon is available and responsive.
4. Scrub the timeline across multiple non-keyframe timestamps:
   - Expected: Preroll frame decoding occurs without visual tearing or macroblock smear; target frames display crisply.

---

## 4. Two-Tier Sleep & Memory Audit (SC-004)

```bash
# Verify worker lifecycle
ps aux | grep ttmedia-worker

# 1. Start video playback -> ttmedia-worker active
# 2. Pause/close playback -> after 30 seconds, worker drops to warm standby (<25 MB)
# 3. Wait 5 minutes -> worker process cleanly terminates (0 MB memory)
ps aux | grep ttmedia-worker | grep -v grep || echo "Worker successfully hibernated"
```
