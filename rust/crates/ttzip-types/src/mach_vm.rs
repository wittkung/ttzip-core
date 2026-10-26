// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! 16KB-aligned Mach VM memory allocator and zero-copy COW memory entry generator.
//!
//! # Apple Silicon 16KB Page Invariants
//! - All allocations are rounded up to the hardware VM page boundary (`16384` bytes).
//! - Direct kernel page allocation via `mach_vm_allocate` eliminates userspace malloc fragmentation.
//! - `make_memory_entry` generates a Mach port handle (`mach_port_t`) via `mach_make_memory_entry_64`,
//!   enabling zero-copy, copy-on-write (COW) memory mapping across XPC/Mach boundaries.

use core::fmt;
use core::ptr::NonNull;

pub const MACH_VM_PAGE_SIZE: usize = 16384; // 16KB Apple Silicon page size

/// Round an arbitrary byte size up to the next 16KB boundary.
#[inline(always)]
pub const fn round_up_to_16kb_page(size: usize) -> usize {
    (size + (MACH_VM_PAGE_SIZE - 1)) & !(MACH_VM_PAGE_SIZE - 1)
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum MachVmError {
    AllocationFailed { kern_return: i32, requested_size: usize },
    DeallocationFailed { kern_return: i32, address: u64, size: usize },
    MemoryEntryFailed { kern_return: i32 },
    InvalidSize,
}

impl fmt::Display for MachVmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllocationFailed { kern_return, requested_size } => {
                write!(f, "mach_vm_allocate failed (kern_return={}) for size {}", kern_return, requested_size)
            }
            Self::DeallocationFailed { kern_return, address, size } => {
                write!(f, "mach_vm_deallocate failed (kern_return={}) for addr 0x{:x}, size {}", kern_return, address, size)
            }
            Self::MemoryEntryFailed { kern_return } => {
                write!(f, "mach_make_memory_entry_64 failed (kern_return={})", kern_return)
            }
            Self::InvalidSize => write!(f, "requested Mach VM size must be greater than 0"),
        }
    }
}

impl std::error::Error for MachVmError {}

#[cfg(target_os = "macos")]
#[allow(dead_code)]
mod sys {
    pub type MachPort = u32;
    pub type VmMap = MachPort;
    pub type MachVmAddress = u64;
    pub type MachVmSize = u64;
    pub type MemoryObjectSize = u64;
    pub type MemoryObjectOffset = u64;
    pub type VmProt = i32;
    pub type KernReturn = i32;

    pub const KERN_SUCCESS: KernReturn = 0;
    pub const VM_FLAGS_ANYWHERE: i32 = 0x00000001;

    pub const VM_PROT_NONE: VmProt = 0x00;
    pub const VM_PROT_READ: VmProt = 0x01;
    pub const VM_PROT_WRITE: VmProt = 0x02;
    pub const VM_PROT_DEFAULT: VmProt = VM_PROT_READ | VM_PROT_WRITE;
    pub const MAP_MEM_VM_SHARE: VmProt = 0x00040000;
    pub const MAP_MEM_VM_COPY: VmProt = 0x00020000;

    extern "C" {
        pub fn mach_task_self() -> MachPort;

        pub fn mach_vm_allocate(
            target: VmMap,
            address: *mut MachVmAddress,
            size: MachVmSize,
            flags: i32,
        ) -> KernReturn;

        pub fn mach_vm_deallocate(
            target: VmMap,
            address: MachVmAddress,
            size: MachVmSize,
        ) -> KernReturn;

        pub fn mach_make_memory_entry_64(
            target_task: VmMap,
            size: *mut MemoryObjectSize,
            offset: MemoryObjectOffset,
            permission: VmProt,
            object_handle: *mut MachPort,
            parent_entry: MachPort,
        ) -> KernReturn;

        pub fn mach_port_deallocate(
            task: MachPort,
            name: MachPort,
        ) -> KernReturn;
    }
}

/// RAII Guard for a Mach port (`mach_port_t`).
pub struct MachPortGuard {
    port: u32,
}

impl MachPortGuard {
    #[inline]
    pub const fn new(port: u32) -> Self {
        Self { port }
    }

    #[inline]
    pub const fn raw_port(&self) -> u32 {
        self.port
    }
}

impl Drop for MachPortGuard {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        if self.port != 0 {
            unsafe {
                sys::mach_port_deallocate(sys::mach_task_self(), self.port);
            }
        }
    }
}

/// 16KB-aligned Mach VM memory allocation.
pub struct MachVmAllocation {
    ptr: NonNull<u8>,
    page_aligned_size: usize,
}

// Safety: Mach VM allocations are owned by the process and can be transferred across threads.
unsafe impl Send for MachVmAllocation {}
unsafe impl Sync for MachVmAllocation {}

