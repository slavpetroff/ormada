//! Compile-fail test: Accessing relation field on Model (nullable FK)
//!
//! This test verifies that accessing `article.author` on a Model (not ModelWithRelations)
//! causes a compile-time error, even for nullable foreign keys.

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

    pub mod article {
        use super::*;

        #[ormada_model(table = "articles")]
        pub struct Article {
            #[primary_key]
            pub id: i32,
            #[foreign_key(Author, on_delete = SetNull)]
            pub author_id: Option<i32>,
            pub title: String,
            pub content: String,
            #[auto_now_add]
            pub created_at: DateTimeWithTimeZone,
            #[auto_now]
            pub updated_at: DateTimeWithTimeZone,
        }
    }
}

use models::article::Article;

fn main() {
    // Create an Article Model (not ModelWithRelations)
    let article: Article = Article {
        id: 1,
        author_id: Some(1),
        title: "Test Article".to_string(),
        content: "Content".to_string(),
        ..Default::default()
    };

    // ERROR: Model does not have `author` field - only ModelWithRelations does
    // This should cause a compile error: "no field `author` on type `Model`"
    let _author = article.author;
}
