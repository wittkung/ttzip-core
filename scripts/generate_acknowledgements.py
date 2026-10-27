#!/usr/bin/env python3
# Copyright 2026 MetastudyLine. All rights reserved.
# SPDX-License-Identifier: BSD-3-Clause

"""
generate_acknowledgements.py - Build-Closure-Driven Third-Party License Harvester

Queries the actual Bazel build dependency closure per distribution channel
(mas, direct, steam) via `bazel cquery --notool_deps`, collects every individual
third-party library and Rust crate with its exact resolved version and channel
scope, and reads the verbatim upstream LICENSE / COPYING / NOTICE texts from
the unpacked external repositories (`$(bazel info output_base)/external/`) and
`third_party/`.

Fails fast (non-zero exit) if any dependency in the build closure is missing
an exact version or a verbatim upstream license file.
"""

import argparse
import hashlib
import json
import os
import plistlib
import re
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import Any

CHANNELS = ("mas", "direct", "steam")

BUILD_SYSTEM_REPOS = {
    "apple_support",
    "aspect_bazel_lib",
    "aspect_rules_js",
    "aspect_rules_ts",
    "bazel_features",
    "bazel_skylib",
    "bazel_tools",
    "crates",
    "platforms",
    "rules_apple",
    "rules_cc",
    "rules_Foreign_cc",
    "rules_java",
    "rules_license",
    "rules_nodejs",
    "rules_cc",
    "rules_python",
    "rules_rust",
    "rules_ shell",
    "rules_shell",
    "rules_swift",
    "rust_toolchains",
}

WORKSPACE_FALLBACK_MAP = {
    "alloc-stdlib": "alloc-stdlib",
    "jxl-bitstream": "jxl-oxide",
    "jxl-coding": "jxl-oxide",
    "jxl-color": "jxl-oxide",
    "jxl-frame": "jxl-oxide",
    "jxl-grid": "jxl-oxide",
    "jxl-image": "jxl-oxide",
    "jxl-modular": "jxl-oxide",
    "jxl-oxide": "jxl-oxide",
    "jxl-render": "jxl-oxide",
    "jxl-vardct": "jxl-oxide",
    "kamadak-exif": "kamadak-exif",
    "symphonia": "symphonia",
    "symphonia-bundle-flac": "symphonia",
    "symphonia-bundle-mp3": "symphonia",
    "symphonia-codec-aac": "symphonia",
    "symphonia-codec-adpcm": "symphonia",
    "symphonia-codec-alac": "symphonia",
    "symphonia-codec-pcm": "symphonia",
    "symphonia-codec-vorbis": "symphonia",
    "symphonia-core": "symphonia",
    "symphonia-format-caf": "symphonia",
    "symphonia-format-isomp4": "symphonia",
    "symphonia-format-mkv": "symphonia",
    "symphonia-format-ogg": "symphonia",
    "symphonia-format-riff": "symphonia",
    "symphonia-metadata": "symphonia",
    "symphonia-utils-xiph": "symphonia",
    "tree-sitter-c": "tree-sitter-c",
    "tree-sitter-css": "tree-sitter-css",
    "tree-sitter-html": "tree-sitter-html",
    "tree-sitter-javascript": "tree-sitter-javascript",
    "tree-sitter-json": "tree-sitter-json",
    "tree-sitter-md": "tree-sitter-md",
    "tree-sitter-python": "tree-sitter-python",
    "tree-sitter-rust": "tree-sitter-rust",
    "tree-sitter-swift": "tree-sitter-swift",
    "tree-sitter-typescript": "tree-sitter-typescript",
    "uniffi": "uniffi-rs",
    "uniffi_core": "uniffi-rs",
    "zune-bmp": "zune-image",
    "zune-core": "zune-image",
    "zune-farbfeld": "zune-image",
    "zune-hdr": "zune-image",
    "zune-image": "zune-image",
    "zune-inflate": "zune-image",
    "zune-jpeg": "zune-image",
    "zune-jpegxl": "zune-image",
    "zune-png": "zune-image",
    "zune-ppm": "zune-image",
    "zune-psd": "zune-image",
    "zune-qoi": "zune-image",
}


def resolve_repo_root() -> Path:
    return Path(__file__).resolve().parents[4]


