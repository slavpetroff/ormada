//! Write operations integration tests

mod common;

#[path = "write_tests"]
mod write_tests {
    #[allow(unused_imports)]
    use super::common;

    mod integration_write;
    mod test_bulk_operations;
    mod test_delete_ext;
    mod test_update_method;
    mod test_upsert_coverage;
}
