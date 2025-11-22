//! Tests for types (OnDelete enum, etc.)

use seaorm_django::types::OnDelete;

#[test]
fn test_on_delete_to_sql() {
    assert_eq!(OnDelete::Cascade.to_sql(), "CASCADE");
    assert_eq!(OnDelete::SetNull.to_sql(), "SET NULL");
    assert_eq!(OnDelete::Restrict.to_sql(), "RESTRICT");
    assert_eq!(OnDelete::SetDefault.to_sql(), "SET DEFAULT");
    assert_eq!(OnDelete::NoAction.to_sql(), "NO ACTION");
}

#[test]
fn test_on_delete_display() {
    assert_eq!(format!("{}", OnDelete::Cascade), "CASCADE");
    assert_eq!(format!("{}", OnDelete::SetNull), "SET NULL");
    assert_eq!(format!("{}", OnDelete::Restrict), "RESTRICT");
    assert_eq!(format!("{}", OnDelete::SetDefault), "SET DEFAULT");
    assert_eq!(format!("{}", OnDelete::NoAction), "NO ACTION");
}

#[test]
fn test_on_delete_requires_nullable() {
    assert!(OnDelete::SetNull.requires_nullable());
    assert!(!OnDelete::Cascade.requires_nullable());
    assert!(!OnDelete::Restrict.requires_nullable());
    assert!(!OnDelete::SetDefault.requires_nullable());
    assert!(!OnDelete::NoAction.requires_nullable());
}

#[test]
fn test_on_delete_equality() {
    assert_eq!(OnDelete::Cascade, OnDelete::Cascade);
    assert_ne!(OnDelete::Cascade, OnDelete::SetNull);
    assert_ne!(OnDelete::SetNull, OnDelete::Restrict);
}

#[test]
fn test_on_delete_clone() {
    let on_delete = OnDelete::Cascade;
    let cloned = on_delete.clone();
    assert_eq!(on_delete, cloned);
}

#[test]
fn test_on_delete_copy() {
    let on_delete = OnDelete::Cascade;
    let copied = on_delete; // Copy, not move
    assert_eq!(on_delete, copied);
}

#[test]
fn test_on_delete_debug() {
    let on_delete = OnDelete::Cascade;
    let debug_str = format!("{:?}", on_delete);
    assert!(debug_str.contains("Cascade"));
}

#[test]
fn test_on_delete_hash() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(OnDelete::Cascade);
    set.insert(OnDelete::SetNull);
    set.insert(OnDelete::Cascade); // Duplicate

    assert_eq!(set.len(), 2); // Only 2 unique values
    assert!(set.contains(&OnDelete::Cascade));
    assert!(set.contains(&OnDelete::SetNull));
}

#[test]
fn test_all_on_delete_variants() {
    // Ensure all variants work
    let variants = vec![
        OnDelete::Cascade,
        OnDelete::SetNull,
        OnDelete::Restrict,
        OnDelete::SetDefault,
        OnDelete::NoAction,
    ];

    for variant in variants {
        assert!(!variant.to_sql().is_empty());
    }
}
