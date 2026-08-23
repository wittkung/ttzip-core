// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

import SwiftUI
import AppKit
import TTZipCore

extension NativeArchiveOutlineView.Coordinator {
    public func outlineView(_ outlineView: NSOutlineView, numberOfChildrenOfItem item: Any?) -> Int {
        if item == nil {
            return rootItems.count
        }
        if let outlineItem = item as? ArchiveOutlineItem {
            return outlineItem.children.count
        }
        return 0
    }
    
    public func outlineView(_ outlineView: NSOutlineView, child index: Int, ofItem item: Any?) -> Any {
        if item == nil {
            return rootItems[index]
        }
        if let outlineItem = item as? ArchiveOutlineItem {
            return outlineItem.children[index]
        }
        fatalError("Invalid item index")
    }
    
    public func outlineView(_ outlineView: NSOutlineView, isItemExpandable item: Any) -> Bool {
        if let outlineItem = item as? ArchiveOutlineItem {
            return outlineItem.isDirectory
        }
        return false
    }
    
    public func outlineViewItemWillExpand(_ notification: Notification) {
        NSAnimationContext.current.duration = 0.0
    }
    
    public func outlineViewItemWillCollapse(_ notification: Notification) {
        NSAnimationContext.current.duration = 0.18
    }
    
    public func outlineView(_ outlineView: NSOutlineView, viewFor tableColumn: NSTableColumn?, item: Any) -> NSView? {
        guard let outlineItem = item as? ArchiveOutlineItem else { return nil }
        let identifier = tableColumn?.identifier.rawValue ?? ""
        
        if identifier == "name" {
            let cellIdentifier = NSUserInterfaceItemIdentifier("ArchiveNodeCell")
            let cell = outlineView.makeView(withIdentifier: cellIdentifier, owner: nil) as? ArchiveNodeTableCellView
                ?? ArchiveNodeTableCellView(frame: .zero)
            cell.identifier = cellIdentifier
            
            let iconName = fileIconName(isDirectory: outlineItem.isDirectory, name: outlineItem.name)
            cell.configure(name: outlineItem.name, isDirectory: outlineItem.isDirectory, iconName: iconName)
            
            return cell
        } else if identifier == "size" {
            let cellIdentifier = NSUserInterfaceItemIdentifier("ArchiveSizeCell")
            let tf = outlineView.makeView(withIdentifier: cellIdentifier, owner: nil) as? NSTextField
                ?? {
                    let field = NSTextField(labelWithString: "")
                    field.identifier = cellIdentifier
                    field.font = .systemFont(ofSize: 12)
                    field.textColor = .secondaryLabelColor
                    field.alignment = .right
                    return field
                }()
            tf.stringValue = outlineItem.isDirectory ? "--" : formatBytes(outlineItem.uncompressedSize)
            return tf
        } else if identifier == "encoding" {
            let cellIdentifier = NSUserInterfaceItemIdentifier("ArchiveEncodingCell")
            let tf = outlineView.makeView(withIdentifier: cellIdentifier, owner: nil) as? NSTextField
                ?? {
                    let field = NSTextField(labelWithString: "")
                    field.identifier = cellIdentifier
                    field.font = .systemFont(ofSize: 11, weight: .medium)
                    field.textColor = .systemBlue
                    field.alignment = .center
                    return field
                }()
            tf.stringValue = outlineItem.detectedEncoding
            return tf
        }
        
        return nil
    }
    
    public func outlineViewSelectionDidChange(_ notification: Notification) {
        guard let outlineView = notification.object as? NSOutlineView else { return }
        let selectedRow = outlineView.selectedRow
        guard selectedRow >= 0, let outlineItem = outlineView.item(atRow: selectedRow) as? ArchiveOutlineItem else { return }
        DispatchQueue.main.async {
            self.parent.selectedPath = outlineItem.node.id
            self.parent.onSelectFile(outlineItem.node)
        }
    }
    
    func fileIconName(isDirectory: Bool, name: String) -> String {
        if isDirectory { return "folder.fill" }
        let ext = (name as NSString).pathExtension.lowercased()
        switch ext {
        case "png", "jpg", "jpeg", "gif", "webp", "heic", "svg", "bmp": return "photo.fill"
        case "mp4", "mov", "m4v", "avi", "mkv": return "film.fill"
        case "mp3", "wav", "m4a", "aac", "flac": return "music.note"
        case "pdf": return "doc.richtext.fill"
        case "swift", "json", "c", "cpp", "h", "md", "py", "sh", "xml", "html", "css": return "doc.text.fill"
        default: return "doc.fill"
        }
    }
    
    func formatBytes(_ bytes: Int64) -> String {
        return ByteCountFormatterCache.string(fromByteCount: bytes)
    }
}
