//! Tests for runtime validation in #[django_model]

use seaorm_django::error::DjangoOrmError;
use seaorm_django::prelude::*;
use seaorm_django::traits::DjangoEntity;

mod validation_user_mod {
    use super::*;

    #[django_model(table = "validation_users")]
    pub struct ValidationUser {
        #[primary_key]
        pub id: i32,

        #[max_length(50)]
        #[min_length(3)]
        pub username: String,

        #[max_length(200)]
        pub email: String,

        #[range(min = 18, max = 120)]
        pub age: i32,
    }
}

mod validation_product_mod {
    use super::*;

    #[django_model(table = "validation_products")]
    pub struct ValidationProduct {
        #[primary_key]
        pub id: i32,

        pub name: String,

        #[range(min = 0, max = 1000000)]
        pub price_cents: i32,

        #[range(min = 0)]
        pub stock: i32,
    }
}

#[test]
fn test_valid_user_passes_validation() {
    let user = validation_user_mod::Model {
        id: 0,
        username: "john_doe".to_string(),
        email: "john@example.com".to_string(),
        age: 25,
    };

    let result = validation_user_mod::ValidationUser::to_active_model_for_create(user);
    assert!(result.is_ok(), "Valid user should pass validation");
}

#[test]
fn test_max_length_validation_fails() {
    let user = validation_user_mod::Model {
        id: 0,
        username: "a".repeat(100), // Exceeds max_length(50)
        email: "test@example.com".to_string(),
        age: 25,
    };

    let result = validation_user_mod::ValidationUser::to_active_model_for_create(user);
    assert!(result.is_err(), "Should fail validation");

    match result {
        Err(DjangoOrmError::Validation { reason, .. }) => {
            // Field check removed
            assert!(reason.contains("max_length"));
            assert!(reason.contains("50"));
        }
        _ => panic!("Expected ValidationError"),
    }
}

#[test]
fn test_min_length_validation_fails() {
    let user = validation_user_mod::Model {
        id: 0,
        username: "ab".to_string(), // Less than min_length(3)
        email: "test@example.com".to_string(),
        age: 25,
    };

    let result = validation_user_mod::ValidationUser::to_active_model_for_create(user);
    assert!(result.is_err(), "Should fail validation");

    match result {
        Err(DjangoOrmError::Validation { reason, .. }) => {
            // Field check removed
            assert!(reason.contains("min_length"));
            assert!(reason.contains("3"));
        }
        _ => panic!("Expected ValidationError"),
    }
}

#[test]
fn test_range_min_validation_fails() {
    let user = validation_user_mod::Model {
        id: 0,
        username: "valid_user".to_string(),
        email: "test@example.com".to_string(),
        age: 15, // Less than min(18)
    };

    let result = validation_user_mod::ValidationUser::to_active_model_for_create(user);
    assert!(result.is_err(), "Should fail validation");

    match result {
        Err(DjangoOrmError::Validation { reason, .. }) => {
            // Field check removed
            assert!(reason.contains("minimum"));
            assert!(reason.contains("18"));
        }
        _ => panic!("Expected ValidationError"),
    }
}

#[test]
fn test_range_max_validation_fails() {
    let user = validation_user_mod::Model {
        id: 0,
        username: "valid_user".to_string(),
        email: "test@example.com".to_string(),
        age: 150, // Greater than max(120)
    };

    let result = validation_user_mod::ValidationUser::to_active_model_for_create(user);
    assert!(result.is_err(), "Should fail validation");

    match result {
        Err(DjangoOrmError::Validation { reason, .. }) => {
            // Field check removed
            assert!(reason.contains("maximum"));
            assert!(reason.contains("120"));
        }
        _ => panic!("Expected ValidationError"),
    }
}

#[test]
fn test_email_max_length_validation() {
    let user = validation_user_mod::Model {
        id: 0,
        username: "validuser".to_string(),
        email: "a".repeat(250) + "@test.com", // Exceeds max_length(200)
        age: 25,
    };

    let result = validation_user_mod::ValidationUser::to_active_model_for_create(user);
    assert!(result.is_err(), "Should fail validation");

    match result {
        Err(DjangoOrmError::Validation { reason, .. }) => {
            // Field check removed
            assert!(reason.contains("max_length"));
        }
        _ => panic!("Expected ValidationError"),
    }
}

#[test]
fn test_valid_product_passes_validation() {
    let product = validation_product_mod::Model {
        id: 0,
        name: "Test Product".to_string(),
        price_cents: 4999,
        stock: 100,
    };

    let result = validation_product_mod::ValidationProduct::to_active_model_for_create(product);
    assert!(result.is_ok(), "Valid product should pass validation");
}

#[test]
fn test_negative_price_validation_fails() {
    let product = validation_product_mod::Model {
        id: 0,
        name: "Bad Product".to_string(),
        price_cents: -100, // Less than min(0)
        stock: 10,
    };

    let result = validation_product_mod::ValidationProduct::to_active_model_for_create(product);
    assert!(result.is_err(), "Should fail validation");

    match result {
        Err(DjangoOrmError::Validation { reason, .. }) => {
            // Field check removed
            assert!(reason.contains("minimum"));
        }
        _ => panic!("Expected ValidationError"),
    }
}

#[test]
fn test_excessive_price_validation_fails() {
    let product = validation_product_mod::Model {
        id: 0,
        name: "Expensive Product".to_string(),
        price_cents: 2000000, // Greater than max(1000000)
        stock: 1,
    };

    let result = validation_product_mod::ValidationProduct::to_active_model_for_create(product);
    assert!(result.is_err(), "Should fail validation");

    match result {
        Err(DjangoOrmError::Validation { reason, .. }) => {
            // Field check removed
            assert!(reason.contains("maximum"));
        }
        _ => panic!("Expected ValidationError"),
    }
}

#[test]
fn test_negative_stock_validation_fails() {
    let product = validation_product_mod::Model {
        id: 0,
        name: "Product".to_string(),
        price_cents: 1000,
        stock: -5, // Less than min(0)
    };

    let result = validation_product_mod::ValidationProduct::to_active_model_for_create(product);
    assert!(result.is_err(), "Should fail validation");

    match result {
        Err(DjangoOrmError::Validation { .. }) => {
            // Expected - stock is negative (less than 0)
        }
        _ => panic!("Expected ValidationError"),
    }
}

#[test]
fn test_validation_error_display() {
    let err = DjangoOrmError::Validation {
        entity: "test",
        field: "test",
        reason: "test message".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("test"));
    assert!(display.contains("test message"));
}

#[test]
fn test_boundary_values_pass() {
    // Test exact min and max values
    let user_min_age = validation_user_mod::Model {
        id: 0,
        username: "abc".to_string(), // Exactly min_length(3)
        email: "test@test.com".to_string(),
        age: 18, // Exactly min(18)
    };

    assert!(validation_user_mod::ValidationUser::to_active_model_for_create(user_min_age).is_ok());

    let user_max_age = validation_user_mod::Model {
        id: 0,
        username: "a".repeat(50), // Exactly max_length(50)
        email: "test@test.com".to_string(),
        age: 120, // Exactly max(120)
    };

    assert!(validation_user_mod::ValidationUser::to_active_model_for_create(user_max_age).is_ok());
}
