#!/usr/bin/env python3
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine for macOS.
# ==============================================================================
# scripts/lint_cabi_context.py
# Bidirectional C-ABI static dead code and dropped struct field context linter.
# Uses Clang AST JSON (no external Python dependencies required).
# ==============================================================================

import sys
import os
import re
import json
import subprocess
from pathlib import Path
from typing import Dict, List, Set, Any, Tuple

def get_project_root() -> Path:
    return Path(__file__).resolve().parent.parent

class ClangAstExtractor:
    """Extracts C-ABI functions and struct definitions using native Clang JSON AST."""
    
    @staticmethod
    def extract(header_path: Path) -> Tuple[Dict[str, Any], Dict[str, List[str]]]:
        cmd = [
            "/usr/bin/clang", "-fsyntax-only", "-Xclang", "-ast-dump=json",
            "-I", str(header_path.parent),
            str(header_path)
        ]
        res = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        if res.returncode != 0:
            raise RuntimeError(f"Clang AST dump failed: {res.stderr}")
            
        data = json.loads(res.stdout)
        functions: Dict[str, Any] = {}
        structs: Dict[str, List[str]] = {}
        
        def walk(node: Dict[str, Any]):
            kind = node.get("kind")
            loc = node.get("loc", {})
            
            if kind == "FunctionDecl":
                name = node.get("name", "")
                if name.startswith("ttzip_rust_"):
                    functions[name] = {
                        "line": loc.get("line", 0),
                        "type": node.get("type", {}).get("qualType", "")
                    }
                    
            elif kind == "RecordDecl" and node.get("tagUsed") == "struct":
                name = node.get("name", "")
                if name.startswith("TTZip"):
                    fields = []
                    for inner in node.get("inner", []):
                        if inner.get("kind") == "FieldDecl":
                            fields.append(inner.get("name", ""))
                    if fields:
                        structs[name] = fields
                        
            for child in node.get("inner", []):
                walk(child)
                
        walk(data)
        return functions, structs

class SwiftConsumptionScanner:
    """Scans Swift codebase for function calls, struct field references, and wildcard ignores."""
    
    def __init__(self, root_dir: Path):
        self.root_dir = root_dir
        self.sources_dir = root_dir / "Sources"
        self.tests_dir = root_dir / "Tests"
        
    def scan_calls_and_fields(self, struct_schemas: Dict[str, List[str]]) -> Tuple[Set[str], Dict[str, Set[str]], List[Dict[str, Any]]]:
        called_functions: Set[str] = set()
        accessed_fields: Dict[str, Set[str]] = {s: set() for s in struct_schemas}
        violations: List[Dict[str, Any]] = []
        
        fn_pattern = re.compile(r'\b(ttzip_rust_[A-Za-z0-9_]+)\b')
        ignore_pattern = re.compile(r'let\s+_\s*=\s*[A-Za-z0-9_.]+\.([A-Za-z0-9_]+)')
        
        target_paths = list(self.sources_dir.rglob("*.swift")) + list(self.tests_dir.rglob("*.swift"))
        
        for path in target_paths:
            try:
                content = path.read_text(encoding="utf-8")
            except Exception:
                continue
                
            lines = content.splitlines()
            for line_idx, line in enumerate(lines, 1):
                stripped = line.strip()
                if stripped.startswith("//") or stripped.startswith("/*") or stripped.startswith("*"):
                    continue
                    
                # 1. Match functions
                for fn in fn_pattern.findall(line):
                    called_functions.add(fn)
                    
                # 2. Match field access (e.g. .detected_encoding or .uncompressed_size)
                for struct_name, fields in struct_schemas.items():
                    for field in fields:
                        if f".{field}" in line or f"{field}:" in line:
                            accessed_fields[struct_name].add(field)
                            
                # 3. Match wildcard ignore pattern
                m_ign = ignore_pattern.search(line)
                if m_ign:
                    dropped_field = m_ign.group(1)
                    rel_path = str(path.relative_to(self.root_dir))
                    violations.append({
                        "ruleId": "CABI_005_WILDCARD_IGNORE_DETECTED",
                        "filePath": rel_path,
                        "lineNumber": line_idx,
                        "snippet": stripped,
                        "remediationHint": f"Field '{dropped_field}' is explicitly discarded. Route context to Swift model."
                    })
                    
        return called_functions, accessed_fields, violations

