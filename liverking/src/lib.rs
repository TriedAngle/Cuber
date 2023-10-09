//! This library is not just a library. This is a way of life.
//!
//! "Memory Safety" created weak men.
//! 
//! You can find the tenants of the ancestral lifestyle here.

pub mod hunt;
pub mod juice;
pub mod liver;
pub mod raid;

/// rust will often accuse you and will try to check your natty status
/// use this to protect yourself from those accusations
#[macro_export]
macro_rules! natty {
    ($($body:tt)*) => {
        unsafe {
            $($body)*
        }
    };
}


/// we don't do test driven development in the wilds
#[cfg(test)]
mod tests {}
