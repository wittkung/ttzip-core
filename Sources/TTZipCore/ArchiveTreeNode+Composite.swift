// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

// MARK: - ArchiveTreeNode Composite Protocol Conformance

extension ArchiveTreeNode: ArchiveComponentProtocol {
    public var sizeBytes: Int64 {
        if isDirectory, uncompressedSize == 0, let explicit = _children, !explicit.isEmpty {
            return explicit.reduce(0) { $0 + $1.sizeBytes }
        }
        return uncompressedSize
    }

    public func getChildren() -> [ArchiveComponentProtocol] {
        if let direct = _children, !direct.isEmpty {
            return direct
        }
        if let tree = vfsTree, isDirectory {
            var accumulated: [ArchiveTreeNode] = []
            var currentOffset = 0
            let batchSize = 1000
            while true {
                let paged = tree.getChildrenPaged(subpath: path, offset: UInt32(currentOffset), limit: UInt32(batchSize))
                let valid = paged.nodes.filter { !$0.name.isEmpty && $0.name != "." && $0.path != path }
                accumulated.append(contentsOf: valid.map { ArchiveTreeNode(summary: $0, vfsTree: tree) })
                currentOffset += paged.nodes.count
                if currentOffset >= Int(paged.totalCount) || paged.nodes.isEmpty {
                    break
                }
            }
            return accumulated
        }
        return []
    }

    /// Maximum depth of eagerly materialized `ArchiveCompositeDirectory` value-type wrappers.
    /// Beyond this depth, child directories remain attached as lazy `ArchiveTreeNode` components
    /// (`ArchiveComponentProtocol`) inside `ArchiveCompositeDirectory.customComponents`, preventing
    /// both silent empty-directory truncation and recursive value-witness stack overflow on deallocation.
    private static let maxEagerCompositeDepth = 64

    private struct ComponentWorkFrame {
        let node: ArchiveTreeNode
        let childNodes: [ArchiveTreeNode]
        var nextChildIndex: Int
        var builtChildren: [ArchiveComponentProtocol]
    }

    private static func filterValidChildNodes(
        of parent: ArchiveTreeNode,
        activeAncestors: Set<String>
    ) -> [ArchiveTreeNode] {
        (parent.children ?? []).filter { child in
            !child.name.isEmpty &&
                child.name != "." &&
                child.path != parent.path &&
                (!child.isDirectory || !activeAncestors.contains(child.path))
        }
    }

    private static func makeLeafComponent(from node: ArchiveTreeNode) -> ArchiveLeafFile {
        ArchiveLeafFile(
            name: node.name,
            path: node.path,
            sizeBytes: node.uncompressedSize,
            entry: node.entry,
            modificationDate: node.modificationDate,
            compressedSizeBytes: node._compressedSize,
            crc32: node._crc32,
            posixPermissions: node._posixPermissions,
            isEncrypted: node._isEncrypted,
            detectedEncoding: node._detectedEncoding
        )
    }

    private static func makeLeafNode(
        from leaf: ArchiveLeafFile,
        fallbackEncoding: String
    ) -> ArchiveTreeNode {
        ArchiveTreeNode(
            name: leaf.name,
            path: leaf.path,
            uncompressedSize: leaf.sizeBytes,
            isDirectory: false,
            detectedEncoding: leaf.detectedEncoding ?? leaf.entry?.detectedEncoding ?? fallbackEncoding,
            children: nil,
            entry: leaf.entry,
            compressedSize: leaf.compressedSizeBytes ?? leaf.entry?.compressedSize,
            crc32: leaf.crc32 ?? leaf.entry?.crc32,
            modificationDate: leaf.modificationDate ?? leaf.entry?.modificationDate,
            posixPermissions: leaf.posixPermissions ?? leaf.entry?.posixPermissions,
            isEncrypted: leaf.isEncrypted ?? leaf.entry?.isEncrypted,
            hasChildren: false,
            vfsTree: nil
        )
    }

