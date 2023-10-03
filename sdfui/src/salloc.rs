//! (Very) Safe Cross-Platform Memory Allocation
use crate::safe;
use std::{ffi, ptr};

#[cfg(target_os = "windows")]
extern "system" {
    fn VirtualAlloc(
        lpAddress: *mut ffi::c_void,
        dwSize: usize,
        flAllocationType: u32,
        flProtect: u32,
    ) -> *mut ffi::c_void;

    fn VirtualFree(lpAddress: *mut ffi::c_void, dwSize: usize, dwFreeType: u32) -> bool;
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
extern "system" {
    fn mmap(
        addr: *mut std::ffi::c_void,
        length: usize,
        prot: i32,
        flags: i32,
        fd: i32,
        offset: i64,
    ) -> *mut std::ffi::c_void;

    fn munmap(addr: *mut std::ffi::c_void, length: usize) -> i32;
}

/// alloc function, to make my editor import the right function without thinking I named it this way
pub fn allocard(size: usize) -> *mut ffi::c_void {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    safe! {
        const PROT_READ: i32 = 0x1;
        const PROT_WRITE: i32 = 0x2;
        const MAP_PRIVATE: i32 = 0x02;
        const MAP_ANONYMOUS: i32 = 0x20;
        let ptr = mmap(
            std::ptr::null_mut(),
            size,
            PROT_READ | PROT_WRITE,
            MAP_PRIVATE | MAP_ANONYMOUS,
            -1,
            0,
        );
        return ptr;
    }
    #[cfg(target_os = "windows")]
    safe! {
        const MEM_COMMIT: u32 = 0x00001000;
        const MEM_RESERVE: u32 = 0x00002000;
        const PAGE_READWRITE: u32 = 0x04;
        let ptr = VirtualAlloc(
            ptr::null_mut(),
            size,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE
        );

        return ptr;
    }
}
/// free function, to make my editor import the right function without thinking I named it this way
pub fn freebie(address: *mut ffi::c_void, size: usize) {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    safe! {
        munmap(address, size);
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    safe! {
        const MEM_RELEASE: u32 =  0x00008000;
        let _ = VirtualFree(
            address,
            size,
            MEM_RELEASE
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alloc_free() {
        let size = 4096;
        let ptr = allocard(size); // this works without * but IntelliJ lint errors ok
        freebie(ptr, size)
    }
}
