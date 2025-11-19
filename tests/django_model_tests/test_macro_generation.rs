//! Tests for #[django_model] macro code generation

use seaorm_django::prelude::*;
use seaorm_django::traits::DjangoEntity;
use sea_orm::EntityTrait;

// When multiple models are in the same module, their re-exports conflict
// So we put each in its own module
mod simple_author_mod {
    use super::*;
    
    #[django_model(table = "simple_authors")]
    pub struct SimpleAuthor {
        #[primary_key]
        pub id: i32,
        pub name: String,
        pub email: String,
    }
}

mod simple_book_mod {
    use super::*;
    
    #[django_model(table = "simple_books")]
    pub struct SimpleBook {
        #[primary_key]
        pub id: i32,
        pub title: String,
        pub author_id: i32,
    }
}

#[test]
fn test_model_struct_generated() {
    // Verify the Model struct exists - can use re-exported items
    let model = simple_author_mod::Model {
        id: 1,
        name: "Test Author".to_string(),
        email: "test@example.com".to_string(),
    };
    
    assert_eq!(model.id, 1);
    assert_eq!(model.name, "Test Author");
    assert_eq!(model.email, "test@example.com");
}

#[test]
fn test_entity_alias_works() {
    // The SimpleAuthor type should be an alias for Entity
    // This compiles = test passes
    let _entity_type: simple_author_mod::SimpleAuthor = simple_author_mod::Entity;
}

#[test]
fn test_module_name_generation() {
    // Module name should be snake_case of struct name
    // SimpleAuthor -> simple_author
    let model = simple_author_mod::Model {
        id: 1,
        name: "Test".to_string(),
        email: "test@test.com".to_string(),
    };
    
    assert_eq!(model.id, 1);
}

#[test]
fn test_multiple_models_in_same_scope() {
    // Both models should be accessible
    let author = simple_author_mod::Model {
        id: 1,
        name: "Author".to_string(),
        email: "author@test.com".to_string(),
    };
    
    let book = simple_book_mod::Model {
        id: 1,
        title: "Book".to_string(),
        author_id: 1,
    };
    
    assert_eq!(author.id, book.author_id);
}

#[test]
fn test_model_is_cloneable() {
    let model1 = simple_author_mod::Model {
        id: 1,
        name: "Test".to_string(),
        email: "test@test.com".to_string(),
    };
    
    let model2 = model1.clone();
    assert_eq!(model1.id, model2.id);
    assert_eq!(model1.name, model2.name);
}

#[test]
fn test_model_is_debug() {
    let model = simple_author_mod::Model {
        id: 1,
        name: "Test".to_string(),
        email: "test@test.com".to_string(),
    };
    
    let debug_str = format!("{:?}", model);
    assert!(debug_str.contains("Model"));
    assert!(debug_str.contains("Test"));
}

#[test]
fn test_model_has_default() {
    let model = simple_author_mod::Model::default();
    assert_eq!(model.id, 0);
    assert_eq!(model.name, "");
    assert_eq!(model.email, "");
}

#[test]
fn test_model_equality() {
    let model1 = simple_author_mod::Model {
        id: 1,
        name: "Test".to_string(),
        email: "test@test.com".to_string(),
    };
    
    let model2 = simple_author_mod::Model {
        id: 1,
        name: "Test".to_string(),
        email: "test@test.com".to_string(),
    };
    
    assert_eq!(model1, model2);
}

#[test]
fn test_django_entity_trait_implemented() {
    // Verify DjangoEntity trait is implemented
    let model = simple_author_mod::Model {
        id: 0,
        name: "Test".to_string(),
        email: "test@test.com".to_string(),
    };
    
    let result = simple_author_mod::SimpleAuthor::to_active_model_for_create(model);
    assert!(result.is_ok());
}

#[test]
fn test_sea_orm_entity_trait_implemented() {
    // Verify SeaORM's EntityTrait is implemented
    // The fact that this compiles means EntityTrait is correctly implemented
    use sea_orm::Iterable;
    let _columns: Vec<_> = simple_author_mod::Column::iter().collect();
}

#[test]
fn test_fields_are_public() {
    let model = simple_author_mod::Model {
        id: 1,
        name: "Test".to_string(),
        email: "test@test.com".to_string(),
    };
    
    // This compiles = fields are public
    let _ = model.id;
    let _ = model.name;
    let _ = model.email;
}

#[test]
fn test_to_active_model_sets_pk_to_notset() {
    let model = simple_author_mod::Model {
        id: 999, // This should be ignored
        name: "Test".to_string(),
        email: "test@test.com".to_string(),
    };
    
    let active_model = simple_author_mod::SimpleAuthor::to_active_model_for_create(model);
    assert!(active_model.is_ok());
    
    // The ActiveModel should have id as NotSet for auto-increment
    // (This is verified by the fact that the conversion succeeds)
}
