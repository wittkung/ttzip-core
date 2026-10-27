// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Cross-platform physical resident set size (RSS) memory budget guard and pressure monitor.
//!
//! Provides deterministic memory bounds checking for archive decompression, decoding DAGs,
//! and parallel batch pipelines to prevent out-of-memory kernel terminations (OOM kills).

use crate::status::TTZipStatus;

/// Default hard ceiling for physical resident memory (128 MiB).
pub const DEFAULT_MAX_RSS_BYTES: u64 = 128 * 1024 * 1024;

/// Default threshold percentage (80%) indicating high memory pressure.
pub const DEFAULT_PRESSURE_PERCENT: u64 = 80;

/// Maximum allowable dictionary size (1 GiB) to protect against header-spoofing attacks (Zip/7z bombs).
pub const MAX_VALID_DICT_SIZE: u64 = 1024 * 1024 * 1024;

/// Baseline runtime overhead for arena, thread stacks, and I/O ring buffers (32 MiB).
pub const BASELINE_RUNTIME_OVERHEAD: u64 = 32 * 1024 * 1024;

/// Error returned when the system physical memory cannot support even a single worker for the given dictionary size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("physical memory quota exceeded: minimum required {required_bytes} bytes exceeds available system memory {available_bytes} bytes")]
pub struct MemoryQuotaExceededError {
    /// Minimum required memory in bytes.
    pub required_bytes: u64,
    /// Available system physical memory in bytes.
    pub available_bytes: u64,
}

impl From<MemoryQuotaExceededError> for TTZipStatus {
    #[inline]
    fn from(_: MemoryQuotaExceededError) -> Self {
        TTZipStatus::ErrOutOfMemory
    }
}

/// Error returned when the process physical resident set size exceeds the allocated budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("physical memory budget exceeded: current RSS {current_bytes} bytes exceeds hard limit {limit_bytes} bytes")]
pub struct MemoryBudgetExceededError {
    /// Current physical resident set size (RSS) in bytes.
    pub current_bytes: u64,
    /// Maximum allowed physical memory budget in bytes.
    pub limit_bytes: u64,
}

impl From<MemoryBudgetExceededError> for TTZipStatus {
    #[inline]
    fn from(_: MemoryBudgetExceededError) -> Self {
        TTZipStatus::ErrOutOfMemory
    }
}

/// Dynamic memory budget guard that queries operating system kernel telemetry to monitor
/// real physical resident set size (RSS) and throttle or abort runaway pipelines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryBudgetGuard {
    max_rss_bytes: u64,
    pressure_threshold_bytes: u64,
}

impl MemoryBudgetGuard {
    /// Creates a new memory budget guard with the given maximum physical RSS byte limit.
    /// The pressure threshold is automatically set to 80% of the hard limit.
    #[inline]
    #[must_use]
    pub const fn new(max_rss_bytes: u64) -> Self {
        let pressure_threshold_bytes = (max_rss_bytes / 100).saturating_mul(DEFAULT_PRESSURE_PERCENT);
        Self {
            max_rss_bytes,
            pressure_threshold_bytes,
        }
    }

    /// Creates a memory budget guard dynamically computed for a codec decompression pipeline.
    ///
    /// Allocates `(dict_size * workers + BASELINE_RUNTIME_OVERHEAD) * 1.25` as the hard limit.
    #[inline]
    #[must_use]
    pub fn for_codec_pipeline(dict_size: u64, workers: usize) -> Self {
        let workers = workers.max(1) as u64;
        let base = dict_size.saturating_mul(workers).saturating_add(BASELINE_RUNTIME_OVERHEAD);
        let hard_limit = (base / 100).saturating_mul(125).max(DEFAULT_MAX_RSS_BYTES);
        Self::new(hard_limit)
    }

    /// Creates a memory budget guard initialized with the default 128 MiB limit.
    #[inline]
    #[must_use]
    pub const fn default_budget() -> Self {
        Self::new(DEFAULT_MAX_RSS_BYTES)
    }


    /// Creates a memory budget guard with explicit maximum RSS and pressure threshold values.
    #[inline]
    #[must_use]
    pub const fn with_threshold(max_rss_bytes: u64, pressure_threshold_bytes: u64) -> Self {
        Self {
            max_rss_bytes,
            pressure_threshold_bytes,
        }
    }

