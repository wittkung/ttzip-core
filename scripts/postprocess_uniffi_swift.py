#!/usr/bin/env python3
# Copyright 2026 MetastudyLine. All rights reserved.
# SPDX-License-Identifier: BSD-3-Clause

"""
Mozilla UniFFI Swift Bindings Postprocessing and Invariant Verification Engine.

This tool hardens UniFFI-generated Swift bindings for ttzip_engine against:
1. Swift 6 strict concurrency violations (annotating mutable static globals with
   `nonisolated(unsafe)`, enforcing `Sendable` conformances on classes/protocols).
2. Memory leaks during buffer deserialization (injecting `defer { buf.deallocate() }`
   inside `FfiConverterRustBuffer.lift(_:)` so errors during `read` do not leak buffers).
3. Fatal crashes during object destruction (replacing throwing `try! rustCall` in `deinit`
   with non-fatal `_ = try? rustCall`).
4. Process termination on cooperative cancellation (mapping `CALL_CANCELLED` to buffer
   deallocation and throwing `CancellationError()` instead of calling `fatalError`).
5. Null-pointer dereferences on uninitialized/zero-capacity buffers in `Data.init(rustBuffer:)`
   and `RustBuffer.deallocate()`.
6. Monorepo invariant linter violations (stripping bare `print()` calls in scaffolding).

Strict Fail-Fast Gate:
Every replacement is backed by an explicit `PatchDescriptor` with an expected match
count >= 1. If any patch fails to match (e.g. due to UniFFI template changes), the script
aborts immediately with exit code 1 and rich diagnostic diff output. It also performs
lexical validation of Swift syntax (balanced delimiters and proper string/comment termination).
"""

import argparse
import os
import re
import sys
from dataclasses import dataclass
from typing import Callable, List, Optional, Tuple, Union


PROPRIETARY_HEADER = """// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

"""

SENDABLE_EXTENSIONS = """
// MARK: - Swift 6 Strict Concurrency Conformances
extension UniFfiVfsTree: @unchecked Sendable {}
extension CancellationToken: @unchecked Sendable {}
extension UniFfiPluginSigner: @unchecked Sendable {}
extension UniFfiPluginVerifier: @unchecked Sendable {}
extension UniFfiMmapReader: @unchecked Sendable {}
extension ExtractionSession: @unchecked Sendable {}
extension UniFfiCancellationToken: @unchecked Sendable {}
extension CollisionPolicy: Sendable {}
extension CollisionAction: Sendable {}
extension ExtractionRequest: Sendable {}
extension StageReport: Sendable {}
extension ExtractionOutcome: Sendable {}
extension ConflictSummary: Sendable {}
extension ConflictPage: Sendable {}
extension CollisionRecord: Sendable {}
extension ResolutionSet: Sendable {}
extension ResolutionOverride: Sendable {}
extension OutcomeStatus: Sendable {}
extension SessionState: Sendable {}
extension FailedItem: Sendable {}
extension FailureReason: Sendable {}
extension ItemKind: Sendable {}
extension ItemMeta: Sendable {}
extension ConflictKind: Sendable {}
extension DetectionPhase: Sendable {}
extension CommitResult: Sendable {}
extension VfsSession: @unchecked Sendable {}
extension VfsPagedNode: Sendable {}
extension VfsPageResult: Sendable {}
extension VfsNodeDetail: Sendable {}
extension VfsSortKey: Sendable {}
extension VfsSortDirection: Sendable {}
extension VfsSessionStatus: Sendable {}
extension EncryptedHeaderChallenge: @unchecked Sendable {}
extension VfsMissingVolumeDescriptor: Sendable {}
extension VfsVolumePartInfo: Sendable {}
extension VfsMissingVolumeSeverity: Sendable {}
extension UniFfiSemanticSearchEngine: @unchecked Sendable {}
extension UniFfiSemanticArchiveEntryInput: Sendable {}
extension UniFfiSemanticIndexProgress: Sendable {}
extension UniFfiSemanticModelInfo: Sendable {}
extension UniFfiSemanticOpenRequest: Sendable {}
extension UniFfiSemanticScopeDescriptor: Sendable {}
extension UniFfiSemanticSearchHit: Sendable {}
extension UniFfiSemanticSearchRequest: Sendable {}
extension UniFfiSemanticSearchResponse: Sendable {}
extension UniFfiSemanticComputeBackend: Sendable {}
extension UniFfiSemanticIngestPhase: Sendable {}
extension UniFfiSemanticMatchKind: Sendable {}
extension UniFfiSemanticModelState: Sendable {}
extension UniFfiSemanticScopeKind: Sendable {}
public typealias UniFFISemanticSearchEngineProtocol = UniFfiSemanticSearchEngineProtocol
public typealias UniFFISemanticSearchEngine = UniFfiSemanticSearchEngine
public typealias UniFFISemanticArchiveEntryInput = UniFfiSemanticArchiveEntryInput
public typealias UniFFISemanticIndexProgress = UniFfiSemanticIndexProgress
public typealias UniFFISemanticModelInfo = UniFfiSemanticModelInfo
public typealias UniFFISemanticOpenRequest = UniFfiSemanticOpenRequest
public typealias UniFFISemanticScopeDescriptor = UniFfiSemanticScopeDescriptor
public typealias UniFFISemanticSearchHit = UniFfiSemanticSearchHit
public typealias UniFFISemanticSearchRequest = UniFfiSemanticSearchRequest
public typealias UniFFISemanticSearchResponse = UniFfiSemanticSearchResponse
public typealias UniFFISemanticComputeBackend = UniFfiSemanticComputeBackend
public typealias UniFFISemanticIngestPhase = UniFfiSemanticIngestPhase
public typealias UniFFISemanticMatchKind = UniFfiSemanticMatchKind
public typealias UniFFISemanticModelState = UniFfiSemanticModelState
public typealias UniFFISemanticScopeKind = UniFfiSemanticScopeKind
"""



