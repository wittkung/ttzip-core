// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Local work-stealing thread pool and hardware-conscious compute scheduler.

pub mod qos;
pub mod work_stealing;

pub use qos::apply_thread_compute_qos;
pub use work_stealing::{LocalWorkStealingPool, Scope};
