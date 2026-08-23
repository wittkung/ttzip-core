# Quickstart Guide: Standalone Rust CLI & Swift CLI Bridge

## 1. Building the Binaries
```bash
# Build standalone Rust CLI / TUI binary
cargo build --release -p ttzip-tui

# Build Swift binaries
swift build -c release
```

## 2. Using the Standalone Rust CLI
```bash
# List archive contents
./rust/target/release/ttzip list archive.zip

# Extract archive
./rust/target/release/ttzip extract archive.zip -o ./out

# Create new ZIP or 7z archive
./rust/target/release/ttzip create backup.7z file1.txt folder/ -l 9

# Recover password
./rust/target/release/ttzip recover secret.zip -d wordlist.txt

# Repair damaged archive
./rust/target/release/ttzip repair damaged.zip -o repaired.zip

# Split archive
./rust/target/release/ttzip split big.zip -v 100M

# Join archive volumes
./rust/target/release/ttzip join big.zip.001 -o merged.zip
```
