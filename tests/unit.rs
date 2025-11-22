//! Unit tests

mod common;

#[path = "unit_tests"]
mod unit_tests {
    #[allow(unused_imports)]
    use super::common;

    mod test_aggregations_coverage;
    mod unit_error;
}
