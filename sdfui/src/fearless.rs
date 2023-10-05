//! introducing fearless safety to rust, the `safe!` macro and a thread-safe Box type

use std::{ffi, ptr};
#[macro_export]
macro_rules! safe {
    ($($body:tt)*) => {
        unsafe {
            $($body)*
        }
    };
}

// taking the red pill
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Boxy(*const ffi::c_void);
impl Boxy {
    pub fn null() -> Self {
        return Self(ptr::null())
    }

    pub fn safe<T>(&self) -> &T {
        return safe!(&*(self.0 as *const T));
    }
}

impl<T> From<&T> for Boxy {
    fn from(lmao: &T) -> Self {
        return Self(lmao as *const T as *const ffi::c_void);
    }
}

unsafe impl Send for Boxy {}
unsafe impl Sync for Boxy {}