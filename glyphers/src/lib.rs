use liverking::{natty, raid};
use std::ffi;

type GLint = i32;
type GLuint = u32;
type GLenum = u32;
type GLsizei = i32;

type PFN_glCreateTextures = extern "system" fn(target: GLenum, n: GLsizei, textures: *mut GLuint);

static mut glCreateTextures: PFN_glCreateTextures = {
    extern "system" fn dummy(_: GLenum, _: GLsizei, _: *mut GLuint) { panic!("roflmao"); } dummy };


#[no_mangle]
pub extern "C" fn load_opengl_function_pointers() -> ffi::c_int {
    natty!{
        let lib = ffi::CString::new("opengl32.dll").unwrap();
        let handle = raid::invade(lib.as_ptr());
        if handle.is_null() { return -1; }
        
        let proc_name = ffi::CString::new("wglGetProcAddress").unwrap();
        let addr = raid::steal(handle, proc_name.as_ptr());
        if addr.is_null() { return -2; }
        let wglGetProcAddress: unsafe extern "system" fn(*const ffi::c_char) -> *const () = std::mem::transmute(addr);
        
        let gl_proc_name = ffi::CString::new("glCreateTextures").unwrap();
        let gl_proc = wglGetProcAddress(gl_proc_name.as_ptr() as *const i8);
        if gl_proc.is_null() { return -3; }
        glCreateTextures = std::mem::transmute(gl_proc);

        raid::leave(handle);
    }
    return 0;
}