def get_bazel_external_dir(repo_root: Path) -> Path:
    out_base = subprocess.check_output(
        ["bazel", "info", "output_base"],
        cwd=str(repo_root),
        text=True,
    ).strip()
    ext_dir = Path(out_base) / "external"
    if not ext_dir.is_dir():
        raise RuntimeError(f"Bazel external directory not found at {ext_dir}")
    return ext_dir


def parse_module_bazel_versions(module_path: Path) -> dict[str, str]:
    if not module_path.is_file():
        raise RuntimeError(f"Missing MODULE.bazel at {module_path}")
    content = module_path.read_text(encoding="utf-8")
    deps: dict[str, str] = {}
    for m in re.finditer(
        r'bazel_dep\(\s*name\s*=\s*"([^"]+)"\s*,\s*version\s*=\s*"([^"]+)"\s*\)',
        content,
    ):
        deps[m.group(1)] = m.group(2)
    return deps


def query_channel_closure(repo_root: Path) -> tuple[
    dict[str, set[str]],
    dict[str, set[str]],
    dict[tuple[str, str], set[str]],
]:
    tp_channels: dict[str, set[str]] = {}
    bcr_channels: dict[str, set[str]] = {}
    crate_channels: dict[tuple[str, str], set[str]] = {}

    for ch in CHANNELS:
        cmd = [
            "bazel",
            "cquery",
            "-c",
            "opt",
            "--notool_deps",
            f"--//products/ttzip/apple:channel={ch}",
            "deps(//products/ttzip/apple:TTZipApp)",
        ]
        output = subprocess.check_output(cmd, cwd=str(repo_root), text=True)
        for line in output.splitlines():
            target = line.strip().split()[0] if line.strip() else ""
            if not target:
                continue

            m_tp = re.match(r"^//third_party/([A-Za-z0-9_.-]+):", target)
            if m_tp:
                pkg = m_tp.group(1)
                if pkg != "patches":
                    tp_channels.setdefault(pkg, set()).add(ch)
                continue

            m_crate = re.match(
                r"^@@rules_rust\+\+crate\+crates__([A-Za-z0-9_-]+)-([0-9][^\s/:]*)\b",
                target,
            )
            if m_crate:
                key = (m_crate.group(1), m_crate.group(2))
                crate_channels.setdefault(key, set()).add(ch)
                continue

            m_bcr = re.match(r"^@+([A-Za-z0-9_.-]+?)\+?//", target)
            if m_bcr:
                repo_name = m_bcr.group(1)
                if (
                    repo_name not in BUILD_SYSTEM_REPOS
                    and not repo_name.startswith(("rules_", "local_config_", "internal_", "rust_"))
                ):
                    bcr_channels.setdefault(repo_name, set()).add(ch)

    if not tp_channels or not bcr_channels or not crate_channels:
        raise RuntimeError(
            "Failed to collect non-empty dependency closure across third_party, BCR, and Rust crates."
        )
    return tp_channels, bcr_channels, crate_channels


def is_license_filename(filename: str) -> bool:
    upper = filename.upper()
    if upper.endswith((".PY", ".SH", ".RS", ".C", ".H", ".SWIFT", ".JSON", ".TOML", ".BAZEL", ".BZL")):
        return False
    if upper in ("COPYING.GPLV2", "COPYING.GPLV3", "COPYING.LGPLV2.1"):
        return False
    return any(
        upper.startswith(prefix)
        for prefix in ("LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT", "UNLICENSE")
    )


def collect_license_files(directory: Path) -> list[Path]:
    if not directory.is_dir():
        return []
    top_level: list[Path] = []
    for child in sorted(directory.iterdir()):
        if child.is_file() and is_license_filename(child.name):
            top_level.append(child)
    lib_dir = directory / "lib"
    if lib_dir.is_dir():
        for child in sorted(lib_dir.iterdir()):
            if child.is_file() and is_license_filename(child.name):
                top_level.append(child)
    if top_level:
        return top_level

    nested: list[Path] = []
    for root, _, files in os.walk(directory):
        for f in sorted(files):
            if is_license_filename(f):
                nested.append(Path(root) / f)
    return nested


