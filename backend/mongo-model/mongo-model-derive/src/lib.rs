extern crate proc_macro;
use darling::FromDeriveInput;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::visit_mut::VisitMut;
use syn::{
    parse_macro_input, parse_quote, DataStruct, DeriveInput, Field, Path, PathArguments,
    PathSegment, Type, TypePath,
};

#[derive(Default, FromDeriveInput)]
#[darling(attributes(mongo_model))]
struct Options {
    collection: String,
}

fn sanitize_type(root: &Type, ty: &mut Type) {
    struct Visitor<'a> {
        root: &'a Type,
    }

    impl VisitMut for Visitor<'_> {
        fn visit_type_mut(&mut self, i: &mut Type) {
            syn::visit_mut::visit_type_mut(self, i);
            let segments = match i {
                Type::Path(TypePath {
                    path:
                        Path {
                            leading_colon: None,
                            segments,
                            ..
                        },
                    ..
                }) => segments,
                _ => return,
            };
            let segments = segments.iter().collect::<Vec<_>>();
            if segments.len() != 1 {
                return;
            }
            let ident = match segments[0] {
                PathSegment {
                    ident,
                    arguments: PathArguments::None,
                } => ident,
                _ => return,
            };
            if ident == "Self" {
                *i = self.root.clone();
            }
        }
    }

    Visitor { root }.visit_type_mut(ty);
}

fn process(input: DeriveInput) -> Result<TokenStream, darling::Error> {
    let Options { collection } = Options::from_derive_input(&input)?;

    let DeriveInput { ident, data, .. } = input;

    let DataStruct { fields, .. } = match data {
        syn::Data::Struct(s) => s,
        _ => return Err(darling::Error::custom("Only structs are supported")),
    };

    let filter_type = format_ident!("{}Filter", ident);
    let struct_ty: Type = parse_quote!(#ident);

    let field_names = fields
        .iter()
        .map(|Field { ident, .. }| ident)
        .cloned()
        .collect::<Vec<_>>();

    let filter_fields = fields.iter().map(
        |Field {
             ident: field,
             ty,
             ..
         }| {
            let mut ty = ty.clone();
            sanitize_type(&struct_ty, &mut ty);
            quote! {
                #[serde(default)]
                #[serde(flatten)]
                #[serde(skip_serializing_if = "Option::is_none")]
                #field: Option<::mongo_model::bson::Bson>,
            }
        },
    );

    let filter_helpers = fields.iter().map(
        |Field {
             ident: field,
             ty,
             attrs,
             ..
         }| {
            let mut ty = ty.clone();
            sanitize_type(&struct_ty, &mut ty);
            let serde_attrs = attrs
                .iter()
                .filter(|a| {
                    a.meta
                        .path()
                        .get_ident()
                        .map(|i| *i == "serde")
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>();
            quote! {
                #[derive(::mongo_model::serde::Serialize, ::mongo_model::serde::Deserialize)]
                #[allow(non_camel_case_types)]
                struct #field {
                    #(#serde_attrs)*
                    val: #ty,

                    #[serde(skip)]
                    _phantom: core::marker::PhantomData<#ident>,
                }
            }
        },
    );

    let filter_fns = fields.iter().map(
        |Field {
             ident: field,
             ty,
             ..
         }| {
            let mut ty = ty.clone();
            sanitize_type(&struct_ty, &mut ty);
            let field = field.as_ref().expect("field must have a name");
            let field_name = field.to_string();
            let trimmed = field_name.trim_start_matches('_');
            let name = syn::Ident::new(trimmed, field.span());
            let like = format_ident!("{}_like", name);
            let any = format_ident!("{}_any", name);
            quote! {
                pub fn #name(mut self, val: #ty) -> Self {
                    self.#field = Some(::mongo_model::bson::ser::to_bson(&#field {
                        val,
                        _phantom: ::core::marker::PhantomData::default(),
                    }).unwrap());
                    self
                }

                pub fn #like(mut self, val: ::mongo_model::bson::Document) -> Self {
                    self.#field = Some(::mongo_model::bson::Bson::Document(val));
                    self
                }

                pub fn #any(mut self, items: impl ::core::iter::IntoIterator<Item=#ty>) -> Self {
                    let values = items
                        .into_iter()
                        .map(|val| #field {
                            val,
                            _phantom: ::core::marker::PhantomData::default(),
                        })
                        .map(|x| ::mongo_model::bson::ser::to_document(&x).unwrap())
                        .collect::<::std::vec::Vec<_>>();
                    // Given array
                    //      [ {x: 1, y: 'a'}, {x: 2, y: 'b'} ]
                    // we need to convert it to
                    //      {x: {$in: [1, 2]}, y: {$in: ['a', 'b']}}
                    let mut result = values
                        .iter()
                        .flat_map(|doc| doc.iter())
                        .map(|(k, _)| (k.clone(), ::std::vec::Vec::<::mongo_model::bson::Bson>::new()))
                        .collect::<::std::collections::HashMap<_, _>>();
                    assert_eq!(result.len(), 1);
                    for (k, v) in values.into_iter().flat_map(|doc| doc.into_iter()) {
                        result.get_mut(&k).unwrap().push(v);
                    }
                    let result = result
                        .into_iter()
                        .map(|(k, v)| (k, ::mongo_model::bson::bson!({"$in": v})))
                        .map(|(k, v)| (k, ::mongo_model::bson::Bson::from(v)))
                        .collect();
                    let result = ::mongo_model::bson::Bson::Document(result);
                    let filter: Self = ::mongo_model::bson::de::from_bson(result).unwrap();
                    self.#field = filter.#field;
                    self
                }
            }
        },
    );

    let res = quote! {
        impl ::mongo_model::Model for #ident {
            const COLLECTION: &'static str = #collection;

            fn id(&self) -> ::mongo_model::Id<Self> {
                self.id
            }
        }

        impl #ident {
            pub fn filter() -> #filter_type {
                <_ as core::default::Default>::default()
            }
        }

        #[derive(Debug, Clone, ::mongo_model::serde::Serialize, ::mongo_model::serde::Deserialize)]
        pub struct #filter_type {
            #(#filter_fields)*

            #[serde(skip)]
            _phantom: core::marker::PhantomData<#ident>,
        }

        const _: () = {
            impl ::core::default::Default for #filter_type {
                fn default() -> Self {
                    Self {
                        #(
                            #field_names: None,
                        )*
                        _phantom: ::core::marker::PhantomData::default(),
                    }
                }
            }

            #(#filter_helpers)*

            impl #filter_type {
                #(#filter_fns)*
            }

            impl ::mongo_model::ModelFilter for #filter_type {
                type Model = #ident;

                fn build(self) -> Option<::mongo_model::bson::Document> {
                    ::mongo_model::bson::ser::to_document(&self).ok()
                }
            }
        };
    };
    Ok(res.into())
}

#[proc_macro_derive(Model, attributes(mongo_model))]
pub fn derive_mongo_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input);
    process(input).unwrap_or_else(|e| e.write_errors().into())
}
