//! raid the function pointers from subprimal systems
use crate::natty;
use std::ffi;

#[cfg(target_os = "windows")]
extern "system" {
    fn LoadLibraryA(lpFileName: *const ffi::c_char) -> *mut ffi::c_void;
    fn GetProcAddress(
        hModule: *mut ffi::c_void,
        lpProcName: *const ffi::c_char,
    ) -> *mut ffi::c_void;
    fn FreeLibrary(hModule: *mut ffi::c_void) -> ffi::c_int;
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
extern "C" {
    fn dlopen(filename: *const ffi::c_char, flag: ffi::c_int) -> *mut ffi::c_void;
    fn dlsym(handle: *mut ffi::c_void, symbol: *const ffi::c_char) -> *mut ffi::c_void;
    fn dlclose(handle: *mut ffi::c_void) -> ffi::c_int;
}

pub fn invade(path: *const ffi::c_char) -> *mut ffi::c_void {
    #[cfg(target_os = "windows")]
    return natty!(LoadLibraryA(path));
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    return natty!(dlopen(path, 1));
}

pub fn steal(place: *mut ffi::c_void, object: *const ffi::c_char) -> *mut ffi::c_void {
    #[cfg(target_os = "windows")]
    return natty!(GetProcAddress(place, object));
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    return natty!(dlsym(place, object));
}

pub fn leave(place: *mut ffi::c_void) -> ffi::c_int {
    #[cfg(target_os = "windows")]
    return natty!(FreeLibrary(place));
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    return natty!(dlclose(place));
}