@dataclass(frozen=True)
class PatchDescriptor:
    """
    Descriptor defining an invariant transformation on UniFFI-generated Swift code.

    Attributes:
        name: Unique identifier for the patch.
        description: Rationale and invariant enforced by this patch.
        pattern: Regular expression targeting the raw UniFFI code construct.
        replacement: Substitution string or match callable.
        min_matches: Minimum acceptable match count. Mismatches trigger fail-fast abort.
        verify_pattern: Optional regex asserting the patch's post-condition is satisfied.
        forbidden_pattern: Optional regex asserting dangerous raw patterns do not linger.
    """

    name: str
    description: str
    pattern: re.Pattern
    replacement: Union[str, Callable[[re.Match], str]]
    min_matches: int = 1
    verify_pattern: Optional[re.Pattern] = None
    forbidden_pattern: Optional[re.Pattern] = None


PATCH_DESCRIPTORS: List[PatchDescriptor] = [
    PatchDescriptor(
        name="ffi_conditional_imports",
        description="Wrap UniFFI C-ABI imports with canImport checks for TTZipEngineFFI / ttzip_engineFFI",
        pattern=re.compile(
            r"(?:#if canImport\(ttzip_engineFFI\)\s*\n\s*import ttzip_engineFFI\s*\n\s*#endif|import CTTZipBridge)"
        ),
        replacement="""#if canImport(TTZipEngineFFI)
import TTZipEngineFFI
#elseif canImport(ttzip_engineFFI)
import ttzip_engineFFI
#endif""",
        min_matches=1,
        verify_pattern=re.compile(
            r"#if canImport\(TTZipEngineFFI\)\s*\nimport TTZipEngineFFI\s*\n#elseif canImport\(ttzip_engineFFI\)"
        ),
    ),
    PatchDescriptor(
        name="strict_concurrency_vtable",
        description="Annotate mutable static callback vtables with nonisolated(unsafe) for Swift 6 strict concurrency",
        pattern=re.compile(r"(?<!nonisolated\(unsafe\) )static var vtable:"),
        replacement="nonisolated(unsafe) static var vtable:",
        min_matches=1,
        verify_pattern=re.compile(r"nonisolated\(unsafe\) static var vtable:"),
        forbidden_pattern=re.compile(r"(?<!nonisolated\(unsafe\) )static var vtable:"),
    ),
    PatchDescriptor(
        name="strict_concurrency_handle_map",
        description="Annotate mutable fileprivate handleMaps with nonisolated(unsafe) for Swift 6 strict concurrency",
        pattern=re.compile(r"(?<!nonisolated\(unsafe\) )fileprivate static var handleMap ="),
        replacement="nonisolated(unsafe) fileprivate static var handleMap =",
        min_matches=1,
        verify_pattern=re.compile(r"nonisolated\(unsafe\) fileprivate static var handleMap ="),
        forbidden_pattern=re.compile(r"(?<!nonisolated\(unsafe\) )fileprivate static var handleMap ="),
    ),
    PatchDescriptor(
        name="strict_concurrency_initialization_result",
        description="Convert mutable initializationResult variable to let constant for thread-safety",
        pattern=re.compile(r"private var initializationResult:"),
        replacement="private let initializationResult:",
        min_matches=1,
        verify_pattern=re.compile(r"private let initializationResult:"),
        forbidden_pattern=re.compile(r"private var initializationResult:"),
    ),
    PatchDescriptor(
        name="strip_uniffi_free_print_calls",
        description="Strip bare print calls in uniffiFree callback handlers for invariant linter compliance",
        pattern=re.compile(r'print\("Uniffi callback interface ([^"]+): handle missing in uniffiFree"\)'),
        replacement=r"// Uniffi callback interface \1: handle missing in uniffiFree",
        min_matches=1,
        verify_pattern=re.compile(
            r"// Uniffi callback interface [^:]+: handle missing in uniffiFree"
        ),
        forbidden_pattern=re.compile(
            r'print\("Uniffi callback interface [^"]+: handle missing in uniffiFree"\)'
        ),

    ),
    PatchDescriptor(
        name="uniffi_handle_map_sendable",
        description="Mark UniffiHandleMap as @unchecked Sendable for concurrent callback dispatch",
        pattern=re.compile(r"fileprivate class UniffiHandleMap<T>(?!\s*:\s*@unchecked Sendable)\s*\{"),
        replacement="fileprivate class UniffiHandleMap<T>: @unchecked Sendable {",
        min_matches=1,
        verify_pattern=re.compile(r"fileprivate class UniffiHandleMap<T>:\s*@unchecked Sendable\s*\{"),
    ),
    PatchDescriptor(
        name="callback_protocols_sendable",
        description="Add Sendable inheritance to public callback protocols (ProgressHandler, UniFfiLogCallback, etc.)",
        pattern=re.compile(
            r"public protocol (ProgressHandler|UniFfiDeviceEventListener|UniFfiLogCallback|UniFfiProgressCallback)\s*:\s*AnyObject(?!\s*,\s*Sendable)\s*\{"
        ),
        replacement=r"public protocol \1 : AnyObject, Sendable {",
        min_matches=1,
        verify_pattern=re.compile(r"public protocol ProgressHandler\s*:\s*AnyObject,\s*Sendable\s*\{"),
    ),
    PatchDescriptor(
        name="defer_rustbuffer_deallocate_in_lift",
        description="Enforce defer { buf.deallocate() } at the beginning of lift(_:) to prevent RustBuffer leaks upon throwing",
        pattern=re.compile(
            r"(public static func lift\(_ buf: RustBuffer\) throws -> SwiftType \{\n)"
            r"(\s+)(var reader = createReader\(data: Data\(rustBuffer: buf\)\)\n"
            r"\s+let value = try read\(from: &reader\)\n"
            r"\s+if hasRemaining\(reader\) \{\n"
            r"\s+throw UniffiInternalError\.incompleteData\n"
            r"\s+\}\n)"
            r"\s+buf\.deallocate\(\)\n"
            r"(\s+return value\n"
            r"\s+\})"
        ),
        replacement=r"\1\2defer { buf.deallocate() }\n\2\3\4",
        min_matches=1,
        verify_pattern=re.compile(
            r"public static func lift\(_ buf: RustBuffer\) throws -> SwiftType \{\s+defer \{ buf\.deallocate\(\) \}"
        ),
        forbidden_pattern=re.compile(
            r"public static func lift\(_ buf: RustBuffer\) throws -> SwiftType \{\s+var reader"
        ),
    ),
    PatchDescriptor(
        name="deinit_non_fatal_try",
        description="Replace throwing try! rustCall in deinit with non-fatal _ = try? rustCall to prevent crashes",
        pattern=re.compile(r"([ \t]+)try!\s+rustCall\s*\{\s*(uniffi_[a-zA-Z0-9_]+_fn_free_[a-zA-Z0-9_]+)"),
        replacement=r"\1_ = try? rustCall { \2",
        min_matches=1,
        verify_pattern=re.compile(r"_ = try\? rustCall\s*\{\s*uniffi_[a-zA-Z0-9_]+_fn_free_"),
    ),
    PatchDescriptor(
        name="cancellation_error_throw",
        description="Replace CALL_CANCELLED fatalError with buffer deallocation and CancellationError throw",
        pattern=re.compile(r"case CALL_CANCELLED:\s*\n\s*fatalError\(\"Cancellation not supported yet\"\)"),
        replacement="""        case CALL_CANCELLED:
            callStatus.errorBuf.deallocate()
            throw CancellationError()""",
        min_matches=1,
        verify_pattern=re.compile(
            r"case CALL_CANCELLED:\s*\n\s*callStatus\.errorBuf\.deallocate\(\)\s*\n\s*throw CancellationError\(\)"
        ),
        forbidden_pattern=re.compile(r"case CALL_CANCELLED:\s*\n\s*fatalError"),
    ),
    PatchDescriptor(
        name="guard_data_init",
        description="Guard Data(rustBuffer:) against null pointer dereference and empty buffer edge cases",
        pattern=re.compile(
            r"fileprivate extension Data \{\n"
            r"\s+init\(rustBuffer: RustBuffer\) \{\n"
            r"\s+self\.init\(\n"
            r"\s+bytesNoCopy: rustBuffer\.data!,\n"
            r"\s+count: Int\(rustBuffer\.len\),\n"
            r"\s+deallocator: \.none\n"
            r"\s+\)\n"
            r"\s+\}\n"
            r"\}"
        ),
        replacement="""fileprivate extension Data {
    init(rustBuffer: RustBuffer) {
        guard let data = rustBuffer.data, rustBuffer.len > 0 else {
            self.init()
            return
        }
        self.init(
            bytesNoCopy: data,
            count: Int(rustBuffer.len),
            deallocator: .none
        )
    }
}""",
        min_matches=1,
        verify_pattern=re.compile(
            r"guard let data = rustBuffer\.data, rustBuffer\.len > 0 else \{\s*\n\s*self\.init\(\)\s*\n\s*return"
        ),
        forbidden_pattern=re.compile(r"bytesNoCopy:\s*rustBuffer\.data!"),
    ),
    PatchDescriptor(
        name="guard_rustbuffer_deallocate",
        description="Guard RustBuffer.deallocate against null pointers or 0 capacity to prevent double-free crashes",
        pattern=re.compile(
            r"([ \t]+)func deallocate\(\) \{\n"
            r"(?!\s+guard\s+self\.data)"
            r"\s+try!\s+rustCall\s*\{\s*(ffi_[a-zA-Z0-9_]+_rustbuffer_free\(self,\s*\$0\))\s*\}\n"
            r"\s+\}"
        ),
        replacement=r"""\1func deallocate() {
\1    guard self.data != nil && self.capacity > 0 else { return }
\1    try! rustCall { \2 }
\1}""",
        min_matches=1,
        verify_pattern=re.compile(
            r"guard self\.data != nil && self\.capacity > 0 else \{ return \}\s*\n\s*try! rustCall \{\s*ffi_[a-zA-Z0-9_]+_rustbuffer_free"
        ),
    ),
]


