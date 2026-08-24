// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import TTZipCore

extension AppViewState {
    // MARK: - Command Undo / Redo
    
    public func updateUndoRedoState() async {
        self.canUndo = await historyManager.canUndo
        self.canRedo = await historyManager.canRedo
        let descriptions = await historyManager.undoHistoryDescriptions
        self.lastCommandDescription = descriptions.last
    }
    
    @discardableResult
    public func executeCommand(_ command: ArchiveCommandProtocol) async throws -> CommandResult {
        guard !self.isLoading else {
            throw CommandError.invalidState(reason: "Another task is in progress.")
        }
        self.isLoading = true
        defer {
            self.isLoading = false
        }
        do {
            let result = try await historyManager.execute(command: command)
            self.statusMessage = "Command succeeded: [\(command.description)]"
            await updateUndoRedoState()
            return result
        } catch {
            self.statusMessage = "Command failed: \(error.localizedDescription)"
            await updateUndoRedoState()
            throw error
        }
    }
    
    public func performUndo() {
        guard !self.isLoading else { return }
        self.isLoading = true
        Task { @MainActor in
            defer {
                self.isLoading = false
            }
            do {
                let canUndoVal = await self.historyManager.canUndo
                if canUndoVal, let res = try await self.historyManager.undo() {
                    self.statusMessage = "Undone: \(res.message)"
                }
            } catch {
                self.statusMessage = "Undo failed: \(error.localizedDescription)"
            }
            await self.updateUndoRedoState()
        }
    }
    
    public func performRedo() {
        guard !self.isLoading else { return }
        self.isLoading = true
        Task { @MainActor in
            defer {
                self.isLoading = false
            }
            do {
                let canRedoVal = await self.historyManager.canRedo
                if canRedoVal, let res = try await self.historyManager.redo() {
                    self.statusMessage = "Redone: \(res.message)"
                }
            } catch {
                self.statusMessage = "Redo failed: \(error.localizedDescription)"
            }
            await self.updateUndoRedoState()
        }
    }
}
