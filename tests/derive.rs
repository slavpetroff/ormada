// Integration tests are allowed to use unwrap/expect for clarity
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]

//! Derive macro integration tests

mod common;

#[path = "derive_tests"]
mod derive_tests {
    #[allow(unused_imports)]
    use super::common;

    mod test_coverage_boost;
}
