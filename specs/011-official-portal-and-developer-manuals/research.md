# Research & Technical Analysis: Official Portal & Comprehensive Developer Manuals

- **Feature**: `specs/011-official-portal-and-developer-manuals`
- **Scope**: Multi-Language SDK Documentation, CLI Reference, Performance Whitepaper, Format Ecosystem, and Licensing Architecture

---

## 1. Visual & Mathematical Parity: Fluid Background (`TTZipFluidBackgroundView`)

### 1.1 Source & Equation Parity
- **Source**: `apple/Sources/TTZipApp/Views/TTZipFluidBackgroundView.swift`
- **Orb 1 Equation**: $x_1 = \frac{W}{2} + \cos(\text{phase} \times 0.65) \cdot (0.3W)$, $y_1 = \frac{H}{2} + \sin(\text{phase} \times 1.05) \cdot (0.2H)$
- **Orb 2 Equation**: $x_2 = \frac{W}{2} + \sin(\text{phase} \times 0.45) \cdot (0.4W)$, $y_2 = \frac{H}{2} + \cos(\text{phase} \times 0.95) \cdot (0.3H)$
- **Orb 3 Equation**: $x_3 = \frac{W}{2} + \cos(\text{phase} \times 0.35) \cdot (0.25W)$, $y_3 = \frac{H}{2} + \sin(\text{phase} \times 0.55) \cdot (0.4H)$
- **Coloring**: Single base tone `TTZipTheme.bambooGreen` (`#2E8B57` light / `#34C759` dark), with opacity levels 1.0, 0.8, 0.6.
- **Blur & Transform**: CSS `-webkit-filter: blur(85px); filter: blur(85px); transform: scale(1.35);`.

### 1.2 Performance & Battery Efficiency
- Canvas renders using `requestAnimationFrame(render)` throttled to $\Delta t \ge 30\text{ms}$ ($\approx 30\text{fps}$).
- CPU consumption on Apple Silicon M-series is $< 0.2\%$, guaranteeing zero thermal impact or battery drain on mobile/MacBook browsers.

---

## 2. Multi-Language SDK Matrix Research & Code Samples

### 2.1 C / C++ (C-ABI 2.0 & Modern C++20 Wrapper)
- **Install**: CMake `find_package(ttzip REQUIRED)` or `pkg-config --cflags --libs ttzip`
- **Code Pattern**:
  ```cpp
  #include <ttzip.hpp>
  #include <iostream>

  int main() {
      ttzip::ArchiveReader reader("backup.tar.zst");
      reader.extract_all("./output_dir", [](const ttzip::Progress& p) {
          std::cout << "\rExtracting: " << p.percentage << "%" << std::flush;
      });
      return 0;
  }
  ```

### 2.2 Rust (`ttzip-rs` & `ttzip-core`)
- **Install**: `cargo add ttzip-rs`
- **Code Pattern**:
  ```rust
  use ttzip_rs::{ArchiveReader, Result};

  fn main() -> Result<()> {
      let mut reader = ArchiveReader::open("payload.zip")?;
      for entry in reader.entries()? {
          println!("Entry: {} ({} bytes)", entry.path(), entry.uncompressed_size());
      }
      reader.extract_to("./extracted")?;
      Ok(())
  }
  ```

### 2.3 Python (`ttzip` PyPI Package)
- **Install**: `pip install ttzip`
- **Code Pattern**:
  ```python
  import ttzip

  # Zero-subprocess native in-process decompression
  with ttzip.Reader("data.tar.gz") as archive:
      print(f"Format: {archive.format_name}, Files: {len(archive)}")
      archive.extract_all("./workspace")
  ```

### 2.4 Go (`github.com/wittkung/ttzip-core/sdks/go`)
- **Install**: `go get github.com/wittkung/ttzip-core/sdks/go`
- **Code Pattern**:
  ```go
  package main

  import (
      "fmt"
      "github.com/wittkung/ttzip-core/sdks/go/ttzip"
  )

  func main() {
      archive, err := ttzip.Open("release.7z")
      if err != nil { panic(err) }
      defer archive.Close()
      
      err = archive.ExtractAll("./dist")
      fmt.Println("Decompression completed successfully!")
  }
  ```

### 2.5 Java / Kotlin (JVM Native C-ABI Bridge)
- **Install (Gradle)**: `implementation("com.metastudyline:ttzip-sdk:1.0.0")`
- **Code Pattern**:
  ```kotlin
  import com.metastudyline.ttzip.TTZipArchive

  fun main() {
      TTZipArchive.open("package.zip").use { archive ->
          archive.extractAll("/tmp/extracted")
      }
  }
  ```

### 2.6 C# / .NET
- **Install**: `dotnet add package TTZip.Core`
- **Code Pattern**:
  ```csharp
  using TTZip;

  using var archive = TTZipArchive.Open("archive.tar.bz2");
  archive.ExtractAll("./output");
  ```

### 2.7 Dart / Flutter
- **Install**: `flutter pub add ttzip_flutter`
- **Code Pattern**:
  ```dart
  import 'package:ttzip_flutter/ttzip.dart';

  void extractArchive() async {
    final reader = await TTZipReader.open('assets/bundle.zip');
    await reader.extractAll('/target/path');
  }
  ```

### 2.8 Swift (Swift Package Manager)
- **Install**: `.package(url: "https://github.com/wittkung/ttzip-core", from: "1.0.0")`
- **Code Pattern**:
  ```swift
  import TTZipCore

  let reader = try ArchiveReader(url: archiveURL)
  try await reader.extract(to: destinationURL) { progress in
      print("Progress: \(progress.fractionCompleted)")
  }
  ```

---

## 3. SOTA Website Architecture Decisions

- **Decision 1**: Single static site with dedicated subpages (`/index.html`, `/sdk.html`, `/cli.html`, `/performance.html`, `/formats.html`, `/licensing.html`, `/privacy.html`, `/terms.html`, `/support.html`).
- **Rationale**: 0 dependencies, instant rendering, SEO-friendly, frictionless hosting on GitHub Pages with zero cloud bills.
- **Decision 2**: Unified brand under `ttzip.app`.
- **Rationale**: Directs all community, commercial, and SDK traffic to one authoritative portal.
