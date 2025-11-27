//! Helper macro for ergonomic lifecycle hooks implementation
//!
//! NOTE: This macro is now deprecated. Use `#[async_trait]` directly instead:
//!
//! ```rust,ignore
//! use ormada::prelude::*;
//!
//! #[async_trait]
//! impl LifecycleHooks for User {
//!     async fn before_save(&mut self) -> Result<(), OrmadaOrmError> {
//!         self.updated_at = Utc::now().into();
//!         Ok(())
//!     }
//! }
//! ```

/// Legacy macro - use `#[async_trait] impl LifecycleHooks` instead
#[macro_export]
macro_rules! hooks {
    // Entry point - parse methods and generate #[async_trait] impl
    (impl $model:ident { $($tt:tt)* }) => {
        hooks! { @impl $model [] [] $($tt)* }
    };

    // Parse &mut self methods
    (@impl $model:ident [$($mut_methods:tt)*] [$($immut_methods:tt)*]
     async fn $method:ident(&mut self) -> Result<(), OrmadaOrmError> $body:block
     $($rest:tt)*
    ) => {
        hooks! { @impl $model
            [$($mut_methods)* { $method $body }]
            [$($immut_methods)*]
            $($rest)*
        }
    };

    // Parse &self methods (no generic needed anymore)
    (@impl $model:ident [$($mut_methods:tt)*] [$($immut_methods:tt)*]
     async fn $method:ident(&self) -> Result<(), OrmadaOrmError> $body:block
     $($rest:tt)*
    ) => {
        hooks! { @impl $model
            [$($mut_methods)*]
            [$($immut_methods)* { $method $body }]
            $($rest)*
        }
    };

    // Generate impl using async_trait
    (@impl $model:ident [$( { $mut_method:ident $mut_body:block } )*] [$( { $immut_method:ident $immut_body:block } )*]) => {
        #[::async_trait::async_trait]
        impl $crate::hooks::LifecycleHooks for $model {
            $(
                async fn $mut_method(&mut self) -> ::core::result::Result<(), $crate::error::OrmadaOrmError> $mut_body
            )*
            $(
                async fn $immut_method(&self) -> ::core::result::Result<(), $crate::error::OrmadaOrmError> $immut_body
            )*
        }
    };
}
