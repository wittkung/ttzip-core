# TTZip Code Generation & Mathematical Deduction Toolchain (DEFLATE / CRC)

This directory contains deterministic mathematical derivation and code generation scripts for TTZip's microkernel compression and checksum sub-engines.

---

## Toolchain Inventory

1. **`gen-crc32-consts.py`**:
   - **Mathematical Basis**: Polynomial long division over the Galois Field $\text{GF}(2)$ with IEEE 802.3 generator polynomial $G(x) = x^{32} + x^{26} + x^{23} + x^{22} + x^{16} + x^{12} + x^{11} + x^{10} + x^8 + x^7 + x^5 + x^4 + x^2 + x + 1$ (`0x104c11db7`).
   - **Output**: Derives 12-Way PMULL / PCLMUL vector folding constants, Barrett reduction multipliers, and Slice-by-8 lookup tables without heuristic magic numbers.
   - **Usage**: `./gen-crc32-consts.py > crc32_constants.h`

2. **`gen_default_litlen_costs.py`**:
   - **Mathematical Basis**: Shannon information entropy bit cost modeling derived from statistical distribution of literal and length symbols across multi-gigabyte corpora.
   - **Output**: Default bit cost tables for Near-Optimal dynamic programming graph parsing (Level 10~12).

3. **`gen_bitreverse_tab.py`**:
   - Generates 8-bit / 16-bit bit reversal tables for fast Canonical Huffman code emission.

4. **`gen_offset_slot_map.py`**:
   - Generates compact mapping arrays for RFC 1951 distance slot fast lookups.
