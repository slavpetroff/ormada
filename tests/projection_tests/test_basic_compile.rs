//! Basic compile-time tests for projection derive macro

use seaorm_django::prelude::*;

// Test that the macro compiles with valid syntax
mod user_projection {
    use super::*;
    
    #[django_model(table = "users")]
    pub struct User {
        #[primary_key]
        pub id: i32,
        pub name: String,
        pub email: String,
        pub age: i32,
    }
}

#[test]
fn test_model_compiles() {
    // Basic test that the model compiles
    let user = user_projection::Model {
        id: 1,
        name: "Alice".into(),
        email: "alice@example.com".into(),
        age: 30,
    };
    
    assert_eq!(user.id, 1);
    assert_eq!(user.name, "Alice");
}

#[test]
fn test_model_with_optional_fields() {
    #[django_model(table = "products")]
    struct Product {
        #[primary_key]
        id: i32,
        name: String,
        description: Option<String>,
    }
    
    let product = Model {
        id: 1,
        name: "Widget".into(),
        description: None,
    };
    
    assert_eq!(product.id, 1);
    assert!(product.description.is_none());
    
    let product2 = Model {
        id: 2,
        name: "Gadget".into(),
        description: Some("A useful gadget".into()),
    };
    
    assert!(product2.description.is_some());
}
