//! Unit tests for error types and conversions
//!
//! Tests error handling and conversion traits following Rust best practices

use sea_orm::DbErr;
mod common;

use seaorm_django::error::DjangoOrmError;
use std::error::Error;

#[test]
fn test_error_from_sea_orm_db_err() {
    let db_err = DbErr::RecordNotFound("test".to_string());
    let django_err: DjangoOrmError = db_err.into();

    match django_err {
        DjangoOrmError::Database(_) => {}
        _ => panic!("Expected Database error variant"),
    }
}

#[test]
fn test_error_from_string() {
    let err: DjangoOrmError = "test error".to_string().into();

    match err {
        DjangoOrmError::Custom(msg) => assert_eq!(msg, "test error"),
        _ => panic!("Expected Custom error variant"),
    }
}

#[test]
fn test_error_from_str() {
    let err: DjangoOrmError = "test error".into();

    match err {
        DjangoOrmError::Custom(msg) => assert_eq!(msg, "test error"),
        _ => panic!("Expected Custom error variant"),
    }
}

#[test]
fn test_error_display() {
    let err = DjangoOrmError::Custom("test message".to_string());
    let display = format!("{}", err);
    assert_eq!(display, "test message");
}

#[test]
fn test_error_debug() {
    let err = DjangoOrmError::Custom("test message".to_string());
    let debug = format!("{:?}", err);
    assert!(debug.contains("Custom"));
    assert!(debug.contains("test message"));
}

#[test]
fn test_error_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<DjangoOrmError>();
}

#[test]
fn test_error_can_be_used_as_error_trait() {
    let django_err = DjangoOrmError::Custom("test".to_string());

    // Should implement Error trait
    let _: &dyn Error = &django_err;
}

#[test]
fn test_database_error_wrapping() {
    let db_err = DbErr::Custom("database error".to_string());
    let django_err = DjangoOrmError::Database(db_err);

    // Should display the error
    let display = format!("{}", django_err);
    assert!(display.len() > 0);
}

#[test]
fn test_custom_error_no_source() {
    let err = DjangoOrmError::Custom("test".to_string());
    assert!(err.source().is_none());
}

#[test]
fn test_error_conversion_preserves_message() {
    let original_msg = "specific error message";
    let err: DjangoOrmError = original_msg.to_string().into();
    let display = format!("{}", err);
    assert_eq!(display, original_msg);
}