    /// Returns the maximum allowed resident set size (RSS) in bytes.
    #[inline]
    #[must_use]
    pub const fn max_rss_bytes(&self) -> u64 {
        self.max_rss_bytes
    }

    /// Returns the pressure threshold in bytes.
    #[inline]
    #[must_use]
    pub const fn pressure_threshold_bytes(&self) -> u64 {
        self.pressure_threshold_bytes
    }

    /// Queries the operating system for the current process physical resident set size (RSS) in bytes.
    /// Returns 0 if the resident set size could not be retrieved from the kernel.
    #[inline]
    #[must_use]
    pub fn current_rss(&self) -> u64 {
        query_current_rss().unwrap_or(0)
    }

    /// Validates the current physical RSS against the hard limit.
    ///
    /// # Errors
    /// Returns [`MemoryBudgetExceededError`] if current RSS exceeds `max_rss_bytes`.
    #[inline]
    pub fn check_budget(&self) -> Result<(), MemoryBudgetExceededError> {
        let current = self.current_rss();
        self.check_budget_with_rss(current)
    }

    /// Validates an explicitly supplied RSS measurement against the hard limit.
    ///
    /// # Errors
    /// Returns [`MemoryBudgetExceededError`] if `current_rss` exceeds `max_rss_bytes`.
    #[inline]
    pub fn check_budget_with_rss(&self, current_rss: u64) -> Result<(), MemoryBudgetExceededError> {
        if current_rss > self.max_rss_bytes {
            Err(MemoryBudgetExceededError {
                current_bytes: current_rss,
                limit_bytes: self.max_rss_bytes,
            })
        } else {
            Ok(())
        }
    }

    /// Returns `true` if the current physical memory consumption has reached or exceeded
    /// the pressure threshold (default 80%), indicating that caller pipelines should yield,
    /// page buffers to disk, or reduce worker concurrency.
    #[inline]
    #[must_use]
    pub fn is_under_pressure(&self) -> bool {
        let current = self.current_rss();
        self.is_under_pressure_with_rss(current)
    }

    /// Returns `true` if the supplied RSS measurement exceeds the pressure threshold.
    #[inline]
    #[must_use]
    pub const fn is_under_pressure_with_rss(&self, current_rss: u64) -> bool {
        current_rss >= self.pressure_threshold_bytes
    }

    /// Returns the current memory utilization ratio between `[0.0, ...]`.
    #[inline]
    #[must_use]
    pub fn usage_ratio(&self) -> f64 {
        if self.max_rss_bytes == 0 {
            return 1.0;
        }
        self.current_rss() as f64 / self.max_rss_bytes as f64
    }
}

impl Default for MemoryBudgetGuard {
    #[inline]
    fn default() -> Self {
        Self::default_budget()
    }
}

/// Dynamically calculates safe worker concurrency for dictionary-driven codecs (LZMA2 / Zstd).
///
/// Implements the adaptive concurrency throttling equation:
/// `ActualWorkers = max(1, min(max_cores, floor((AvailPhysRAM * 0.75) / DictSize)))`
///
/// # Errors
/// - Returns [`MemoryQuotaExceededError`] if `dict_size` exceeds [`MAX_VALID_DICT_SIZE`] (header spoofing / corrupt).
/// - Returns [`MemoryQuotaExceededError`] if minimum memory for 1 worker exceeds 90% of available RAM.
pub fn calculate_adaptive_workers(
    dict_size: u64,
    max_cores: usize,
    avail_phys_ram: u64,
) -> Result<usize, MemoryQuotaExceededError> {
    if dict_size > MAX_VALID_DICT_SIZE {
        return Err(MemoryQuotaExceededError {
            required_bytes: dict_size,
            available_bytes: avail_phys_ram,
        });
    }

    let single_worker_min = dict_size.saturating_add(BASELINE_RUNTIME_OVERHEAD);
    let usable_ram = (avail_phys_ram / 100).saturating_mul(90);
    if single_worker_min > usable_ram {
        return Err(MemoryQuotaExceededError {
            required_bytes: single_worker_min,
            available_bytes: avail_phys_ram,
        });
    }

    let budget_pool = (avail_phys_ram / 100).saturating_mul(75);
    let allowed_by_ram = budget_pool
        .checked_div(dict_size)
        .map_or(max_cores, |v| v as usize);
    let workers = allowed_by_ram.clamp(1, max_cores.max(1));
    Ok(workers)
}

