// Integration tests are allowed to use unwrap/expect for clarity
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]

//! Cache integration tests

mod common;

#[path = "cache_tests"]
mod cache_tests {
    #[allow(unused_imports)]
    use super::common;

    mod basic;
    mod internal;
    mod verification;
}
