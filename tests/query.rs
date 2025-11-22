//! Query integration tests
//!
//! This module contains all query-related integration tests for the Django-like ORM.

mod common;

// Include all query test modules
#[path = "query_tests"]
mod query_tests {
    use super::common;

    mod comprehensive_test_suite;
    mod integration_query;
    mod test_advanced_queryset;
    mod test_aggregate_edge_cases;
    mod test_aggregations;
    mod test_column_methods;
    mod test_concurrency;
    mod test_delete_performance;
    mod test_error_paths;
    mod test_get_method;
    mod test_get_or_create;
    mod test_iterator_methods;
    mod test_q_objects;
    mod test_query_combinations;
    mod test_values_methods;
}