/// Queries the operating system for current available physical host RAM in bytes.
/// Returns a conservative default (2 GiB) if telemetry is unavailable.
#[must_use]
pub fn query_available_physical_memory() -> u64 {
    #[cfg(target_vendor = "apple")]
    {
        if let Some(bytes) = get_macos_available_ram() {
            return bytes;
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(bytes) = get_linux_available_ram() {
            return bytes;
        }
    }

    #[cfg(windows)]
    {
        if let Some(bytes) = get_windows_available_ram() {
            return bytes;
        }
    }

    #[cfg(all(unix, not(target_vendor = "apple")))]
    {
        if let Some(bytes) = get_unix_sysconf_available_ram() {
            return bytes;
        }
    }


    // Conservative default fallback: 2 GiB
    2 * 1024 * 1024 * 1024
}


/// Queries the operating system for the current process physical resident set size (RSS) in bytes.
#[must_use]
pub fn query_current_rss() -> Option<u64> {
    #[cfg(target_vendor = "apple")]
    {
        if let Some(rss) = get_macos_rss() {
            return Some(rss);
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(rss) = get_linux_rss() {
            return Some(rss);
        }
    }

    #[cfg(windows)]
    {
        if let Some(rss) = get_windows_rss() {
            return Some(rss);
        }
    }

    #[cfg(unix)]
    {
        if let Some(rss) = get_rusage_rss() {
            return Some(rss);
        }
    }

    None
}

#[cfg(target_vendor = "apple")]
fn get_macos_rss() -> Option<u64> {
    extern "C" {
        fn mach_task_self() -> libc::mach_port_t;
    }

    unsafe {
        let mut info = std::mem::MaybeUninit::<libc::mach_task_basic_info>::uninit();
        let mut count = (std::mem::size_of::<libc::mach_task_basic_info>()
            / std::mem::size_of::<libc::natural_t>()) as libc::mach_msg_type_number_t;
        let kerr = libc::task_info(
            mach_task_self(),
            libc::MACH_TASK_BASIC_INFO,
            info.as_mut_ptr() as libc::task_info_t,
            &mut count,
        );
        if kerr == libc::KERN_SUCCESS {
            let info = info.assume_init();
            Some(info.resident_size)
        } else {
            None
        }
    }
}

#[cfg(target_os = "linux")]
fn get_linux_rss() -> Option<u64> {
    let content = std::fs::read_to_string("/proc/self/statm").ok()?;
    let mut parts = content.split_whitespace();
    let _total_pages = parts.next()?;
    let rss_pages: u64 = parts.next()?.parse().ok()?;
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    let page_size = if page_size > 0 { page_size as u64 } else { 4096 };
    Some(rss_pages.saturating_mul(page_size))
}

#[cfg(windows)]
fn get_windows_rss() -> Option<u64> {
    #[repr(C)]
    #[allow(non_snake_case)]
    struct ProcessMemoryCounters {
        cb: u32,
        PageFaultCount: u32,
        PeakWorkingSetSize: usize,
        WorkingSetSize: usize,
        QuotaPeakPagedPoolUsage: usize,
        QuotaPagedPoolUsage: usize,
        QuotaPeakNonPagedPoolUsage: usize,
        QuotaNonPagedPoolUsage: usize,
        PagefileUsage: usize,
        PeakPagefileUsage: usize,
    }

    extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn K32GetProcessMemoryInfo(
            process: *mut std::ffi::c_void,
            counters: *mut ProcessMemoryCounters,
            cb: u32,
        ) -> i32;
    }

    unsafe {
        let mut counters = std::mem::MaybeUninit::<ProcessMemoryCounters>::zeroed();
        let cb = std::mem::size_of::<ProcessMemoryCounters>() as u32;
        (*counters.as_mut_ptr()).cb = cb;
        let proc = GetCurrentProcess();
        if K32GetProcessMemoryInfo(proc, counters.as_mut_ptr(), cb) != 0 {
            let counters = counters.assume_init();
            Some(counters.WorkingSetSize as u64)
        } else {
            None
        }
    }
}

#[cfg(unix)]
fn get_rusage_rss() -> Option<u64> {
    unsafe {
        let mut rusage = std::mem::MaybeUninit::<libc::rusage>::uninit();
        if libc::getrusage(libc::RUSAGE_SELF, rusage.as_mut_ptr()) == 0 {
            let rusage = rusage.assume_init();
            #[cfg(target_vendor = "apple")]
            let bytes = rusage.ru_maxrss as u64;
            #[cfg(not(target_vendor = "apple"))]
            let bytes = (rusage.ru_maxrss as u64).saturating_mul(1024);
            Some(bytes)
        } else {
            None
        }
    }
}

#[cfg(target_vendor = "apple")]
fn get_macos_available_ram() -> Option<u64> {
    extern "C" {
        fn mach_host_self() -> libc::mach_port_t;
    }
    unsafe {
        let mut vm_stat = std::mem::MaybeUninit::<libc::vm_statistics64>::uninit();
        let mut count = (std::mem::size_of::<libc::vm_statistics64>()
            / std::mem::size_of::<libc::integer_t>()) as libc::mach_msg_type_number_t;
        let kerr = libc::host_statistics64(
            mach_host_self(),
            libc::HOST_VM_INFO64,
            vm_stat.as_mut_ptr() as libc::host_info64_t,
            &mut count,
        );
        if kerr == libc::KERN_SUCCESS {
            let vm_stat = vm_stat.assume_init();
            let page_size = libc::sysconf(libc::_SC_PAGESIZE) as u64;
            let free_pages = (vm_stat.free_count + vm_stat.inactive_count) as u64;
            Some(free_pages.saturating_mul(page_size))
        } else {
            None
        }
    }
}

#[cfg(target_os = "linux")]
fn get_linux_available_ram() -> Option<u64> {
    let content = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("MemAvailable:") {
            let kb: u64 = rest.split_whitespace().next()?.parse().ok()?;
            return Some(kb.saturating_mul(1024));
        }
    }
    None
}

