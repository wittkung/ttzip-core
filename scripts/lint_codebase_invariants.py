#!/usr/bin/env python3
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

"""
TTZip Codebase Invariant Linter (AST & Token Analysis)
Enforces Constitution & Performance Invariants across C, Swift, and UI layers.
"""

import sys
import os
import re
import json
from pathlib import Path

def get_project_root():
    return Path(__file__).resolve().parent.parent

def scan_file_lines(file_path):
    try:
        with open(file_path, "r", encoding="utf-8", errors="ignore") as f:
            return f.readlines()
    except Exception:
        return []

def lint_hardcoded_paths(root_dir):
    violations = []
    pattern = re.compile(r'/Users/[a-zA-Z0-9_.-]+')
    sources_dir = root_dir / "Sources"
    
    for path in sources_dir.rglob("*"):
        if path.is_file() and path.suffix in [".swift", ".c", ".h", ".m"]:
            lines = scan_file_lines(path)
            for idx, line in enumerate(lines, 1):
                # Ignore comments
                stripped = line.strip()
                if stripped.startswith("//") or stripped.startswith("/*") or stripped.startswith("*"):
                    continue
                if pattern.search(line):
                    rel_path = str(path.relative_to(root_dir))
                    violations.append({
                        "ruleId": "NO_HARDCODED_USERS_PATH",
                        "filePath": rel_path,
                        "lineNumber": idx,
                        "snippet": line.strip(),
                        "remediationHint": "Replace /Users/... path with Bundle.main.path or dynamic relative resolution."
                    })
    return violations

def lint_bare_logging(root_dir):
    violations = []
    
    # 1. Swift scanning: check for bare print(...) and NSLog(...)
    swift_dirs = []
    # Local core Sources
    if (root_dir / "Sources" / "TTZipCore").exists():
        swift_dirs.append(root_dir / "Sources" / "TTZipCore")
    if (root_dir / "Sources" / "TTZipApp").exists():
        swift_dirs.append(root_dir / "Sources" / "TTZipApp")
    if (root_dir / "Sources" / "TTZipPluginKit").exists():
        swift_dirs.append(root_dir / "Sources" / "TTZipPluginKit")
        
    # Dual-repo / apple workspace Sources
    apple_sources = root_dir.parent / "apple" / "Sources"
    if apple_sources.exists():
        if (apple_sources / "TTZipApp").exists() and (apple_sources / "TTZipApp") not in swift_dirs:
            swift_dirs.append(apple_sources / "TTZipApp")
        if (apple_sources / "TTZipPluginKit").exists() and (apple_sources / "TTZipPluginKit") not in swift_dirs:
            swift_dirs.append(apple_sources / "TTZipPluginKit")
    
    swift_print_pattern = re.compile(r'\b(print|NSLog)\s*\(')
    for folder in swift_dirs:
        for path in folder.rglob("*.swift"):
            if path.name == "Logger.swift" or "CLI" in path.parts or "TUI" in path.parts or "Generated" in path.parts:
                continue
            lines = scan_file_lines(path)
            for idx, line in enumerate(lines, 1):
                stripped = line.strip()
                if stripped.startswith("//") or stripped.startswith("/*") or stripped.startswith("*"):
                    continue
                match = swift_print_pattern.search(line)
                if match:
                    log_kind = match.group(1)
                    rel_path = str(path.relative_to(root_dir)) if path.is_relative_to(root_dir) else str(path)
                    violations.append({
                        "ruleId": f"NO_BARE_{log_kind.upper()}_LOGGING",
                        "filePath": rel_path,
                        "lineNumber": idx,
                        "snippet": line.strip(),
                        "remediationHint": f"Replace bare {log_kind}(...) with TTLogger.debug / TTLogger.info or unified OSLog."
                    })

    # 2. Rust engine: no println! / eprintln! / eprint! in production code
    rust_src = root_dir / "rust" / "ttzip-engine" / "src"
    if rust_src.exists():
        rust_log_pattern = re.compile(r'\b(println|eprintln|eprint)!\s*\(')
        for path in rust_src.rglob("*.rs"):
            # Exclude benchmark module and tests/benches
            parts_lower = [p.lower() for p in path.parts]
            if "benchmark" in parts_lower or "tests" in parts_lower or "benches" in parts_lower:
                continue
            if path.name in ["tests.rs", "test.rs", "benches.rs", "bench.rs"] or path.name.endswith("_test.rs") or path.name.endswith("_tests.rs"):
                continue

            lines = scan_file_lines(path)
            in_cfg_test = False
            brace_level = 0
            test_brace_level = 0

            for idx, line in enumerate(lines, 1):
                stripped = line.strip()
                if stripped.startswith("//") or stripped.startswith("/*") or stripped.startswith("*"):
                    continue

                # Detect entering #[cfg(test)] module or test function
                if "#[cfg(test)]" in line or "#[test]" in line or "mod tests" in line or "mod test" in line:
                    in_cfg_test = True
                    test_brace_level = brace_level

                open_braces = line.count("{")
                close_braces = line.count("}")
                brace_level += open_braces - close_braces

                if in_cfg_test:
                    if brace_level <= test_brace_level and (open_braces > 0 or close_braces > 0):
                        in_cfg_test = False
                    continue

                # Check for bare macro calls outside tests
                if rust_log_pattern.search(line):
                    # Exclude string literals containing code examples
                    if line.strip().startswith("let code =") or line.strip().startswith("r#\"") or line.strip().startswith("\""):
                        continue
                    rel_path = str(path.relative_to(root_dir)) if path.is_relative_to(root_dir) else str(path)
                    violations.append({
                        "ruleId": "NO_BARE_RUST_PRINT_LOGGING",
                        "filePath": rel_path,
                        "lineNumber": idx,
                        "snippet": line.strip(),
                        "remediationHint": "Replace println!/eprintln!/eprint! with log::info! / log::error! / log::warn!."
                    })

    # 3. C bridge: no printf / NSLog in production
    c_bridge = root_dir / "Sources" / "CTTZipBridge"
    if c_bridge.exists():
        c_log_pattern = re.compile(r'\b(printf|NSLog)\s*\(')
        for path in c_bridge.rglob("*"):
            if path.is_file() and path.suffix in [".c", ".h"]:
                if "Diagnostics" in path.name or "fast-lzma2" in path.parts:
                    continue
                lines = scan_file_lines(path)
                for idx, line in enumerate(lines, 1):
                    stripped = line.strip()
                    if stripped.startswith("//") or stripped.startswith("/*") or stripped.startswith("*"):
                        continue
                    if c_log_pattern.search(line):
                        rel_path = str(path.relative_to(root_dir)) if path.is_relative_to(root_dir) else str(path)
                        violations.append({
                            "ruleId": "NO_BARE_PRINT_LOGGING",
                            "filePath": rel_path,
                            "lineNumber": idx,
                            "snippet": line.strip(),
                            "remediationHint": "Route C diagnostics through ttzip_log or TTZipDiagnostics."
                        })
    return violations

