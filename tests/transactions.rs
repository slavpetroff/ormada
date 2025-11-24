// Integration tests are allowed to use unwrap/expect for clarity
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]

//! Transaction integration tests

mod common;

#[path = "transaction_tests"]
mod transaction_tests {
    #[allow(unused_imports)]
    use super::common;

    mod test_atomic_macro;
    mod test_savepoints;
    mod test_transactions;
}
