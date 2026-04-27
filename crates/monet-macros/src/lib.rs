use proc_macro2::TokenStream;
use syn::{DeriveInput, Item, parse_macro_input};

mod handler;
mod util;

#[proc_macro_attribute]
pub fn handler(_args: TokenStream, input: TokenStream) -> TokenStream {
    let item = parse_macro_input!(input as Item);
    match hand

}

