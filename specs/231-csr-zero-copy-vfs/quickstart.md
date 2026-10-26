# Quickstart & Verification Guide: CSR Zero-Copy VFS (Feature 231)

**Feature Directory**: `products/ttzip/core/specs/231-csr-zero-copy-vfs`

---

## 1. Prerequisites

- macOS 14.0+ (Apple Silicon M-series recommended)
- Bazel 8.0+
- Rust 1.84+ toolchain (`cargo`, `clippy`)
- Swift 6.0+ toolchain

---

## 2. Fast Verification Scenarios

### Scenario 1: Validate Schema Contracts
Ensure all C-ABI and search JSON contracts strictly conform to Draft-07:

```bash
bash products/ttzip/core/.specify/scripts/bash/lint-contracts.sh \
  products/ttzip/core/specs/231-csr-zero-copy-vfs/contracts
```
*Expected Result*: `All 2 contract schemas verified successfully.` (Exit code 0).

---

### Scenario 2: 1,000,000-Node CSR Microkernel Test
Run the million-node synthetic tree and CSR flattening test:

```bash
cargo test --manifest-path products/ttzip/core/rust/Cargo.toml \
  -p ttzip-engine --test test_1m_node_tree -- --nocapture
```
*Expected Result*:
- Construction and CSR flattening of 1,000,000 nodes completes in $\le 250\text{ ms}$.
- Physical memory footprint remains $\le 100\text{ MB}$.
- Test passes 100% green.

---

### Scenario 3: Mach VM Memory Lifecycle & Page Alignment
Validate 16KB page-aligned Mach VM allocation, memory entry generation, and clean release:

```bash
cargo test --manifest-path products/ttzip/core/rust/Cargo.toml \
  -p ttzip-types --test test_mach_vm -- --nocapture
```
*Expected Result*:
- Allocation matches 16KB boundaries (`MACH_VM_PAGE_SIZE = 16384`).
- `mach_make_memory_entry_64` yields valid `MachPortGuard`.
- Dropping guard deallocates port via `mach_port_deallocate`.

---

### Scenario 4: Swift VfsSnapshotReader & Viewport Virtualization
Validate that Swift's `VfsSnapshotReader` extracts only visible rows on demand:

```bash
bazel test //products/ttzip/apple:AppCoreGateTests \
  --test_filter="VfsSnapshotReaderTests"
```
*Expected Result*:
- Reading children slice of 50 items from a 50,000-item directory takes $\le 1\text{ ms}$.
- Memory allocations remain strictly bounded to visible cell counts.

---

### Scenario 5: End-to-End Latency & Memory Gate (SC-001 through SC-005)
Run the full benchmark suite measuring cold archive loading, keystroke filtering, and memory:

```bash
cargo bench --manifest-path products/ttzip/core/rust/Cargo.toml \
  -p ttzip-engine --bench bench_vfs_million_items -- --sample-size 10
```
*Expected Thresholds*:
1. **Load Latency**: $\le 300\text{ ms}$ (SC-001)
2. **RSS Memory**: $\le 200\text{ MB}$ (SC-002)
3. **Search Latency**: $\le 8.33\text{ ms}$ (SC-003)
4. **Copy Volume**: 0 bytes across FFI boundary (SC-004)
5. **Memory Reclamation**: $\pm 2\text{ MB}$ baseline delta after lease release (SC-005)
