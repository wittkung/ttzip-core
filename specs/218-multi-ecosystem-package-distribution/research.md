# Research: Multi-Ecosystem Package Distribution Standards

**Feature**: `218-multi-ecosystem-package-distribution`  

---

## 1. Homebrew Tap Specification & Architecture

### Formula Best Practices
- **Naming**: `Formula/ttzip.rb` allows users to install via `brew install wittkung/ttzip/ttzip` or `brew tap wittkung/ttzip && brew install ttzip`.
- **Class Name**: `class Ttzip < Formula` (PascalCase for `ttzip`).
- **Dependencies**: Depends on `rust` (`:build`).
- **Build Method**:
  ```ruby
  system "cargo", "build", "--release", "--manifest-path", "rust/Cargo.toml", "-p", "ttzip-tui"
  bin.install "rust/target/release/ttzip"
  ```
- **Test Block**:
  ```ruby
  test do
    assert_match "TTZip", shell_output("#{bin}/ttzip --version")
    (testpath/"test.txt").write("Homebrew distribution test payload")
    system bin/"ttzip", "archive", "test.zip", "test.txt"
    assert_predicate testpath/"test.zip", :exist?
  end
  ```

---

## 2. Crates.io Packaging Requirements

### Mandatory Fields in Cargo.toml
1. `version = "1.0.0"`
2. `license = "BSD-3-Clause OR Apache-2.0"`
3. `description = "..."`
4. `repository = "https://github.com/wittkung/ttzip-core"`
5. `readme = "README.md"`
6. `keywords = ["compression", "zip", "7z", "simd", "archive"]`
7. `categories = ["compression", "command-line-utilities", "filesystem"]`

### Publishing Order (DAG Dependency Resolution)
1. Step 1: `ttzip-engine` (No internal dependencies)
2. Step 2: `ttzip-glue` (Depends on `ttzip-engine`)
3. Step 3: `ttzip-tui` (Depends on `ttzip-glue`)

---

## 3. Maturin Wheel Distribution

- **Command**: `maturin build --release --strip --out dist`
- **Output**: `dist/ttzip-1.0.0-cp310-abi3-macosx_11_0_arm64.whl` (or universal2).
- **Inspection Tool**: `wheel pack`, `unzip -l`, or `twine check dist/*`.
