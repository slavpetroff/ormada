// Integration tests are allowed to use unwrap/expect for clarity
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]

//! Relations integration tests

mod common;

#[path = "relation_tests"]
mod relation_tests {
    use super::common;

    mod integration_relations;
    mod test_django_model_relations;
    mod test_minimal_relation;
    mod test_multi_relation_tuples;
    mod test_multi_relations;
    mod test_relations_advanced;
    mod test_simple_book;
    mod test_tuple_implementations;
}
