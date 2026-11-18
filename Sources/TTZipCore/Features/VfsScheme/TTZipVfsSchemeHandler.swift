// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
import TTZipArchiveContract
#if canImport(WebKit)
import TTZipWebKit
import WebKit
#endif

#if canImport(WebKit)
/// WebKit handler for the archive VFS scheme.
@MainActor
public final class TTZipVfsSchemeHandler: NSObject, WKURLSchemeHandler {
    public nonisolated static let scheme = ArchiveVfs.scheme

    private let handler: VfsSchemeHandler

    public init(provider: TTZipVfsResourceProvider) {
        handler = VfsSchemeHandler(
            read: { @Sendable uri in
                try await provider.loadResource(uri: uri)
            },
            readRange: { @Sendable uri, range in
                try await provider.loadResourceRange(uri: uri, byteRange: range)
            }
        )
        super.init()
    }

    public func webView(_ webView: WKWebView, start urlSchemeTask: WKURLSchemeTask) {
        handler.webView(webView, start: urlSchemeTask)
    }

    public func webView(_ webView: WKWebView, stop urlSchemeTask: WKURLSchemeTask) {
        handler.webView(webView, stop: urlSchemeTask)
    }
}
#endif
