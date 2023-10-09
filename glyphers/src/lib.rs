#![feature(c_size_t)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(unused)]

use liverking::{natty, raid};
use std::{ffi, fs};

type GLint = i32;
type GLuint = u32;
type GLenum = u32;
type GLsizei = i32;

type PFN_wglGetProcAddress = extern "system" fn(*const ffi::c_char) -> *const ffi::c_void ;
type PFN_glCreateTextures = extern "system" fn(target: GLenum, n: GLsizei, textures: *mut GLuint);
#[no_mangle]
static mut glCreateTextures: PFN_glCreateTextures = {
    extern "system" fn dummy(_: GLenum, _: GLsizei, _: *mut GLuint) { panic!("lmfao"); } dummy };


#[no_mangle]
pub extern "C" fn load_opengl(path: *const ffi::c_char) -> ffi::c_int {
    natty!{
        let handle = raid::invade(path);
        if handle.is_null() { return -1; }
        
        let proc_name = std::ffi::CString::new("wglGetProcAddress").unwrap();
        let addr = raid::steal(handle, proc_name.as_ptr());
        if addr.is_null() { return -2; }
        let wglGetProcAddress: PFN_wglGetProcAddress = std::mem::transmute(addr);
        
        let gl_proc_name = std::ffi::CString::new("glCreateTextures").unwrap();
        let gl_proc = wglGetProcAddress(gl_proc_name.as_ptr() as *const i8);
        if gl_proc.is_null() { return -3; }
        glCreateTextures = std::mem::transmute(gl_proc);

        raid::leave(handle);
    }
    return 0;
}


#[no_mangle]
pub extern "C" fn string_to_texture(path: *const ffi::c_char, size: core::ffi::c_size_t) {
    let p =  natty!(ffi::CStr::from_ptr(path));
    let path = p.to_str().unwrap();
}

fn load_font_file(path: &str) -> Vec<u8> {
    if path.starts_with(".") || path.starts_with("/") {
        return fs::read(path).unwrap();
    } else {
        #[cfg(target_os = "windows")]
        let path = format!("C:/Windows/Fonts/{}", path);
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        let path = format!("/usr/share/fonts/truetype/{}", path);
        return fs::read(&path).unwrap();
    }
}


#[cfg(test)]
pub mod test {
    use super::*;

    #[test]
    fn test_fonts() {
        use fontdue::{Font, layout::{ Layout, CoordinateSystem, LayoutSettings, TextStyle } };
        let text = "meow 猫";
        let font_file = load_font_file("Arial.ttf");
        let font = Font::from_bytes(font_file, fontdue::FontSettings::default()).unwrap();
        let fonts = &[&font];

        let mut layout = Layout::new(CoordinateSystem::PositiveYUp);
        
        layout.append(fonts, &TextStyle::new(text, 42.0, 0));
        let glyphs = layout.glyphs();
        for glyph in glyphs {
            let (metrics, bitmap) = font.rasterize(glyph.parent, glyph.key.px);
            println!("Glyph: {:?}", bitmap);
        }
    }
}