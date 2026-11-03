//! Repository acquisition interfaces. Implementations remain unavailable until later P04 tasks.
#![forbid(unsafe_code)]

// Sensitive values and their lengths must never appear in Debug output.
macro_rules! masked_debug {
    ($($name:ty),+ $(,)?) => {$ (
        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(concat!(stringify!($name), "(redacted)"))
            }
        }
    )+};
}

pub mod git;
pub mod lock;
pub mod remote;
