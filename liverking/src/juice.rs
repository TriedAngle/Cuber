//! the core of the ancestral lifestyle
use crate::natty;
use std::{ffi, ptr};

/// Core of the ancestral lifestyle
/// if something needs to be hidden,
/// enjoy this smol juicer.
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Juice(*const ffi::c_void);
impl Juice {
    pub fn none() -> Self {
        return Self(ptr::null());
    }

    pub fn inject<T>(&self) -> &T {
        return natty!(&*(self.0 as *const T));
    }
}

impl<T> From<&T> for Juice {
    fn from(inject: &T) -> Self {
        return Self(inject as *const T as *const ffi::c_void);
    }
}

unsafe impl Send for Juice {}
unsafe impl Sync for Juice {}
