//! Helper macro for ergonomic lifecycle hooks implementation

/// Ergonomic macro to implement lifecycle hooks without Box::pin boilerplate!
///
/// This macro lets you write clean `async fn` methods that return `Result<(), DjangoOrmError>`.
/// The macro handles all the Pin<Box<...>> wrapping internally.
///
/// # Example
///
/// ```rust,ignore
/// use seaorm_django::prelude::*;
///
/// #[django_model(table = "users")]
/// pub struct User {
///     #[primary_key]
///     pub id: i32,
///     pub email: String,
///     pub updated_at: DateTime<FixedOffset>,
/// }
///
/// // Clean ergonomic hooks - NO Box::pin needed!
/// hooks! {
///     impl User {
///         async fn before_save(&mut self) -> Result<(), DjangoOrmError> {
///             self.updated_at = Utc::now().into();
///             Ok(())
///         }
///         
///         async fn after_create<C: ConnectionTrait>(&self, db: &C) -> Result<(), DjangoOrmError> {
///             println!("User {} created!", self.email);
///             Ok(())
///         }
///     }
/// }
/// ```
#[macro_export]
macro_rules! hooks {
    // Entry point
    (impl $model:ident { $($tt:tt)* }) => {
        hooks! { @impl $model [] [] $($tt)* }
    };
    
    // Parse &mut self methods
    (@impl $model:ident [$($mut_methods:tt)*] [$($immut_methods:tt)*] 
     async fn $method:ident(&mut self) -> Result<(), DjangoOrmError> $body:block 
     $($rest:tt)*
    ) => {
        hooks! { @impl $model 
            [$($mut_methods)* { $method $body }] 
            [$($immut_methods)*] 
            $($rest)* 
        }
    };
    
    // Parse &self methods with generic
    (@impl $model:ident [$($mut_methods:tt)*] [$($immut_methods:tt)*]
     async fn $method:ident<$($generics:tt)*>(&self, $param:ident: &$gen2:ident) -> Result<(), DjangoOrmError> $body:block
     $($rest:tt)*
    ) => {
        hooks! { @impl $model 
            [$($mut_methods)*] 
            [$($immut_methods)* { $method [$($generics)*] $param $gen2 $body }]
            $($rest)* 
        }
    };
    
    // Generate impl
    (@impl $model:ident [$(  { $mut_method:ident $mut_body:block } )*] [$( { $immut_method:ident [$($generics:tt)*] $param:ident $gen2:ident $immut_body:block } )*]) => {
        impl $crate::hooks::AsyncLifecycleHooks for $model {
            $(
                fn $mut_method(&mut self) -> ::std::pin::Pin<::std::boxed::Box<dyn ::std::future::Future<Output = Result<(), $crate::error::DjangoOrmError>> + Send + '_>> {
                    ::std::boxed::Box::pin(async move $mut_body)
                }
            )*
            $(
                fn $immut_method<$($generics)*>(&self, $param: &$gen2) -> ::std::pin::Pin<::std::boxed::Box<dyn ::std::future::Future<Output = Result<(), $crate::error::DjangoOrmError>> + Send + '_>> {
                    ::std::boxed::Box::pin(async move $immut_body)
                }
            )*
        }
    };
}
