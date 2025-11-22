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
