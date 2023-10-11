#![feature(c_size_t)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(unused)]

use fontdue::{
    layout::{CoordinateSystem, GlyphPosition, Layout, TextStyle},
    Font, FontSettings,
};
use liverking::{natty, raid};
use std::{ffi, fs};
use unicode_segmentation::UnicodeSegmentation;

// type GLint = i32;
// type GLuint = u32;
// type GLenum = u32;
// type GLsizei = i32;

// type PFN_wglGetProcAddress = extern "system" fn(*const ffi::c_char) -> *const ffi::c_void ;
// type PFN_glCreateTextures = extern "system" fn(target: GLenum, n: GLsizei, textures: *mut GLuint);
// #[no_mangle]
// static mut glCreateTextures: PFN_glCreateTextures = {
//     extern "system" fn dummy(_: GLenum, _: GLsizei, _: *mut GLuint) { panic!("lmfao"); } dummy };

// #[no_mangle]
// pub extern "C" fn load_opengl(path: *const ffi::c_char) -> ffi::c_int {
//     natty!{
//         let handle = raid::invade(path);
//         if handle.is_null() { return -1; }

//         let proc_name = std::ffi::CString::new("wglGetProcAddress").unwrap();
//         let addr = raid::steal(handle, proc_name.as_ptr());
//         if addr.is_null() { return -2; }
//         let wglGetProcAddress: PFN_wglGetProcAddress = std::mem::transmute(addr);

//         let gl_proc_name = std::ffi::CString::new("glCreateTextures").unwrap();
//         let gl_proc = wglGetProcAddress(gl_proc_name.as_ptr() as *const i8);
//         if gl_proc.is_null() { return -3; }
//         glCreateTextures = std::mem::transmute(gl_proc);

//         raid::leave(handle);
//     }
//     return 0;
// }

pub fn rasterize(text: &str, fonts: &[&str]) -> (Vec<u8>, usize, usize) {
    let fonts: Vec<Font> = load_fonts(fonts);
    let mut layout = Layout::new(CoordinateSystem::PositiveYUp);
    let size = 42.0;

    let mut start_byte_idx = 0;
    let mut current_font_idx = 0;

    for (end_byte_idx, grapheme) in text.grapheme_indices(true) {
        let start_char = grapheme.chars().next().unwrap();
        let (font_idx, _font) = fonts
            .iter()
            .enumerate()
            .find(|(_idx, font)| font.lookup_glyph_index(start_char) != 0)
            .unwrap_or((0, &fonts[0]));

        if font_idx != current_font_idx {
            layout.append(
                &fonts,
                &TextStyle::new(&text[start_byte_idx..end_byte_idx], size, current_font_idx),
            );
            start_byte_idx = end_byte_idx;
            current_font_idx = font_idx;
        }
    }
    if start_byte_idx < text.len() {
        layout.append(
            &fonts,
            &TextStyle::new(&text[start_byte_idx..], size, current_font_idx),
        );
    }

    let glyphs = layout.glyphs();
    let mut total_width = 0;
    let mut total_height = 0;

    for glyph in glyphs {
        let padding = glyph.x as usize - total_width;
        total_width += glyph.width;
        total_width += padding;
        if glyph.height > total_height {
            total_height = glyph.height
        };
    }
    let mut out = vec![0u8; total_width * total_height];
    for glyph in glyphs {
        let font = fonts
            .iter()
            .find(|font| font.lookup_glyph_index(glyph.parent) != 0)
            .unwrap_or(&fonts[0]);
        rasterize_glyph(&font, glyph, &mut out, total_width, total_height);
    }
    return (out, total_width, total_height);
}

fn rasterize_glyph(
    font: &Font,
    glyph: &GlyphPosition,
    out: &mut Vec<u8>,
    out_width: usize,
    _out_height: usize,
) {
    let (_metrics, bitmap) = font.rasterize(glyph.parent, glyph.key.px);
    let (width, _height) = (glyph.width, glyph.height);

    for sub_y in 0..glyph.height {
        for sub_x in 0..glyph.width {
            let image_index = sub_y * out_width + (glyph.x as usize + sub_x);
            let glyph_index = sub_y * width + sub_x;
            out[image_index] = bitmap[glyph_index];
        }
    }
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

fn load_fonts(paths: &[&str]) -> Vec<Font> {
    paths
        .iter()
        .map(|p| load_font_file(p))
        .map(|bytes| Font::from_bytes(bytes, FontSettings::default()).unwrap())
        .collect()
}

#[cfg(test)]
pub mod test {
    use image::{GrayImage, Luma};

    use super::*;

    #[test]
    fn test_fonts() {
        let text = "meowwy 猫🐱 XD";
        let (out, width, height) = rasterize(text, &["Arial.ttf", "msyh.ttc", "seguiemj.ttf"]);

        let mut img = GrayImage::new(width as u32, height as u32);
        for y in 0..height {
            for x in 0..width {
                let pixel_index = y * width + x;
                let pixel_value = out[pixel_index];
                img.put_pixel(x as u32, y as u32, Luma([pixel_value]));
            }
        }
        img.save("merge.png").unwrap();
    }
}
