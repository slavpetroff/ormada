//! Compile-fail test: Accessing relation field on Model (non-nullable FK)
//!
//! This test verifies that accessing `book.author` on a Model (not ModelWithRelations)
//! causes a compile-time error. This is the key type safety feature.

use ormada::prelude::*;

mod models {
    use ormada::prelude::*;

    pub mod author {
        use super::*;

        #[ormada_model(table = "authors")]
        pub struct Author {
            #[primary_key]
            pub id: i32,
            pub name: String,
            pub email: String,
            pub age: i32,
            #[auto_now_add]
            pub created_at: DateTimeWithTimeZone,
            #[auto_now]
            pub updated_at: DateTimeWithTimeZone,
        }
    }

    pub mod book {
        use super::*;

        #[ormada_model(table = "books")]
        pub struct Book {
            #[primary_key]
            pub id: i32,
            #[foreign_key(Author)]
            pub author_id: i32,
            pub title: String,
            pub price: i32,
            pub published: bool,
            #[auto_now_add]
            pub created_at: DateTimeWithTimeZone,
            #[auto_now]
            pub updated_at: DateTimeWithTimeZone,
        }
    }
}

use models::author::Author;
use models::book::Book;

fn main() {
    // Create a Book Model (not ModelWithRelations)
    let book: Book = Book {
        id: 1,
        author_id: 1,
        title: "Test Book".to_string(),
        price: 1999,
        published: true,
        ..Default::default()
    };

    // ERROR: Model does not have `author` field - only ModelWithRelations does
    // This should cause a compile error: "no field `author` on type `Model`"
    let _author_id = book.author.id;
}
