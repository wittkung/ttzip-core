// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 4-step USB pipe stall and hardware FIFO recovery state machine.
//!
//! Provides deterministic fault-recovery escalating across four distinct stages:
//! 1. `AbortPipe`: Discards all pending in-flight USB requests on the stalled endpoint.
//! 2. `ClearPipeStallBothEnds`: Issues standard USB `ClearFeature(ENDPOINT_HALT)` and resets host FIFO.
//! 3. `ProtocolReset`: Invokes upper-layer session reset (e.g. MTP CancelRequest / DeviceReset, ADB A_CLSE).
//! 4. `ResetDevice`: Issues hardware port-level bus reset (SE0 signal), forcing full device re-enumeration.

use crate::error::DeviceError;

/// Individual stage in the 4-step pipe stall recovery state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecoveryStep {
    /// Normal quiescent state; no stall detected.
    Idle,
    /// Step 1: Terminate all in-flight asynchronous transfers on the pipe.
    AbortPipe,
    /// Step 2: Clear hardware stall condition on both device endpoint and host controller.
    ClearPipeStallBothEnds,
    /// Step 3: Application-layer protocol state reset.
    ProtocolReset,
    /// Step 4: Physical USB bus-level port reset forcing device re-enumeration.
    ResetDevice,
    /// Recovery completed successfully; pipe returned to operational state.
    Recovered,
    /// All recovery stages exhausted without restoring communication.
    Failed,
}

/// Target hardware or simulated transport capable of executing the 4 recovery operations.
pub trait TransportRecoveryTarget: Send {
    /// Step 1: Abort all queued transfers on the specified endpoint.
    fn abort_pipe(&mut self, ep_addr: u8) -> Result<(), DeviceError>;

    /// Step 2: Clear endpoint halt / stall feature on device and host controller.
    fn clear_pipe_stall(&mut self, ep_addr: u8) -> Result<(), DeviceError>;

    /// Step 3: Issue protocol-level reset sequence (e.g. MTP CancelRequest / ADB stream reset).
    fn protocol_reset(&mut self) -> Result<(), DeviceError>;

    /// Step 4: Issue hardware USB bus reset forcing bus re-enumeration.
    fn reset_device(&mut self) -> Result<(), DeviceError>;
}

/// Deterministic 4-step pipe stall recovery state machine.
#[derive(Debug, Clone)]
pub struct PipeRecoveryStateMachine {
    current_step: RecoveryStep,
    stalled_endpoint: Option<u8>,
    attempts_in_current_step: u32,
    max_attempts_per_step: u32,
    history: Vec<RecoveryStep>,
    diagnostic_log: Vec<String>,
}

impl Default for PipeRecoveryStateMachine {
    fn default() -> Self {
        Self::new(1)
    }
}

impl PipeRecoveryStateMachine {
    /// Creates a new recovery state machine with the specified maximum retries per step.
    pub fn new(max_attempts_per_step: u32) -> Self {
        Self {
            current_step: RecoveryStep::Idle,
            stalled_endpoint: None,
            attempts_in_current_step: 0,
            max_attempts_per_step: max_attempts_per_step.max(1),
            history: Vec::with_capacity(8),
            diagnostic_log: Vec::with_capacity(8),
        }
    }

    /// Initializes recovery for the given stalled endpoint address.
    pub fn start(&mut self, ep_addr: u8) {
        self.current_step = RecoveryStep::AbortPipe;
        self.stalled_endpoint = Some(ep_addr);
        self.attempts_in_current_step = 0;
        self.history.clear();
        self.diagnostic_log.clear();
        self.history.push(RecoveryStep::AbortPipe);
        self.diagnostic_log.push(format!(
            "Initiating 4-step recovery for stalled endpoint 0x{ep_addr:02x}"
        ));
    }

    /// Returns the current active recovery step.
    pub fn current_step(&self) -> RecoveryStep {
        self.current_step
    }

    /// Returns the stalled endpoint address if recovery is active.
    pub fn stalled_endpoint(&self) -> Option<u8> {
        self.stalled_endpoint
    }