def load_exemptions(root_dir: Path) -> Tuple[Set[str], Dict[str, Set[str]]]:
    exemption_file = root_dir / "scripts" / "cabi_exemptions.json"
    if not exemption_file.exists():
        return set(), {}
    try:
        with open(exemption_file, "r", encoding="utf-8") as f:
            data = json.load(f)
            fn_exempt = set(data.get("function_exemptions", []))
            struct_exempt = {k: set(v) for k, v in data.get("struct_field_exemptions", {}).items()}
            return fn_exempt, struct_exempt
    except Exception:
        return set(), {}

def main():
    root = get_project_root()
    header_path = root / "Sources" / "CTTZipBridge" / "include" / "ttzip_rust_glue.h"
    
    if not header_path.exists():
        print(f"❌ Header not found: {header_path}", file=sys.stderr)
        sys.exit(1)
        
    fn_exempt, struct_exempt = load_exemptions(root)
    
    cabi_funcs, cabi_structs = ClangAstExtractor.extract(header_path)
    scanner = SwiftConsumptionScanner(root)
    called_funcs, accessed_fields, violations = scanner.scan_calls_and_fields(cabi_structs)
    
    # 1. Check Dead Exports (CABI_001)
    header_func_set = set(cabi_funcs.keys())
    dead_funcs = header_func_set - called_funcs - fn_exempt
    for df in sorted(dead_funcs):
        violations.append({
            "ruleId": "CABI_001_DEAD_CABI_EXPORT",
            "filePath": "Sources/CTTZipBridge/include/ttzip_rust_glue.h",
            "lineNumber": cabi_funcs[df]["line"],
            "snippet": f"Exported C-ABI function: {df}",
            "remediationHint": f"Function '{df}' is exported in C-ABI header but 0 callers found in Swift. Wire into Swift or register exemption in scripts/cabi_exemptions.json."
        })
        
    # 2. Check Dropped Struct Fields (CABI_003)
    for struct_name, all_fields in cabi_structs.items():
        used = accessed_fields.get(struct_name, set())
        exempt = struct_exempt.get(struct_name, set())
        dropped = set(all_fields) - used - exempt
        for df in sorted(dropped):
            violations.append({
                "ruleId": "CABI_003_STRUCT_FIELD_DROPPED",
                "filePath": "Sources/CTTZipBridge/include/ttzip_rust_glue.h",
                "lineNumber": 0,
                "snippet": f"Struct '{struct_name}' field '{df}'",
                "remediationHint": f"Field '{df}' in '{struct_name}' is not accessed in Swift code. Ensure FFI context integrity."
            })
            
    is_json = "--json" in sys.argv
    is_strict = "--strict" in sys.argv
    
    if is_json:
        print(json.dumps({
            "totalViolations": len(violations),
            "passed": len(violations) == 0,
            "violations": violations
        }, indent=2))
    else:
        print("======================================================================")
        print("   TTZip Bidirectional C-ABI & Struct Context Linter                 ")
        print("======================================================================")
        print(f"Header Functions: {len(header_func_set)} | Swift Calls: {len(called_funcs)}")
        print(f"C Structs Audited: {len(cabi_structs)} | Violations: {len(violations)}")
        print("----------------------------------------------------------------------")
        if not violations:
            print("✅ [PASS] 100% C-ABI & FFI Context Parity Verified (0 violations).")
        else:
            for v in violations:
                print(f"❌ [{v['ruleId']}] {v['filePath']}:{v['lineNumber']}")
                print(f"   Snippet: {v['snippet']}")
                print(f"   Hint:    {v['remediationHint']}")
                print("")
                
    if violations and is_strict:
        sys.exit(1)
    sys.exit(0)

if __name__ == "__main__":
    main()
