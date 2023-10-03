#[macro_export]
macro_rules! safe {
    ($($body:tt)*) => {
        unsafe {
            $($body)*
        }
    };
}