def lint_hotpath_data_count(root_dir):
    violations = []
    hot_dirs = [
        root_dir / "Sources" / "TTZipCore" / "Zip",
        root_dir / "Sources" / "TTZipCore" / "ConcurrencyPatterns"
    ]
    pattern = re.compile(r'\bData\s*\(\s*count\s*:')
    
    # Frozen files in Zip/ that are whitelisted until FORCE UNFREEZE
    frozen_files = {
        "ZipBlockParallelCompressor.swift",
        "ZipBlockParallelDecompressor.swift",
        "ZipParallelExtractor.swift",
        "ZipParallelWriter.swift",
        "ZipCryptoEngine.swift",
        "ZipCentralDirectoryReader.swift",
        "ZipStoreStreamWriter.swift"
    }

    for folder in hot_dirs:
        for path in folder.rglob("*.swift"):
            if path.name in frozen_files:
                continue
            lines = scan_file_lines(path)
            for idx, line in enumerate(lines, 1):
                stripped = line.strip()
                if stripped.startswith("//") or stripped.startswith("/*"):
                    continue
                if pattern.search(line):
                    rel_path = str(path.relative_to(root_dir))
                    violations.append({
                        "ruleId": "NO_HOTPATH_DATA_COUNT",
                        "filePath": rel_path,
                        "lineNumber": idx,
                        "snippet": line.strip(),
                        "remediationHint": "Use UnsafeMutablePointer.allocate + Data(bytesNoCopy:) to avoid kernel zero-fill page faults."
                    })
    return violations

def lint_concurrent_perform_locks(root_dir):
    violations = []
    swift_core = root_dir / "Sources" / "TTZipCore"
    
    frozen_files = {
        "ZipBlockParallelCompressor.swift",
        "ZipBlockParallelDecompressor.swift",
        "ZipParallelExtractor.swift",
        "ZipParallelWriter.swift",
        "ZipCryptoEngine.swift",
        "ZipCentralDirectoryReader.swift",
        "ZipStoreStreamWriter.swift"
    }
    
    for path in swift_core.rglob("*.swift"):
        if path.name in frozen_files:
            continue
        lines = scan_file_lines(path)
        inside_concurrent_perform = False
        brace_depth = 0
        target_depth = 0
        
        for idx, line in enumerate(lines, 1):
            if "DispatchQueue.concurrentPerform" in line:
                inside_concurrent_perform = True
                brace_depth = 0
                target_depth = 0

            if inside_concurrent_perform:
                open_braces = line.count("{")
                close_braces = line.count("}")
                brace_depth += open_braces - close_braces
                
                # Check for locks
                stripped = line.strip()
                if not (stripped.startswith("//") or stripped.startswith("/*")):
                    if re.search(r'\.lock\(\)|pthread_mutex_lock|DispatchSemaphore\b.*\.wait\(', line):
                        rel_path = str(path.relative_to(root_dir))
                        violations.append({
                            "ruleId": "NO_CONCURRENT_PERFORM_LOCK",
                            "filePath": rel_path,
                            "lineNumber": idx,
                            "snippet": line.strip(),
                            "remediationHint": "Eliminate locks in concurrentPerform iterations. Use lock-free atomics (OSAtomicCompareAndSwap32Barrier)."
                        })

                if brace_depth <= 0 and open_braces > 0:
                    inside_concurrent_perform = False

    return violations

def main():
    root_dir = get_project_root()
    all_violations = []
    
    all_violations.extend(lint_hardcoded_paths(root_dir))
    all_violations.extend(lint_bare_logging(root_dir))
    all_violations.extend(lint_hotpath_data_count(root_dir))
    all_violations.extend(lint_concurrent_perform_locks(root_dir))
    
    output_json = {
        "scanTimestamp": "2026-08-17T05:12:00Z",
        "totalViolationsCount": len(all_violations),
        "passed": len(all_violations) == 0,
        "violations": all_violations
    }
    
    if "--json" in sys.argv:
        print(json.dumps(output_json, indent=2))
    else:
        print(f"[LINT] Scanned {root_dir}. Total violations: {len(all_violations)}")
        for v in all_violations:
            print(f"  ❌ [{v['ruleId']}] {v['filePath']}:{v['lineNumber']}")
            print(f"     Code: {v['snippet']}")
            print(f"     Hint: {v['remediationHint']}")
            
    if len(all_violations) > 0 and "--strict" in sys.argv:
        sys.exit(1)
    sys.exit(0)

if __name__ == "__main__":
    main()
