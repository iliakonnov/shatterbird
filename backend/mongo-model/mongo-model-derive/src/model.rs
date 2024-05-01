use proc_macro::TokenStream;

use darling::FromDeriveInput;
use quote::quote;
use syn::DeriveInput;

#[derive(Default, FromDeriveInput)]
#[darling(attributes(mongo_model))]
struct Options {
    collection: String,
}

pub(crate) fn process(input: DeriveInput) -> Result<TokenStream, darling::Error> {
    let Options { collection } = Options::from_derive_input(&input)?;

    let DeriveInput { ident, .. } = input;

    let res = quote! {
        impl ::mongo_model::Model for #ident {
            const COLLECTION: &'static str = #collection;

            fn id(&self) -> ::mongo_model::Id<Self> {
                self.id
            }
        }
    };
    Ok(res.into())
}
