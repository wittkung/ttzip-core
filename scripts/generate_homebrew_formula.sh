#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.
#
# Generates Homebrew formula for ttzip standalone CLI.

set -euo pipefail

VERSION="${1:-1.0.0}"
TAG="v${VERSION}"
REPO_URL="https://github.com/wittkung/ttzip-core"
TARBALL_URL="${REPO_URL}/archive/refs/tags/${TAG}.tar.gz"

echo "Generating Formula/ttzip.rb for ${TAG}..."
mkdir -p Formula

cat << EOF > Formula/ttzip.rb
class Ttzip < Formula
  desc "Ultra-fast zero-dependency compression CLI and TUI for macOS and Linux"
  homepage "${REPO_URL}"
  url "${TARBALL_URL}"
  version "${VERSION}"
  license "BSD-3-Clause"

  depends_on "rust" => :build

  def install
    system "cargo", "build", "--release", "--manifest-path", "rust/Cargo.toml", "--bin", "ttzip"
    bin.install "rust/target/release/ttzip"
  end

  test do
    system "#{bin}/ttzip", "--version"
    system "#{bin}/ttzip", "doctor"
  end
end
EOF

echo "✅ Formula/ttzip.rb generated."
