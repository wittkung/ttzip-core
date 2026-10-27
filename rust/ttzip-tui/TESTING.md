# Copyright 2026 MetastudyLine. All rights reserved.
# Proprietary and confidential.

# TUI cancellation test contract

The generator/pre-start-cancellation test establishes cancellation before spawning the worker. A million dictionary entries cannot guarantee that a worker remains unfinished when a later cancellation is sent. A result already committed before cancellation must not be required to change retroactively.

The test retains the generator checks and uses one unchanged five-second observation budget. It requires the first observed event to be a cancelled terminal result, with no progress or later event, and observes sender disconnection within the remaining budget. No sleep or production recovery change is needed.

This test does not establish cancellation during computation, verification of recovered credentials, or worker join semantics. Those require separate controlled lifecycle coverage; the previous spawn-then-cancel race did not prove them.

Formal target: `bazel test -c opt //products/ttzip/core/rust/ttzip-tui:ttzip_tui_test`. Full integration remains subject to `ttaction run`.