    /// Returns the sequence of recovery steps executed so far.
    pub fn history(&self) -> &[RecoveryStep] {
        &self.history
    }

    /// Returns diagnostic messages logged during recovery transitions.
    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostic_log
    }

    /// Transitions to the next recovery step following an unsuccessful attempt.
    pub fn advance_step(&mut self) -> RecoveryStep {
        self.attempts_in_current_step += 1;
        if self.attempts_in_current_step < self.max_attempts_per_step {
            self.diagnostic_log.push(format!(
                "Retrying step {:?} (attempt {}/{})",
                self.current_step,
                self.attempts_in_current_step + 1,
                self.max_attempts_per_step
            ));
            return self.current_step;
        }

        self.attempts_in_current_step = 0;
        let next = match self.current_step {
            RecoveryStep::Idle => RecoveryStep::AbortPipe,
            RecoveryStep::AbortPipe => RecoveryStep::ClearPipeStallBothEnds,
            RecoveryStep::ClearPipeStallBothEnds => RecoveryStep::ProtocolReset,
            RecoveryStep::ProtocolReset => RecoveryStep::ResetDevice,
            RecoveryStep::ResetDevice => RecoveryStep::Failed,
            RecoveryStep::Recovered => RecoveryStep::Recovered,
            RecoveryStep::Failed => RecoveryStep::Failed,
        };

        self.current_step = next;
        self.history.push(next);
        self.diagnostic_log.push(format!("Escalating to recovery step: {next:?}"));
        next
    }

    /// Marks recovery as successfully completed.
    pub fn mark_recovered(&mut self) {
        self.current_step = RecoveryStep::Recovered;
        self.history.push(RecoveryStep::Recovered);
        self.diagnostic_log.push("Endpoint stall successfully recovered".to_string());
    }

    /// Marks recovery as permanently failed.
    pub fn mark_failed(&mut self, reason: &str) {
        self.current_step = RecoveryStep::Failed;
        self.history.push(RecoveryStep::Failed);
        self.diagnostic_log.push(format!("Recovery failed: {reason}"));
    }

    /// Resets the state machine back to quiescent `Idle`.
    pub fn reset(&mut self) {
        self.current_step = RecoveryStep::Idle;
        self.stalled_endpoint = None;
        self.attempts_in_current_step = 0;
        self.history.clear();
        self.diagnostic_log.clear();
    }

    /// Executes the full recovery workflow against the target until success or exhaustion.
    pub fn run_full_recovery<T: TransportRecoveryTarget>(
        &mut self,
        target: &mut T,
        ep_addr: u8,
    ) -> Result<(), DeviceError> {
        self.start(ep_addr);

        while self.current_step != RecoveryStep::Recovered && self.current_step != RecoveryStep::Failed {
            let ep = self.stalled_endpoint.unwrap_or(ep_addr);
            let step_result = match self.current_step {
                RecoveryStep::AbortPipe => target.abort_pipe(ep),
                RecoveryStep::ClearPipeStallBothEnds => target.clear_pipe_stall(ep),
                RecoveryStep::ProtocolReset => target.protocol_reset(),
                RecoveryStep::ResetDevice => target.reset_device(),
                RecoveryStep::Idle | RecoveryStep::Recovered | RecoveryStep::Failed => Ok(()),
            };

            match step_result {
                Ok(()) => {
                    self.mark_recovered();
                    return Ok(());
                }
                Err(e) => {
                    self.diagnostic_log.push(format!(
                        "Step {:?} failed with error: {e}",
                        self.current_step
                    ));
                    let next = self.advance_step();
                    if next == RecoveryStep::Failed {
                        self.diagnostic_log.push(format!(
                            "Pipe recovery exhausted all 4 stages on endpoint 0x{ep_addr:02x}. Last error: {e}"
                        ));
                        return Err(DeviceError::PipeStall(ep_addr));
                    }
                }
            }
        }

        if self.current_step == RecoveryStep::Recovered {
            Ok(())
        } else {
            Err(DeviceError::PipeStall(ep_addr))
        }
    }
}
