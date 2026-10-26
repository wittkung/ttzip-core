# Quickstart Guide: 233 - Treemap VFS & Storage Space Inspector

> **Target Version**: TTWorkspace v6.2  
> **Audience**: Systems Engineers & macOS Client Developers  

---

## 1. Rust Microkernel Verification & Unit Tests

To run the pure Rust core tests for the Treemap layout engine and Seen deduplicator:

```bash
cd products/ttzip/app-core
cargo test workspace::treemap
cargo test workspace::scanner
```

### Benchmark Layout Performance (Criterion)
```bash
cargo bench --bench treemap_layout
```
Expected output:
```text
treemap_layout/50000_nodes
                        time:   [2.10 ms 2.22 ms 2.38 ms]
```

---

## 2. Darwin Bulk Scanner Verification (macOS Terminal)

To run a standalone test of Darwin's `getattrlistbulk(2)` scanner against a local directory (e.g., `~/Downloads`):

```bash
cargo test -p ttzip-app-core --test test_darwin_bulk_scanner -- --nocapture
```

---

## 3. Building and Running the Desktop Application

### Step 1: Compile Rust Dynamic Libraries & UniFFI Glue
```bash
cd products/ttzip/app-core
./scripts/build_rust.sh
./scripts/generate_uniffi.sh
```

### Step 2: Build and Launch macOS App
```bash
cd products/ttzip/apple
swift build
# Or launch directly with Xcode / build_and_run command
open TTZipApple.xcodeproj
```

---

## 4. Manual Verification Flow

### Test Case A: Archive-Internal Treemap (⌘3)
1. Launch TTZip and drag a large ZIP or TAR archive (e.g., a node project or multi-GB asset pack) into the window.
2. In the top toolbar, click the **Treemap** button or press `⌘3`.
3. Verify that the window renders an interactive squarified treemap.
4. Hover over tiles and observe the right Inspector showing file details.
5. Double-click an inner folder tile and verify the viewport smoothly navigates into the subfolder.
6. Press `Esc` or `Backspace` to navigate back to the parent directory.

### Test Case B: Storage Space Inspector
1. In the left sidebar, locate the **Locations** section.
2. Click **Storage Inspector**.
3. Select your `Downloads` or `Projects` folder.
4. Click **Start Analysis**.
5. Verify progress indicator streaming discovery rates ($\ge 25,000\text{ files/s}$).
6. Observe identified `target/` and `node_modules/` folders rendered with amber diagonal hatch marks.
7. Click a reclaimable tile and verify safe deletion routes through macOS Trash.
