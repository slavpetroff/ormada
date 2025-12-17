//! Compile-fail tests for type-safe relation loading
//!
//! These tests verify that accessing relation fields on Model (without prefetch)
//! causes compile-time errors, ensuring type safety.

#[test]
fn compile_fail_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/access_relation_on_model.rs");
    t.compile_fail("tests/ui/access_nullable_relation_on_model.rs");
    t.compile_fail("tests/ui/access_relation_after_create.rs");
    t.compile_fail("tests/ui/access_relation_after_first.rs");
    t.compile_fail("tests/ui/access_relation_after_all.rs");
}
