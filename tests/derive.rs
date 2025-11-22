//! Derive macro integration tests

mod common;

#[path = "derive_tests"]
mod derive_tests {
    #[allow(unused_imports)]
    use super::common;

    mod test_coverage_boost;
}
