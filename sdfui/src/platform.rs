use crate::safe;
use std::{ffi, ptr};

extern "system" {
    fn VirtualAlloc(
        lpAddress: *mut ffi::c_void,
        dwSize: usize,
        flAllocationType: u32,
        flProtect: u32,
    ) -> *mut ffi::c_void;

    fn VirtualFree(lpAddress: *mut ffi::c_void, dwSize: usize, dwFreeType: u32) -> bool;
}

/// alloc function, to make my editor import the right function without thinking I named it this way
pub fn allocard(size: usize) -> *mut ffi::c_void {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    safe! {
        unimplemented!("oh no");
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
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
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    safe! {
        unimplemented!("oh no");
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