#[cfg(windows)]
fn get_windows_available_ram() -> Option<u64> {
    #[repr(C)]
    struct MemoryStatusEx {
        dwLength: u32,
        dwMemoryLoad: u32,
        ullTotalPhys: u64,
        ullAvailPhys: u64,
        ullTotalPageFile: u64,
        ullAvailPageFile: u64,
        ullTotalVirtual: u64,
        ullAvailVirtual: u64,
        ullAvailExtendedVirtual: u64,
    }
    extern "system" {
        fn GlobalMemoryStatusEx(stat: *mut MemoryStatusEx) -> i32;
    }
    unsafe {
        let mut stat = std::mem::MaybeUninit::<MemoryStatusEx>::zeroed();
        (*stat.as_mut_ptr()).dwLength = std::mem::size_of::<MemoryStatusEx>() as u32;
        if GlobalMemoryStatusEx(stat.as_mut_ptr()) != 0 {
            let stat = stat.assume_init();
            Some(stat.ullAvailPhys)
        } else {
            None
        }
    }
}

#[cfg(all(unix, not(target_vendor = "apple")))]
fn get_unix_sysconf_available_ram() -> Option<u64> {
    unsafe {
        let pages = libc::sysconf(libc::_SC_PHYS_PAGES);
        let page_size = libc::sysconf(libc::_SC_PAGESIZE);
        if pages > 0 && page_size > 0 {
            Some(((pages as u64).saturating_mul(page_size as u64)) / 2)
        } else {
            None
        }
    }
}