def read_combined_license_files(files: list[Path], base_for_label: Path) -> tuple[str, list[str]]:
    chunks: list[str] = []
    rel_paths: list[str] = []
    for fpath in files:
        raw = fpath.read_text(encoding="utf-8", errors="replace")
        text = re.sub(r"[\x00-\x08\x0b\x0c\x0e-\x1f]", "\n", raw).strip()
        if not text:
            continue
        try:
            rel = str(fpath.relative_to(base_for_label))
        except ValueError:
            rel = str(fpath)
        rel_paths.append(rel)
        if len(files) > 1:
            chunks.append(f"=== {fpath.name} ===\n{text}")
        else:
            chunks.append(text)
    combined = "\n\n".join(chunks).strip()
    if len(combined) < 80:
        raise RuntimeError(f"License text from {rel_paths} is unexpectedly short ({len(combined)} chars)")
    return combined, rel_paths


def extract_copyright_line(text: str, fallback_authors: list[str] | None = None) -> str:
    for line in text.splitlines():
        clean = line.strip().lstrip("/*#;- ").rstrip("*/ ").strip()
        if re.search(r"copyright\s*(\(c\)|©|\d{4})", clean, re.IGNORECASE):
            return clean
    if fallback_authors:
        return f"Copyright (c) {', '.join(fallback_authors)}"
    return "Copyright (c) Upstream Authors and Contributors"


def resolve_third_party_metadata(pkg_dir: Path, pkg_name: str) -> dict[str, str]:
    metadata_file = pkg_dir / "METADATA.toml"
    if not metadata_file.is_file():
        raise RuntimeError(
            f"Missing traceable METADATA.toml for //third_party/{pkg_name} at {metadata_file}"
        )
    with open(metadata_file, "rb") as fh:
        meta = tomllib.load(fh)

    version = str(meta.get("version", "")).strip()
    upstream_url = str(meta.get("upstream_url", "")).strip()
    upstream_tag = str(meta.get("upstream_tag", "")).strip()
    upstream_commit = str(meta.get("upstream_commit", "")).strip()
    spdx = str(meta.get("spdx", "")).strip()
    verified_hashes = meta.get("verified_files_sha256")

    if not version or not upstream_url or not upstream_tag or not spdx:
        raise RuntimeError(
            f"Incomplete version/provenance fields in {metadata_file}"
        )
    if not re.fullmatch(r"[0-9a-f]{40}", upstream_commit):
        raise RuntimeError(
            f"Invalid 40-char git commit hash in {metadata_file}: {upstream_commit!r}"
        )
    if not isinstance(verified_hashes, dict) or not verified_hashes:
        raise RuntimeError(
            f"Missing [verified_files_sha256] integrity table in {metadata_file}"
        )

    for rel_file, expected_sha in verified_hashes.items():
        target = pkg_dir / rel_file
        if not target.is_file():
            raise RuntimeError(
                f"Provenance file '{rel_file}' listed in {metadata_file} does not exist"
            )
        actual_sha = hashlib.sha256(target.read_bytes()).hexdigest()
        if actual_sha != expected_sha:
            raise RuntimeError(
                f"SHA-256 mismatch for {target}: expected {expected_sha}, got {actual_sha}"
            )

    fl2_hdr = pkg_dir / "fast-lzma2.h"
    if fl2_hdr.is_file():
        hdr = fl2_hdr.read_text(encoding="utf-8", errors="ignore")
        maj = re.search(r"#define\s+FL2_VERSION_MAJOR\s+(\d+)", hdr)
        minor = re.search(r"#define\s+FL2_VERSION_MINOR\s+(\d+)", hdr)
        rel = re.search(r"#define\s+FL2_VERSION_RELEASE\s+(\d+)", hdr)
        if not (maj and minor and rel):
            raise RuntimeError(f"Missing FL2_VERSION_* macros in {fl2_hdr}")
        hdr_ver = f"{maj.group(1)}.{minor.group(1)}.{rel.group(1)}"
        if hdr_ver != version:
            raise RuntimeError(
                f"Version mismatch for {pkg_name}: METADATA.toml={version} vs fast-lzma2.h={hdr_ver}"
            )

    return {
        "version": f"{version} ({upstream_tag}@{upstream_commit[:12]})",
        "upstream_url": upstream_url,
        "spdx": spdx,
    }


