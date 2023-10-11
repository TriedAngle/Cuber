#![feature(c_size_t)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(unused)]

use glyphers;
use liverking::natty;
use std::{ffi, fs};

#[repr(C)]
pub struct RasterizationResult<T> {
    width: usize,
    height: usize,
    length: usize,
    pointer: *mut T
}

#[no_mangle]
pub extern "C" fn rasterize_norm(
    text: *const ffi::c_char,
    fonts: *const *const ffi::c_char,
    fonts_count: core::ffi::c_size_t,
) -> RasterizationResult<ffi::c_float> {
    let txt = natty!(ffi::CStr::from_ptr(text));
    let text = txt.to_str().unwrap();

    let fonts: Vec<String> = natty!(std::slice::from_raw_parts(fonts, fonts_count)
        .iter()
        .map(|&f| ffi::CStr::from_ptr(f).to_string_lossy().into_owned())
        .collect());

    let font_refs: Vec<&str> = fonts.iter().map(AsRef::as_ref).collect();

    let (rasterized, width, height) = glyphers::rasterize(text, &font_refs);
    let mut normalized: Vec<f32> = rasterized.iter().map(|&val| val as f32 / 255.0).collect();
    let length = normalized.len();
    let ptr = normalized.as_mut_ptr();
    std::mem::forget(normalized);
    return RasterizationResult {
        width,
        height,
        length,
        pointer: ptr,
    };
}

#[no_mangle]
pub extern "C" fn rasterize(
    text: *const ffi::c_char,
    fonts: *const *const ffi::c_char,
    fonts_count: core::ffi::c_size_t,
) -> RasterizationResult<ffi::c_uchar> {
    let txt = natty!(ffi::CStr::from_ptr(text));
    let text = txt.to_str().unwrap();

    let fonts: Vec<String> = natty!(std::slice::from_raw_parts(fonts, fonts_count)
        .iter()
        .map(|&f| ffi::CStr::from_ptr(f).to_string_lossy().into_owned())
        .collect());

    let font_refs: Vec<&str> = fonts.iter().map(AsRef::as_ref).collect();
    let (mut rasterized, width, height) = glyphers::rasterize(text, &font_refs);

    let length = rasterized.len();
    let ptr = rasterized.as_mut_ptr();
    std::mem::forget(rasterized);
    return RasterizationResult {
        width,
        height,
        length,
        pointer: ptr,
    }
}

#[no_mangle]
pub extern "C" fn deallocate_rasterization_norm(ptr: *mut ffi::c_char, size: core::ffi::c_size_t) {
    unsafe {
        Vec::from_raw_parts(ptr, size, size); // Vec will be deallocated when it goes out of scope
    }
}

#[no_mangle]
pub extern "C" fn deallocate_rasterization(ptr: *mut ffi::c_char, size: core::ffi::c_size_t) {
    unsafe {
        Vec::from_raw_parts(ptr, size, size); // Vec will be deallocated when it goes out of scope
    }
}