def apply_proprietary_header(content: str) -> Tuple[str, bool]:
    """
    Ensures the proprietary copyright header is present at the top of the file,
    stripping any upstream open-source / SPDX license headers generated by UniFFI.
    """
    if "SPDX-License-Identifier" in content:
        idx = content.find("// This file was autogenerated")
        if idx != -1:
            return PROPRIETARY_HEADER + content[idx:], True
        else:
            return PROPRIETARY_HEADER + content, True
    elif not content.startswith("// Copyright 2026 MetastudyLine"):
        return PROPRIETARY_HEADER + content, True
    return content, False


def apply_sendable_extensions(content: str) -> Tuple[str, bool]:
    """
    Appends required @unchecked Sendable conformance extensions to UniFFI reference types.
    """
    if "extension UniFfiSemanticScopeKind: Sendable" not in content:
        marker = "// MARK: - Swift 6 Strict Concurrency Conformances"
        if marker in content:
            content = content[:content.find(marker)].rstrip()
        return content.rstrip() + "\n" + SENDABLE_EXTENSIONS, True
    return content, False



def validate_swift_syntax(content: str) -> None:
    """
    Validates Swift syntax invariants on the postprocessed code:
    - Balanced braces {}, parentheses (), and brackets [].
    - Proper lexical closure of string literals ("" and multi-line "\"\"\"").
    - Proper termination and nesting of comments (single-line // and block /* */).
    
    Raises:
        SyntaxError: If any delimiter imbalance or unclosed token is discovered.
    """
    stack: List[Tuple[str, int, int]] = []
    
    in_single_line_comment = False
    in_multiline_comment_depth = 0
    in_multiline_string = False
    in_single_line_string = False
    
    i = 0
    n = len(content)
    line_num = 1
    col_num = 1
    
    delimiters = {"{": "}", "(": ")", "[": "]"}
    closing_delims = {v: k for k, v in delimiters.items()}
    
    while i < n:
        ch = content[i]
        cur_line = line_num
        cur_col = col_num
        
        if ch == "\n":
            line_num += 1
            col_num = 1
            in_single_line_comment = False
            if in_single_line_string:
                raise SyntaxError(
                    f"Unterminated single-line string literal at line {cur_line}, col {cur_col}"
                )
            i += 1
            continue
        else:
            col_num += 1
            
        # Ignore contents inside single-line comments
        if in_single_line_comment:
            i += 1
            continue
            
        # Handle nested block comments in Swift (Swift allows /* /* nested */ */)
        if in_multiline_comment_depth > 0:
            if ch == "/" and i + 1 < n and content[i + 1] == "*":
                in_multiline_comment_depth += 1
                i += 2
                col_num += 1
                continue
            elif ch == "*" and i + 1 < n and content[i + 1] == "/":
                in_multiline_comment_depth -= 1
                i += 2
                col_num += 1
                continue
            i += 1
            continue
            
        # Handle multi-line string literals
        if in_multiline_string:
            if ch == '"' and i + 2 < n and content[i : i + 3] == '"""':
                in_multiline_string = False
                i += 3
                col_num += 2
                continue
            elif ch == "\\" and i + 1 < n:
                i += 2
                col_num += 1
                continue
            i += 1
            continue
            
        # Handle standard single-line string literals
        if in_single_line_string:
            if ch == '"':
                in_single_line_string = False
                i += 1
                continue
            elif ch == "\\" and i + 1 < n:
                i += 2
                col_num += 1
                continue
            i += 1
            continue
            
        # Detect entry into comments
        if ch == "/" and i + 1 < n:
            if content[i + 1] == "/":
                in_single_line_comment = True
                i += 2
                col_num += 1
                continue
            elif content[i + 1] == "*":
                in_multiline_comment_depth = 1
                i += 2
                col_num += 1
                continue
                
        # Detect entry into strings
        if ch == '"':
            if i + 2 < n and content[i : i + 3] == '"""':
                in_multiline_string = True
                i += 3
                col_num += 2
                continue
            else:
                in_single_line_string = True
                i += 1
                continue
                
        # Outside comments and strings: verify delimiter balance
        if ch in delimiters:
            stack.append((ch, cur_line, cur_col))
        elif ch in closing_delims:
            if not stack:
                raise SyntaxError(
                    f"Unexpected closing delimiter '{ch}' at line {cur_line}, col {cur_col}"
                )
            open_ch, open_line, open_col = stack.pop()
            if delimiters[open_ch] != ch:
                raise SyntaxError(
                    f"Mismatched delimiter: expected '{delimiters[open_ch]}', found '{ch}' "
                    f"at line {cur_line}, col {cur_col} (opening '{open_ch}' at line {open_line}, col {open_col})"
                )
        i += 1

    if in_multiline_comment_depth > 0:
        raise SyntaxError("Unterminated multi-line comment at end of file")
    if in_multiline_string:
        raise SyntaxError("Unterminated multi-line string at end of file")
    if in_single_line_string:
        raise SyntaxError("Unterminated string literal at end of file")
    if stack:
        open_ch, open_line, open_col = stack[-1]
        raise SyntaxError(
            f"Unclosed delimiter '{open_ch}' opened at line {open_line}, col {open_col}"
        )


