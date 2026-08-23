// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

import Foundation
import TTZipCore

extension AppViewState {
    // MARK: - Task State Control
    
    public func pauseCurrentTask() {
        taskState.currentTaskHandle?.pause()
        self.canPauseTask = false
        self.canResumeTask = true
        self.taskStateName = "Paused"
    }
    
    public func resumeCurrentTask() {
        taskState.currentTaskHandle?.resume()
        self.canPauseTask = true
        self.canResumeTask = false
        self.taskStateName = "Processing"
    }
    
    public func cancelCurrentTask() {
        taskState.currentTaskHandle?.cancel(reason: 0)
        taskState.currentTask?.cancel()
        self.canPauseTask = false
        self.canResumeTask = false
        self.canCancelTask = false
        self.taskStateName = "Cancelled"
    }
    
    public func updateTaskStateUI() {
        self.taskStateName = "Idle"
        self.canPauseTask = false
        self.canResumeTask = false
        self.canCancelTask = false
        taskState.currentTaskHandle = nil
        taskState.currentTask = nil
    }
}
