extern crate proc_macro;
use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn declare(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn entrypoint(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn infer(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
