//! Projection derive macro tests
//!
//! Tests the #[django_projection] macro and .project::<T>() API

mod common;

#[path = "projection_tests"]
mod projection_tests {
    use super::common;
    
    mod test_macro_behavior;
    mod test_projection_usage;
    mod test_projection_aggregations;
    mod test_projection_validations;
}
