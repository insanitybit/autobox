extern crate proc_macro;
use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn declare(attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn entrypoint(attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn infer(attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
