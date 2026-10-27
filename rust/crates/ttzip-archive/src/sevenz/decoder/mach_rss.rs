// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Darwin Mach kernel task_info FFI binding for physical RSS tracking.

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
struct MachTaskBasicInfo {
    pub virtual_size: u64,
    pub resident_size: u64,
    pub resident_size_max: u64,
    pub user_time_sec: i32,
    pub user_time_usec: i32,
    pub system_time_sec: i32,
    pub system_time_usec: i32,
    pub policy: i32,
    pub suspend_count: i32,
}

const MACH_TASK_BASIC_INFO: u32 = 20;
const MACH_TASK_BASIC_INFO_COUNT: u32 =
    (std::mem::size_of::<MachTaskBasicInfo>() / std::mem::size_of::<i32>()) as u32;

#[cfg(target_os = "macos")]
extern "C" {
    fn mach_task_self() -> u32;
    fn task_info(
        target_task: u32,
        flavor: u32,
        task_info_out: *mut MachTaskBasicInfo,
        task_info_outCnt: *mut u32,
    ) -> i32;
}

/// Retrieves current physical Resident Set Size (RSS) in bytes.
pub fn get_current_rss_bytes() -> u64 {
    #[cfg(target_os = "macos")]
    unsafe {
        let mut info: MachTaskBasicInfo = std::mem::zeroed();
        let mut count = MACH_TASK_BASIC_INFO_COUNT;
        let kret = task_info(mach_task_self(), MACH_TASK_BASIC_INFO, &mut info, &mut count);
        if kret == 0 && info.resident_size > 0 {
            return info.resident_size;
        }
    }

    #[cfg(unix)]
    unsafe {
        let mut usage: libc::rusage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_SELF, &mut usage);
        #[cfg(target_os = "macos")]
        {
            usage.ru_maxrss as u64
        }
        #[cfg(not(target_os = "macos"))]
        {
            (usage.ru_maxrss as u64) * 1024
        }
    }
    #[cfg(not(unix))]
    {
        0
    }
}