def detect_spdx_from_text(text: str, component_name: str = "") -> str:
    lower = text.lower()
    if "mozilla public license" in lower and "2.0" in lower:
        return "MPL-2.0"
    if "apache license" in lower and "version 2.0" in lower and "permission is hereby granted" in lower:
        return "Apache-2.0 AND MIT"
    if "apache license" in lower and "version 2.0" in lower:
        return "Apache-2.0"
    if "permission to use, copy, modify, and/or distribute this software for any purpose" in lower:
        return "0BSD"
    if "permission is hereby granted, free of charge" in lower:
        return "MIT"
    if "neither the name" in lower and "redistribution and use in source and binary forms" in lower:
        return "BSD-3-Clause"
    if "redistribution and use in source and binary forms" in lower:
        return "BSD-2-Clause"
    if "julian r seward" in lower:
        return "bzip2-1.0.6"
    if "jean-loup gailly and mark adler" in lower or "altered source versions must be plainly marked" in lower:
        return "Zlib"
    if "liblzma is in the public domain" in lower:
        return "LicenseRef-XZ-Public-Domain"
    raise RuntimeError(
        f"Unrecognized license text for component '{component_name}'; manual SPDX verification required"
    )


def build_components(repo_root: Path) -> list[dict[str, Any]]:
    ext_dir = get_bazel_external_dir(repo_root)
    module_versions = parse_module_bazel_versions(repo_root / "MODULE.bazel")
    tp_channels, bcr_channels, crate_channels = query_channel_closure(repo_root)
    fallback_lic_root = repo_root / "third_party" / "licenses"

    components: list[dict[str, Any]] = []

    # 1. Native C Libraries from Bazel Central Registry (MODULE.bazel)
    for bcr_name in sorted(bcr_channels.keys()):
        if bcr_name not in module_versions:
            raise RuntimeError(f"BCR dependency '{bcr_name}' missing version in MODULE.bazel")
        ver = module_versions[bcr_name]
        repo_dir = ext_dir / f"{bcr_name}+"
        lic_files = collect_license_files(repo_dir)
        if not lic_files:
            raise RuntimeError(f"BCR dependency '{bcr_name}' ({ver}) has no LICENSE/COPYING file in {repo_dir}")
        lic_text, rel_files = read_combined_license_files(lic_files, repo_dir)
        spdx = detect_spdx_from_text(lic_text, bcr_name)
        ch_list = sorted(bcr_channels[bcr_name])
        components.append({
            "name": bcr_name,
            "version": ver,
            "category": "Native C Library (BCR)",
            "channels": ch_list,
            "source_manifest": f"MODULE.bazel + external/{bcr_name}+/({', '.join(rel_files)})",
            "license_type": spdx,
            "copyright": extract_copyright_line(lic_text),
            "spdx": spdx,
            "url": f"https://registry.bazel.build/modules/{bcr_name}",
            "description": f"Bazel Central Registry native C dependency ({bcr_name} {ver}).",
            "license_text": lic_text,
        })

    # 2. Vendored Native C Libraries in //third_party/*
    for tp_name in sorted(tp_channels.keys()):
        pkg_dir = repo_root / "third_party" / tp_name
        tp_meta = resolve_third_party_metadata(pkg_dir, tp_name)
        lic_files = collect_license_files(pkg_dir)
        if not lic_files:
            raise RuntimeError(f"Vendored dependency '//third_party/{tp_name}' has no LICENSE file in {pkg_dir}")
        lic_text, rel_files = read_combined_license_files(lic_files, repo_root)
        detected_spdx = detect_spdx_from_text(lic_text, tp_name)
        if detected_spdx != tp_meta["spdx"]:
            raise RuntimeError(
                f"SPDX mismatch for //third_party/{tp_name}: METADATA.toml={tp_meta['spdx']} vs LICENSE={detected_spdx}"
            )
        ch_list = sorted(tp_channels[tp_name])
        components.append({
            "name": tp_name,
            "version": tp_meta["version"],
            "category": "Native C Library (Vendored)",
            "channels": ch_list,
            "source_manifest": f"third_party/{tp_name}/METADATA.toml + {', '.join(rel_files)}",
            "license_type": detected_spdx,
            "copyright": extract_copyright_line(lic_text),
            "spdx": detected_spdx,
            "url": tp_meta["upstream_url"],
            "description": f"Vendored native C library in //third_party/{tp_name}.",
            "license_text": lic_text,
        })

    # 3. Every individual Rust crate in the build closure
    for (crate_name, crate_ver) in sorted(crate_channels.keys()):
        if not crate_ver:
            raise RuntimeError(f"Rust crate '{crate_name}' has empty version")
        crate_dir = ext_dir / f"rules_rust++crate+crates__{crate_name}-{crate_ver}"
        cargo_toml_path = crate_dir / "Cargo.toml"
        if not cargo_toml_path.is_file():
            raise RuntimeError(f"Missing Cargo.toml for crate {crate_name}-{crate_ver} at {cargo_toml_path}")

        with open(cargo_toml_path, "rb") as fh:
            cargo_data = tomllib.load(fh)
        pkg_meta = cargo_data.get("package", {})
        resolved_ver = pkg_meta.get("version", "")
        if resolved_ver != crate_ver:
            raise RuntimeError(
                f"Version mismatch for {crate_name}: target={crate_ver} vs Cargo.toml={resolved_ver}"
            )
        spdx = pkg_meta.get("license") or ""
        if not spdx:
            raise RuntimeError(f"Crate {crate_name}-{crate_ver} is missing 'license' SPDX field in Cargo.toml")

        authors = pkg_meta.get("authors") or []
        repo_url = pkg_meta.get("repository") or pkg_meta.get("homepage") or f"https://crates.io/crates/{crate_name}"
        desc = (pkg_meta.get("description") or f"Rust crate {crate_name}.").strip().replace("\n", " ")

        lic_files = collect_license_files(crate_dir)
        manifest_base = crate_dir
        manifest_prefix = f"external/rules_rust++crate+crates__{crate_name}-{crate_ver}"
        if not lic_files:
            fb_key = WORKSPACE_FALLBACK_MAP.get(crate_name)
            if fb_key:
                fb_dir = fallback_lic_root / fb_key
                lic_files = collect_license_files(fb_dir)
                manifest_base = repo_root
                manifest_prefix = ""
        if not lic_files:
            raise RuntimeError(
                f"Rust crate '{crate_name}-{crate_ver}' is missing upstream LICENSE/COPYING/NOTICE files in {crate_dir}"
            )

        lic_text, rel_files = read_combined_license_files(lic_files, manifest_base)
        source_paths = [f"{manifest_prefix}/{r}".lstrip("/") for r in rel_files]
        ch_list = sorted(crate_channels[(crate_name, crate_ver)])

        components.append({
            "name": crate_name,
            "version": resolved_ver,
            "category": "Rust Crate",
            "channels": ch_list,
            "source_manifest": ", ".join(source_paths),
            "license_type": spdx,
            "copyright": extract_copyright_line(lic_text, authors),
            "spdx": spdx,
            "url": repo_url,
            "description": desc,
            "license_text": lic_text,
        })

    # 4. Direct SPM framework dependency (Sparkle)
    pkg_resolved = repo_root / "products" / "ttzip" / "apple" / "Package.resolved"
    if not pkg_resolved.is_file():
        raise RuntimeError(f"Missing Package.resolved at {pkg_resolved}")
    pins_data = json.loads(pkg_resolved.read_text(encoding="utf-8"))
    for pin in pins_data.get("pins", []):
        identity = pin.get("identity", "")
        version = pin.get("state", {}).get("version", "")
        location = pin.get("location", "")
        if not identity or not version:
            raise RuntimeError(f"Malformed pin in {pkg_resolved}: {pin}")
        spm_lic_dir = fallback_lic_root / identity.lower()
        lic_files = collect_license_files(spm_lic_dir)
        if not lic_files:
            raise RuntimeError(f"SPM dependency '{identity}' ({version}) missing LICENSE in {spm_lic_dir}")
        lic_text, rel_files = read_combined_license_files(lic_files, repo_root)
        spdx = detect_spdx_from_text(lic_text, identity)
        components.append({
            "name": identity,
            "version": version,
            "category": "Swift / macOS Framework (Direct SPM)",
            "channels": ["direct"],
            "source_manifest": f"products/ttzip/apple/Package.resolved + {', '.join(rel_files)}",
            "license_type": spdx,
            "copyright": extract_copyright_line(lic_text),
            "spdx": spdx,
            "url": location,
            "description": "macOS software update framework used in Direct standalone builds.",
            "license_text": lic_text,
        })

    return components


