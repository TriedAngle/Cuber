//! introducing fearless safety to rust, the `safe!` macro

#[macro_export]
macro_rules! safe {
    ($($body:tt)*) => {
        unsafe {
            $($body)*
        }
    };
}
