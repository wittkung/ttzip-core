# Contracts: Multi-Ecosystem Package Distribution Interfaces

**Feature**: `218-multi-ecosystem-package-distribution`  

---

## 1. Homebrew Formula Interface (`Formula/ttzip.rb`)

```ruby
class Ttzip < Formula
  desc "High-performance native archiving and compression engine for macOS"
  homepage "https://github.com/wittkung/ttzip-core"
  url "https://github.com/wittkung/ttzip-core/archive/refs/tags/v1.0.0.tar.gz"
  sha256 "<COMPUTED_SHA256>"
  license any_of: ["BSD-3-Clause", "Apache-2.0"]
  head "https://github.com/wittkung/ttzip-core.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "build", "--release", "--manifest-path", "rust/Cargo.toml", "-p", "ttzip-tui"
    bin.install "rust/target/release/ttzip"
  end

  test do
    system bin/"ttzip", "--version"
  end
end
```

---

## 2. Release Packaging Script Contract (`scripts/verify_distribution.sh`)

```bash
# Returns 0 if and only if:
# 1. Homebrew Formula syntax passes ruby linter / audit.
# 2. cargo package --dry-run succeeds across all 3 rust crates.
# 3. maturin build produces a valid wheel inspectable by python.
```
