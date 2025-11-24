// Integration tests are allowed to use unwrap/expect for clarity
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]

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
