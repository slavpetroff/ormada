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