    /// Materializes this node into an `ArchiveComponentProtocol` tree using an explicit heap worklist.
    /// Deep hierarchies (> 64 levels) seamlessly transition to lazy `ArchiveTreeNode` components so
    /// legitimate deep directories are never truncated and value destruction never overflows the
    /// 544 KB Swift Concurrency cooperative thread stack.
    public func toComponent(visitedPaths: Set<String> = [], depth: Int = 0) -> ArchiveComponentProtocol {
        guard isDirectory else {
            return Self.makeLeafComponent(from: self)
        }
        if depth >= Self.maxEagerCompositeDepth {
            return self
        }

        var activeAncestors = visitedPaths
        activeAncestors.insert(path)
        let rootChildren = Self.filterValidChildNodes(of: self, activeAncestors: activeAncestors)
        var stack = [
            ComponentWorkFrame(node: self, childNodes: rootChildren, nextChildIndex: 0, builtChildren: [])
        ]

        while let topIndex = stack.indices.last {
            if stack[topIndex].nextChildIndex < stack[topIndex].childNodes.count {
                let child = stack[topIndex].childNodes[stack[topIndex].nextChildIndex]
                stack[topIndex].nextChildIndex += 1
                if child.isDirectory {
                    guard !activeAncestors.contains(child.path) else { continue }
                    if depth + stack.count >= Self.maxEagerCompositeDepth {
                        stack[topIndex].builtChildren.append(child)
                    } else {
                        activeAncestors.insert(child.path)
                        let grandChildren = Self.filterValidChildNodes(of: child, activeAncestors: activeAncestors)
                        stack.append(
                            ComponentWorkFrame(
                                node: child,
                                childNodes: grandChildren,
                                nextChildIndex: 0,
                                builtChildren: []
                            )
                        )
                    }
                } else {
                    stack[topIndex].builtChildren.append(Self.makeLeafComponent(from: child))
                }
            } else {
                let completed = stack.removeLast()
                activeAncestors.remove(completed.node.path)
                let dirComp = ArchiveCompositeDirectory(
                    name: completed.node.name,
                    path: completed.node.path,
                    entry: completed.node.entry,
                    modificationDate: completed.node.modificationDate,
                    compressedSizeBytes: completed.node._compressedSize,
                    crc32: completed.node._crc32,
                    posixPermissions: completed.node._posixPermissions,
                    isEncrypted: completed.node._isEncrypted,
                    detectedEncoding: completed.node._detectedEncoding,
                    children: completed.builtChildren
                )
                if let parentIndex = stack.indices.last {
                    stack[parentIndex].builtChildren.append(dirComp)
                } else {
                    return dirComp
                }
            }
        }

        return ArchiveCompositeDirectory(
            name: name,
            path: path,
            entry: entry,
            modificationDate: modificationDate,
            compressedSizeBytes: _compressedSize,
            crc32: _crc32,
            posixPermissions: _posixPermissions,
            isEncrypted: _isEncrypted,
            detectedEncoding: _detectedEncoding,
            children: []
        )
    }

    private struct NodeReconstructionFrame {
        let component: ArchiveComponentProtocol
        let childComponents: [ArchiveComponentProtocol]
        var nextChildIndex: Int
        var builtChildren: [ArchiveTreeNode]
    }

    private static func filterValidChildComponents(
        of parent: ArchiveComponentProtocol,
        activeAncestors: Set<String>
    ) -> [ArchiveComponentProtocol] {
        parent.getChildren().filter { child in
            !child.name.isEmpty &&
                child.name != "." &&
                child.path != parent.path &&
                (!child.isDirectory || !activeAncestors.contains(child.path))
        }
    }

    private static func makeDirectoryNode(
        from component: ArchiveComponentProtocol,
        children: [ArchiveTreeNode],
        fallbackEncoding: String
    ) -> ArchiveTreeNode {
        let composite = component as? ArchiveCompositeDirectory
        let entry = composite?.entry
        let totalUncompressed = children.isEmpty
            ? component.sizeBytes
            : children.reduce(Int64(0)) { $0 + $1.uncompressedSize }
        let explicitCompressed = composite?.compressedSizeBytes ?? entry?.compressedSize
        let resolvedCompressed = directoryCompressedSize(explicit: explicitCompressed, children: children)
        return ArchiveTreeNode(
            name: component.name,
            path: component.path,
            uncompressedSize: totalUncompressed,
            isDirectory: true,
            detectedEncoding: composite?.detectedEncoding ?? entry?.detectedEncoding ?? fallbackEncoding,
            children: children,
            entry: entry,
            compressedSize: resolvedCompressed,
            crc32: composite?.crc32 ?? entry?.crc32,
            modificationDate: composite?.modificationDate ?? entry?.modificationDate,
            posixPermissions: composite?.posixPermissions ?? entry?.posixPermissions,
            isEncrypted: composite?.isEncrypted ?? entry?.isEncrypted,
            hasChildren: !children.isEmpty,
            vfsTree: nil
        )
    }

