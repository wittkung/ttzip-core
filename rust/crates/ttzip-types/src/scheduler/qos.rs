// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Operating system platform-native Quality of Service (QoS) and thread priority bindings.
//!
//! Enforces high-performance hardware scheduling on worker threads across major platforms:
//! - macOS: Binds thread to Darwin `QOS_CLASS_USER_INITIATED` to align with Apple Silicon P-Cores.
//! - Windows: Sets thread priority to `THREAD_PRIORITY_HIGHEST` and disables EcoQoS power throttling.
//! - Linux: Sets thread priority to `PRIO_PROCESS` with nice level `-5`.

/// Applies platform-native compute QoS configuration to the calling thread.
///
/// This function must be invoked immediately after a worker thread starts, prior to executing
/// any compute-intensive tasks, guaranteeing priority execution on high-performance cores.
#[inline]
pub fn apply_thread_compute_qos() {
    #[cfg(target_os = "macos")]
    {
        // Darwin QoS: QOS_CLASS_USER_INITIATED directs OS scheduler to allocate P-Core capacity.
        unsafe {
            let _ = libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_USER_INITIATED, 0);
        }
    }

    #[cfg(target_os = "windows")]
    {
        type HANDLE = *mut std::ffi::c_void;
        type BOOL = i32;

        const THREAD_PRIORITY_HIGHEST: i32 = 2;
        const THREAD_POWER_THROTTLING: i32 = 1;
        const THREAD_POWER_THROTTLING_CURRENT_VERSION: u32 = 1;
        const THREAD_POWER_THROTTLING_EXECUTION_SPEED: u32 = 0x1;

        #[repr(C)]
        struct THREAD_POWER_THROTTLING_STATE {
            version: u32,
            control_mask: u32,
            state_mask: u32,
        }

        #[link(name = "kernel32")]
        extern "system" {
            fn GetCurrentThread() -> HANDLE;
            fn SetThreadPriority(h_thread: HANDLE, n_priority: i32) -> BOOL;
            fn SetThreadInformation(
                h_thread: HANDLE,
                thread_information_class: i32,
                thread_information: *const std::ffi::c_void,
                thread_information_size: u32,
            ) -> BOOL;
        }

        unsafe {
            let thread = GetCurrentThread();
            let _ = SetThreadPriority(thread, THREAD_PRIORITY_HIGHEST);

            let throttling_state = THREAD_POWER_THROTTLING_STATE {
                version: THREAD_POWER_THROTTLING_CURRENT_VERSION,
                control_mask: THREAD_POWER_THROTTLING_EXECUTION_SPEED,
                state_mask: 0, // 0 = Disable power throttling (High Performance P-Core scheduling)
            };
            let _ = SetThreadInformation(
                thread,
                THREAD_POWER_THROTTLING,
                &throttling_state as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<THREAD_POWER_THROTTLING_STATE>() as u32,
            );
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Renice thread towards higher scheduling priority (-5)
        // PRIO_PROCESS with who=0 targets the calling thread on Linux/NPTL.
        unsafe {
            let _ = libc::setpriority(libc::PRIO_PROCESS, 0, -5);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_thread_compute_qos_succeeds() {
        // Calling QoS configuration should not panic or corrupt thread state
        apply_thread_compute_qos();
    }
}
