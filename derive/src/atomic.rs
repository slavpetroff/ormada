use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

pub fn impl_atomic(args: TokenStream, input: TokenStream) -> TokenStream {
    let func = parse_macro_input!(input as ItemFn);
    let db_arg_name = parse_macro_input!(args as syn::Ident);

    let func_vis = &func.vis;
    let func_sig = &func.sig;
    let func_block = &func.block;

    // Ensure function is async
    if func.sig.asyncness.is_none() {
        return syn::Error::new_spanned(
            func.sig.fn_token,
            "#[atomic] can only be used on async functions",
        )
        .to_compile_error()
        .into();
    }

    let expanded = quote! {
        #func_vis #func_sig {
            use seaorm_django::prelude::AtomicExt;

            // Execute the body within an atomic transaction
            // The closure receives 'txn' which is &DatabaseTransaction
            #db_arg_name.atomic(move |txn| std::boxed::Box::pin(async move {
                // Shadow the db argument with the transaction handle
                // This allows existing code using 'db' to work seamlessly with the transaction
                let #db_arg_name = txn;

                // Execute the original function body
                // Since we wrapped it in async move, we can await inside
                {
                    #func_block
                }
            })).await
        }
    };

    TokenStream::from(expanded)
}