    private static func directoryCompressedSize(explicit: Int64?, children: [ArchiveTreeNode]) -> Int64? {
        if let explicit, explicit > 0 || children.isEmpty {
            return explicit
        }
        let childCompressed = children.reduce(Int64(0)) { $0 + $1.compressedSize }
        return childCompressed > 0 ? childCompressed : explicit
    }

    private static func reconstructDirectoryNodeIteratively(
        from rootComponent: ArchiveComponentProtocol,
        fallbackEncoding: String
    ) -> ArchiveTreeNode {
        var activeAncestors: Set<String> = [rootComponent.path]
        let rootChildren = filterValidChildComponents(of: rootComponent, activeAncestors: activeAncestors)
        var stack = [
            NodeReconstructionFrame(
                component: rootComponent,
                childComponents: rootChildren,
                nextChildIndex: 0,
                builtChildren: []
            )
        ]

        while let topIndex = stack.indices.last {
            if stack[topIndex].nextChildIndex < stack[topIndex].childComponents.count {
                let child = stack[topIndex].childComponents[stack[topIndex].nextChildIndex]
                stack[topIndex].nextChildIndex += 1
                if let existingNode = child as? ArchiveTreeNode {
                    stack[topIndex].builtChildren.append(existingNode)
                } else if let leaf = child as? ArchiveLeafFile {
                    stack[topIndex].builtChildren.append(makeLeafNode(from: leaf, fallbackEncoding: fallbackEncoding))
                } else if child.isDirectory {
                    guard !activeAncestors.contains(child.path) else { continue }
                    activeAncestors.insert(child.path)
                    let grandChildren = filterValidChildComponents(of: child, activeAncestors: activeAncestors)
                    stack.append(
                        NodeReconstructionFrame(
                            component: child,
                            childComponents: grandChildren,
                            nextChildIndex: 0,
                            builtChildren: []
                        )
                    )
                } else {
                    stack[topIndex].builtChildren.append(
                        ArchiveTreeNode(
                            name: child.name,
                            path: child.path,
                            uncompressedSize: child.sizeBytes,
                            isDirectory: false,
                            detectedEncoding: fallbackEncoding
                        )
                    )
                }
            } else {
                let completed = stack.removeLast()
                activeAncestors.remove(completed.component.path)
                let builtNode = makeDirectoryNode(
                    from: completed.component,
                    children: completed.builtChildren,
                    fallbackEncoding: fallbackEncoding
                )
                if let parentIndex = stack.indices.last {
                    stack[parentIndex].builtChildren.append(builtNode)
                } else {
                    return builtNode
                }
            }
        }

        return makeDirectoryNode(from: rootComponent, children: [], fallbackEncoding: fallbackEncoding)
    }

    /// Reconstructs an `ArchiveTreeNode` from any `ArchiveComponentProtocol` (including lazy `ArchiveTreeNode`,
    /// `ArchiveLeafFile`, and `ArchiveCompositeDirectory`) without losing deep lazy children or metadata.
    public init(component: ArchiveComponentProtocol, detectedEncoding: String = "UTF-8") {
        if let existingNode = component as? ArchiveTreeNode {
            self = existingNode
            return
        }
        if let leaf = component as? ArchiveLeafFile {
            self = Self.makeLeafNode(from: leaf, fallbackEncoding: detectedEncoding)
            return
        }
        if component.isDirectory {
            self = Self.reconstructDirectoryNodeIteratively(
                from: component,
                fallbackEncoding: detectedEncoding
            )
            return
        }
        self = ArchiveTreeNode(
            name: component.name,
            path: component.path,
            uncompressedSize: component.sizeBytes,
            isDirectory: false,
            detectedEncoding: detectedEncoding
        )
    }
}