def generate_markdown(components: list[dict[str, Any]], output_path: Path) -> None:
    lines = [
        "# Third-Party Open Source Software Acknowledgements & Licenses",
        "",
        "TTZip incorporates the following third-party open-source libraries across its distribution channels (`mas`, `direct`, `steam`).",
        f"Total verified dependency closure entries: **{len(components)}**.",
        "",
        "---",
        "",
    ]

    for comp in components:
        channels_str = ", ".join(comp["channels"])
        lines.extend([
            f'## {comp["name"]} (`v{comp["version"]}`)',
            f'- **Category**: {comp["category"]}',
            f'- **Channels**: `{channels_str}`',
            f'- **Resolved Version**: `{comp["version"]}`',
            f'- **Source License Files**: `{comp["source_manifest"]}`',
            f'- **SPDX Identifier**: `{comp["spdx"]}`',
            f'- **Copyright**: {comp["copyright"]}',
            f'- **Upstream Repository**: [{comp["url"]}]({comp["url"]})',
            f'- **Description**: {comp["description"]}',
            "",
            "```text",
            comp["license_text"],
            "```",
            "",
            "---",
            "",
        ])

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text("\n".join(lines), encoding="utf-8")
    print(f"[SUCCESS] Generated {len(components)} full-closure license attributions at {output_path}")


