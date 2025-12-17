//! Compile-fail test: Accessing relation field after all() without prefetch
//!
//! This test verifies that all() without prefetch_related returns Vec<Model>,
//! and therefore accessing relation fields causes a compile-time error.

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

use models::book::Book;

async fn example(db: &ormada::router::DatabaseRouter) {
    // all() without prefetch_related returns Vec<Model>
    let books = Book::objects(db)
        .all()
        .await
        .unwrap();

    for book in &books {
        // ERROR: Model does not have `author` field
        let _name = book.author.name;
    }
}

fn main() {}
