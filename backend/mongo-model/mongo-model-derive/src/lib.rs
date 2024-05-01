extern crate proc_macro;

use proc_macro::TokenStream;
use syn::parse_macro_input;

mod model;

#[proc_macro_derive(Model, attributes(mongo_model))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input);
    model::process(input).unwrap_or_else(|e| e.write_errors().into())
}
