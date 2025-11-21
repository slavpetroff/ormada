//! Projection derive macro tests

mod common;

#[path = "projection_tests"]
mod projection_tests {
    #[allow(unused_imports)]
    use super::common;
    
    mod test_basic_compile;
}