def verify_invariants(content: str, verbose: bool = False) -> Tuple[bool, List[str]]:
    """
    Checks whether all postprocess invariants are already satisfied in the content.
    Returns (is_satisfied, list_of_missing_or_violated_invariants).
    """
    violations = []
    
    # Check header
    if not content.startswith("// Copyright 2026 MetastudyLine"):
        violations.append("Missing proprietary copyright header at file start.")
        
    # Check Sendable extensions
    if "extension UniFfiSemanticScopeKind: Sendable" not in content:
        violations.append("Missing Sendable extensions for UniFFI classes.")
        
    for desc in PATCH_DESCRIPTORS:
        if desc.verify_pattern:
            if not desc.verify_pattern.search(content):
                violations.append(
                    f"Invariant '{desc.name}' verify pattern not satisfied: {desc.description}"
                )
            elif verbose:
                print(f"  [PASS] Verified invariant '{desc.name}'")
        if desc.forbidden_pattern:
            matches = desc.forbidden_pattern.findall(content)
            if matches:
                violations.append(
                    f"Forbidden pattern lingered for '{desc.name}' ({len(matches)} match(es)): {desc.description}"
                )
                
    return (len(violations) == 0, violations)


def postprocess_content(
    content: str,
    verify_only: bool = False,
    verbose: bool = False,
) -> Tuple[str, bool]:
    """
    Core postprocessing and verification engine.

    Returns:
        (processed_content, changed)

    Raises:
        RuntimeError: If any patch descriptor fails match requirements.
        SyntaxError: If the resulting Swift code fails structural syntax validation.
    """
    # 1. Fast-path check: Is the file already fully postprocessed and compliant?
    already_compliant, violations = verify_invariants(content, verbose=verbose)
    if already_compliant:
        validate_swift_syntax(content)
        if verbose:
            print("[INFO] Content already complies with all UniFFI Swift hardening invariants.")
        return content, False

    if verify_only:
        # In verify-only mode on a non-compliant file, report the violations
        error_msg = "\n".join(f"  - {v}" for v in violations)
        raise RuntimeError(
            f"Verification gate failed! File does not satisfy required postprocess invariants:\n{error_msg}"
        )

    # 2. Execute patch pipeline
    patched = content
    header_applied = False
    patched, header_applied = apply_proprietary_header(patched)
    if verbose and header_applied:
        print("  [APPLIED] Proprietary copyright header prepended.")

    for desc in PATCH_DESCRIPTORS:
        patched, count = desc.pattern.subn(desc.replacement, patched)
        if verbose:
            print(f"  [PATCH] {desc.name:36s} -> {count} match(es) (min: {desc.min_matches})")
        if count < desc.min_matches:
            # Check if invariant was already partially satisfied
            if desc.verify_pattern and desc.verify_pattern.search(patched):
                if verbose:
                    print(f"  [INFO] Invariant '{desc.name}' was already present.")
                continue
            # Patch failed! Raise detailed diagnostic error
            raise RuntimeError(
                f"Patch '{desc.name}' failed assertion!\n"
                f"Expected minimum matches: {desc.min_matches}, actual matches: {count}.\n"
                f"Invariant description: {desc.description}\n"
                f"Target regex: {desc.pattern.pattern}"
            )

    patched, sendable_applied = apply_sendable_extensions(patched)
    if verbose and sendable_applied:
        print("  [APPLIED] Sendable conformance extensions appended.")

    # 3. Post-verification of output
    is_valid, post_violations = verify_invariants(patched, verbose=False)
    if not is_valid:
        error_msg = "\n".join(f"  - {v}" for v in post_violations)
        raise RuntimeError(
            f"Post-patch invariant assertion failed:\n{error_msg}"
        )

    # 4. Lexical Swift syntax validation
    validate_swift_syntax(patched)
    if verbose:
        print("  [PASS] Swift lexical syntax validation passed (balanced delimiters & tokens).")

    return patched, True


