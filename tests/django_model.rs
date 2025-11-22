//! Django model macro integration tests

mod common;

#[path = "django_model_tests"]
mod django_model_tests {
    #[allow(unused_imports)]
    use super::common;

    mod test_macro_generation;
    mod test_types;
    mod test_validation;
}
