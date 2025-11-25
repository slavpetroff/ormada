use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

/// Implements `#[atomic]` proc macro for transaction management with static dispatch.
///
/// This macro transforms an async function to automatically wrap its body in a transaction.
/// Uses static dispatch - NO `Box::pin` or `dyn Future`.
///
/// # Example
///
/// ```rust,ignore
/// #[atomic(db)]
/// async fn create_book_with_author(
///     db: &DatabaseConnection,
///     title: String,
/// ) -> Result<Book, DjangoOrmError> {
///     let author = Author::objects(db).create(...).await?;
///     Book::objects(db).create(Book { author_id: author.id, ... }).await
/// }
/// ```
///
/// Expands to code that:
/// 1. Begins a transaction
/// 2. Shadows `db` with `&transaction`
/// 3. Executes the function body
/// 4. Commits on success, rolls back on error
pub fn impl_atomic(args: TokenStream, input: TokenStream) -> TokenStream {
    let func = parse_macro_input!(input as ItemFn);
    let db_arg_name = parse_macro_input!(args as syn::Ident);

    let func_vis = &func.vis;
    let func_sig = &func.sig;
    let func_block = &func.block;
    let func_attrs = &func.attrs;

    // Ensure function is async
    if func.sig.asyncness.is_none() {
        return syn::Error::new_spanned(
            func.sig.fn_token,
            "#[atomic] can only be used on async functions",
        )
        .to_compile_error()
        .into();
    }

    // Generate code with STATIC dispatch - no Box::pin!
    // This creates a concrete future type at the call site
    let expanded = quote! {
        #(#func_attrs)*
        #func_vis #func_sig {
            use sea_orm::TransactionTrait;

            // Begin transaction - static dispatch, no boxing
            let __txn = #db_arg_name.begin().await?;

            // Execute the body with transaction shadowing the db argument
            let __result: Result<_, seaorm_django::error::DjangoOrmError> = async {
                // Shadow the db argument with the transaction handle
                let #db_arg_name = &__txn;

                // Execute the original function body
                #func_block
            }.await;

            // Commit or rollback based on result
            match __result {
                Ok(__value) => {
                    __txn.commit().await?;
                    Ok(__value)
                }
                Err(__err) => {
                    // Attempt rollback, but prioritize original error
                    let _ = __txn.rollback().await;
                    Err(__err)
                }
            }
        }
    };

    TokenStream::from(expanded)
}