def postprocess_file(
    swift_path: str,
    verify_only: bool = False,
    verbose: bool = False,
) -> None:
    """
    Reads, postprocesses, verifies, and optionally writes back a Swift bindings file.
    """
    if not os.path.isfile(swift_path):
        print(f"Error: Target Swift file not found: {swift_path}", file=sys.stderr)
        sys.exit(1)

    with open(swift_path, "r", encoding="utf-8") as f:
        content = f.read()

    try:
        new_content, changed = postprocess_content(
            content,
            verify_only=verify_only,
            verbose=verbose,
        )
    except (RuntimeError, SyntaxError) as err:
        print(f"\n❌ [GATE FAILURE] {swift_path}\n{err}\n", file=sys.stderr)
        sys.exit(1)

    if verify_only:
        print(f"✅ [VERIFIED] {swift_path} satisfies all UniFFI Swift 6 concurrency and safety invariants.")
        return

    if changed:
        with open(swift_path, "w", encoding="utf-8") as f:
            f.write(new_content)
        print(f"✅ [POSTPROCESSED] Successfully hardened {swift_path} for Swift 6 concurrency and memory safety.")
    else:
        print(f"ℹ️ [UNCHANGED] {swift_path} is already fully postprocessed and compliant.")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="UniFFI Swift Bindings Postprocessing and Concurrency Invariant Gate."
    )
    parser.add_argument(
        "swift_file",
        help="Path to UniFFI-generated Swift bindings file (e.g. ttzip_engine.swift)",
    )
    parser.add_argument(
        "--verify-only",
        "--dry-run",
        dest="verify_only",
        action="store_true",
        help="Run strict verification and assertion checks without modifying files on disk (for CI gates)",
    )
    parser.add_argument(
        "-v",
        "--verbose",
        action="store_true",
        help="Enable verbose diagnostic output during patch execution",
    )
    return parser


if __name__ == "__main__":
    cli_parser = build_parser()
    args = cli_parser.parse_args()
    postprocess_file(
        swift_path=args.swift_file,
        verify_only=args.verify_only,
        verbose=args.verbose,
    )