// Compile-time static assertions ensuring memory budget types satisfy zero-cost concurrency invariants.
const _: () = {
    const fn assert_send_sync<T: Send + Sync + 'static>() {}
    const fn assert_unpin<T: Unpin>() {}

    assert_send_sync::<MemoryBudgetGuard>();
    assert_send_sync::<MemoryBudgetExceededError>();
    assert_send_sync::<MemoryQuotaExceededError>();
    assert_unpin::<MemoryBudgetGuard>();
    assert_unpin::<MemoryBudgetExceededError>();
    assert_unpin::<MemoryQuotaExceededError>();

    assert!(std::mem::size_of::<MemoryBudgetGuard>() == 16);
    assert!(std::mem::align_of::<MemoryBudgetGuard>() == 8);
};


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_budget_guard_defaults() {
        let guard = MemoryBudgetGuard::default();
        assert_eq!(guard.max_rss_bytes(), DEFAULT_MAX_RSS_BYTES);
        assert_eq!(guard.pressure_threshold_bytes(), (DEFAULT_MAX_RSS_BYTES / 100) * 80);
    }

    #[test]
    fn test_memory_budget_custom_threshold() {
        let guard = MemoryBudgetGuard::with_threshold(1000, 600);
        assert_eq!(guard.max_rss_bytes(), 1000);
        assert_eq!(guard.pressure_threshold_bytes(), 600);
    }

    #[test]
    fn test_budget_check_within_and_exceeded() {
        let guard = MemoryBudgetGuard::new(100 * 1024 * 1024); // 100MB
        // Under limit
        assert!(guard.check_budget_with_rss(50 * 1024 * 1024).is_ok());
        // Exactly at limit
        assert!(guard.check_budget_with_rss(100 * 1024 * 1024).is_ok());
        // Exceeded
        let err = guard.check_budget_with_rss(100 * 1024 * 1024 + 1).unwrap_err();
        assert_eq!(err.current_bytes, 100 * 1024 * 1024 + 1);
        assert_eq!(err.limit_bytes, 100 * 1024 * 1024);
    }

    #[test]
    fn test_pressure_detection() {
        let guard = MemoryBudgetGuard::new(1000);
        // Pressure threshold is 800
        assert!(!guard.is_under_pressure_with_rss(799));
        assert!(guard.is_under_pressure_with_rss(800));
        assert!(guard.is_under_pressure_with_rss(950));
    }

    #[test]
    fn test_live_os_query() {
        let rss = query_current_rss();
        assert!(rss.is_some(), "Kernel RSS telemetry must be queryable on this platform");
        let rss_val = rss.unwrap();
        assert!(rss_val > 0, "Resident memory of running test process must be > 0");

        let guard = MemoryBudgetGuard::new(u64::MAX);
        assert!(guard.check_budget().is_ok());
    }

    #[test]
    fn test_status_conversion() {
        let err = MemoryBudgetExceededError {
            current_bytes: 200,
            limit_bytes: 100,
        };
        let status: TTZipStatus = err.into();
        assert_eq!(status, TTZipStatus::ErrOutOfMemory);

        let quota_err = MemoryQuotaExceededError {
            required_bytes: 500,
            available_bytes: 400,
        };
        let status2: TTZipStatus = quota_err.into();
        assert_eq!(status2, TTZipStatus::ErrOutOfMemory);
    }

    #[test]
    fn test_calculate_adaptive_workers_concurrency_throttling() {
        // Available RAM: 1000 MiB. Dict: 256 MiB. Max Cores: 8.
        // 75% budget pool = 750 MiB.
        // allowed = 750 / 256 = 2 workers.
        let workers = calculate_adaptive_workers(
            256 * 1024 * 1024,
            8,
            1000 * 1024 * 1024,
        ).unwrap();
        assert_eq!(workers, 2);

        // Constrained RAM: 400 MiB. Dict: 256 MiB.
        // Single worker min: 256 + 32 = 288 MiB <= 360 MiB (90% of 400).
        // 75% budget pool = 300 MiB.
        // allowed = 300 / 256 = 1 worker.
        let workers_throttled = calculate_adaptive_workers(
            256 * 1024 * 1024,
            8,
            400 * 1024 * 1024,
        ).unwrap();
        assert_eq!(workers_throttled, 1);

        // Severe RAM exhaustion: 300 MiB. Dict: 256 MiB.
        // Single worker min = 288 MiB > 270 MiB (90% of 300).
        // Must reject with MemoryQuotaExceededError.
        let err = calculate_adaptive_workers(
            256 * 1024 * 1024,
            8,
            300 * 1024 * 1024,
        ).unwrap_err();
        assert_eq!(err.required_bytes, 288 * 1024 * 1024);
        assert_eq!(err.available_bytes, 300 * 1024 * 1024);

        // Header spoofing / Zip bomb defense: 2 GiB dict exceeds 1 GiB limit
        let spoof_err = calculate_adaptive_workers(
            2 * 1024 * 1024 * 1024,
            8,
            32 * 1024 * 1024 * 1024,
        ).unwrap_err();
        assert_eq!(spoof_err.required_bytes, 2 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_query_available_physical_memory() {
        let avail = query_available_physical_memory();
        assert!(avail > 0, "Available physical RAM must be > 0 bytes");
    }
}

