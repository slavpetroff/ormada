// Integration tests are allowed to use unwrap/expect for clarity
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]

#[path = "soft_delete/test_basic.rs"]
mod test_basic;
#[path = "soft_delete/test_queries.rs"]
mod test_queries;
#[path = "soft_delete/test_restore.rs"]
mod test_restore;
