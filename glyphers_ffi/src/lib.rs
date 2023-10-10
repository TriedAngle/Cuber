#![feature(c_size_t)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(unused)]

use std::{ffi, fs};
use glyphers;
use liverking::natty;

#[no_mangle]
pub extern "C" fn rasterize(
    text: *const ffi::c_char, 
    fonts: *const *const ffi::c_char,
    fonts_count: core::ffi::c_size_t, 
    width: *mut core::ffi::c_size_t,
    height: *mut core::ffi::c_size_t,
    length: *mut core::ffi::c_size_t,
) -> *mut ffi::c_uchar {
    let txt = natty!(ffi::CStr::from_ptr(text));
    let text = txt.to_str().unwrap();

    let fonts: Vec<String> = natty!(std::slice::from_raw_parts(fonts, fonts_count)
        .iter().map(|&f| ffi::CStr::from_ptr(f).to_string_lossy().into_owned()).collect());

    let font_refs: Vec<&str> = fonts.iter().map(AsRef::as_ref).collect();

    let (mut rasterized, rwidth, rheight) = glyphers::rasterize(text, &font_refs);
    
    natty! {
        *width = rwidth;
        *height = rheight;
        *length = rasterized.len();
    }
    let ptr = rasterized.as_mut_ptr();
    std::mem::forget(rasterized);
    return ptr;
}

#[no_mangle]
pub extern "C" fn deallocate_rasterization(ptr: *mut ffi::c_char, size: usize) {
    unsafe {
        Vec::from_raw_parts(ptr, size, size);  // Vec will be deallocated when it goes out of scope
    }
}