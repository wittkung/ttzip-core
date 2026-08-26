# Quickstart: Multi-Ecosystem Package Distribution

**Feature**: `218-multi-ecosystem-package-distribution`  

---

## 1. Install via Homebrew

```bash
brew tap wittkung/ttzip
brew install ttzip

# Verify
ttzip --version
```

---

## 2. Install via Python pip

```bash
pip install ttzip

# Verify in Python
python3 -c "import ttzip; print(ttzip.version())"
```

---

## 3. Use in Rust Cargo.toml

```toml
[dependencies]
ttzip-glue = "1.0.0"
```
