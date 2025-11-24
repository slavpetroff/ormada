// Integration tests are allowed to use unwrap/expect for clarity
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]

//! Unit tests

mod common;

#[path = "unit_tests"]
mod unit_tests {
    #[allow(unused_imports)]
    use super::common;

    mod test_aggregations_coverage;
    mod unit_error;
}