impl MachVmAllocation {
    /// Allocate a virtual memory region rounded up to the nearest 16KB page.
    pub fn allocate(size: usize) -> Result<Self, MachVmError> {
        if size == 0 {
            return Err(MachVmError::InvalidSize);
        }

        let page_aligned_size = round_up_to_16kb_page(size);

        #[cfg(target_os = "macos")]
        {
            let mut address: sys::MachVmAddress = 0;
            let ret = unsafe {
                sys::mach_vm_allocate(
                    sys::mach_task_self(),
                    &mut address,
                    page_aligned_size as sys::MachVmSize,
                    sys::VM_FLAGS_ANYWHERE,
                )
            };

            if ret != sys::KERN_SUCCESS {
                return Err(MachVmError::AllocationFailed {
                    kern_return: ret,
                    requested_size: page_aligned_size,
                });
            }

            let ptr = NonNull::new(address as *mut u8)
                .expect("mach_vm_allocate returned KERN_SUCCESS but address was NULL");

            Ok(Self {
                ptr,
                page_aligned_size,
            })
        }

        #[cfg(not(target_os = "macos"))]
        {
            // Portable fallback for non-Darwin environments
            let layout = std::alloc::Layout::from_size_align(page_aligned_size, MACH_VM_PAGE_SIZE)
                .map_err(|_| MachVmError::InvalidSize)?;
            let raw = unsafe { std::alloc::alloc_zeroed(layout) };
            let ptr = NonNull::new(raw).ok_or(MachVmError::AllocationFailed {
                kern_return: -1,
                requested_size: page_aligned_size,
            })?;
            Ok(Self {
                ptr,
                page_aligned_size,
            })
        }
    }

    #[inline]
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr.as_ptr()
    }

    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr.as_ptr()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.page_aligned_size
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.page_aligned_size == 0
    }

    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.ptr.as_ptr(), self.page_aligned_size) }
    }

    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.page_aligned_size) }
    }

    /// Create a Mach memory entry port for this allocation for zero-copy IPC sharing.
    #[cfg(target_os = "macos")]
    pub fn make_memory_entry(&self, read_only: bool) -> Result<MachPortGuard, MachVmError> {
        let mut handle: sys::MachPort = 0;
        let mut size: sys::MemoryObjectSize = self.page_aligned_size as sys::MemoryObjectSize;
        let permission = if read_only {
            sys::VM_PROT_READ | sys::MAP_MEM_VM_COPY
        } else {
            sys::VM_PROT_DEFAULT | sys::MAP_MEM_VM_COPY
        };

        let ret = unsafe {
            sys::mach_make_memory_entry_64(
                sys::mach_task_self(),
                &mut size,
                self.ptr.as_ptr() as sys::MemoryObjectOffset,
                permission,
                &mut handle,
                0,
            )
        };

        if ret != sys::KERN_SUCCESS {
            return Err(MachVmError::MemoryEntryFailed { kern_return: ret });
        }

        Ok(MachPortGuard::new(handle))
    }
}

impl Drop for MachVmAllocation {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        {
            let ret = unsafe {
                sys::mach_vm_deallocate(
                    sys::mach_task_self(),
                    self.ptr.as_ptr() as sys::MachVmAddress,
                    self.page_aligned_size as sys::MachVmSize,
                )
            };
            if ret != sys::KERN_SUCCESS {
                // In drop, avoid panicking; log error if required
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            if let Ok(layout) = std::alloc::Layout::from_size_align(self.page_aligned_size, MACH_VM_PAGE_SIZE) {
                unsafe {
                    std::alloc::dealloc(self.ptr.as_ptr(), layout);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_up_16kb_page() {
        assert_eq!(round_up_to_16kb_page(1), 16384);
        assert_eq!(round_up_to_16kb_page(16384), 16384);
        assert_eq!(round_up_to_16kb_page(16385), 32768);
        assert_eq!(round_up_to_16kb_page(32768), 32768);
    }

    #[test]
    fn test_mach_vm_allocation_and_read_write() {
        let mut alloc = MachVmAllocation::allocate(100).expect("failed to allocate Mach VM page");
        assert_eq!(alloc.len(), 16384);
        assert_eq!(alloc.as_ptr() as usize % 16384, 0, "must be 16KB page aligned");

        // Write and read
        alloc.as_mut_slice()[0] = 0xAA;
        alloc.as_mut_slice()[16383] = 0xBB;

        assert_eq!(alloc.as_slice()[0], 0xAA);
        assert_eq!(alloc.as_slice()[16383], 0xBB);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_mach_make_memory_entry() {
        let alloc = MachVmAllocation::allocate(16384).expect("allocate failed");
        let port_guard = alloc.make_memory_entry(true).expect("make memory entry failed");
        assert!(port_guard.raw_port() != 0, "returned valid Mach port handle");
    }
}