def generate_plist(components: list[dict[str, Any]], plist_path: Path) -> None:
    preference_specifiers: list[dict[str, Any]] = [
        {
            "Type": "PSGroupSpecifier",
            "Title": "TTZip incorporates the following third-party open-source software:",
            "FooterText": f"Total verified third-party dependencies in build closure: {len(components)}.",
        }
    ]

    for comp in components:
        channels_str = ", ".join(comp["channels"])
        footer = (
            f'{comp["description"]}\n'
            f'Version: {comp["version"]} | Channels: {channels_str}\n'
            f'Source: {comp["source_manifest"]}\n'
            f'License: {comp["spdx"]}\n'
            f'{comp["copyright"]}\n'
            f'{comp["url"]}\n\n'
            f'{comp["license_text"]}'
        )
        preference_specifiers.append({
            "Type": "PSGroupSpecifier",
            "Title": f'{comp["name"]} ({comp["version"]})',
            "FooterText": footer,
            "Version": comp["version"],
            "Channels": comp["channels"],
            "SourceManifest": comp["source_manifest"],
            "Category": comp["category"],
            "SPDX": comp["spdx"],
            "LicenseType": comp["license_type"],
            "Copyright": comp["copyright"],
            "URL": comp["url"],
            "LicenseText": comp["license_text"],
        })

    payload = {
        "StringsTable": "Acknowledgements",
        "PreferenceSpecifiers": preference_specifiers,
        "Components": components,
    }

    plist_path.parent.mkdir(parents=True, exist_ok=True)
    with open(plist_path, "wb") as f:
        plistlib.dump(payload, f, fmt=plistlib.FMT_XML, sort_keys=False)
    print(f"[SUCCESS] Generated {len(components)} full-closure plist entries at {plist_path}")


def main() -> None:
    repo_root = resolve_repo_root()
    default_md = repo_root / "products" / "ttzip" / "apple" / "ACKNOWLEDGEMENTS.md"
    default_plist = (
        repo_root / "products" / "ttzip" / "apple" / "Sources" / "TTZipApp" / "Resources" / "Acknowledgements.plist"
    )

    parser = argparse.ArgumentParser(description="Build-Closure Third-Party License Harvester")
    parser.add_argument("--output", default=str(default_md), help="Output markdown path")
    parser.add_argument("--plist-output", default=str(default_plist), help="Output plist path")
    args = parser.parse_args()

    components = build_components(repo_root)
    generate_markdown(components, Path(args.output))
    generate_plist(components, Path(args.plist_output))


if __name__ == "__main__":
    main()
