use crate::{cfg, file};
use anyhow::Result;
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use std::convert::TryFrom;
use syn_codegen::{Data, Definitions, Node, Type};

const DEBUG_SRC: &str = "../src/gen/arbitrary.rs";

fn expand_impl_body(_defs: &Definitions, node: &Node) -> TokenStream {
    let type_name = &node.ident;
    let ident = Ident::new(type_name, Span::call_site());

    match &node.data {
        Data::Enum(variants) => {
            let count = u64::try_from(variants.len()).unwrap();

            let arms = variants
                .iter()
                .enumerate()
                .map(|(idx, (variant_name, fields))| {
                    let idx = u64::try_from(idx).unwrap();
                    let variant = Ident::new(variant_name, Span::call_site());
                    if fields.is_empty() {
                        quote! {
                            #idx => #ident::#variant,
                        }
                    } else {
                        let arbitraries =
                            fields.into_iter().map(ty_to_arbitrary).collect::<Vec<_>>();

                        quote! {
                            #idx => #ident::#variant(#(#arbitraries),*),
                        }
                    }
                })
                .collect::<Vec<_>>();

            quote! {{
                let index = (u64::from(u32::arbitrary(u)?) * #count) >> 32;
                match index {
                    #(#arms)*
                    _ => unreachable!(),
                }
            }}
        }
        Data::Struct(fields) => {
            let fields = fields.iter().map(|(field, ty)| {
                let ident = Ident::new(field, Span::call_site());

                let arbitrary = ty_to_arbitrary(ty);

                quote! {
                    #ident: #arbitrary,
                }
            });
            quote!(#ident { #(#fields)* })
        }
        Data::Private => {
            unreachable!()
        }
    }
}

fn ty_to_arbitrary(ty: &Type) -> TokenStream {
    match ty {
        Type::Ext(name) if name.contains("Span") => {
            quote! { Span::call_site() }
        }
        Type::Ext(name) if name.contains("Ident") => {
            quote! { Ident::new(Arbitrary::arbitrary(u)?, Span::call_site()) }
        }
        Type::Ext(name) if name.contains("TokenStream") => {
            quote! { quote::quote! {} }
        }
        Type::Ext(name) if name.contains("Literal") => {
            quote! { Literal::string(&String::arbitrary(u)?) }
        }
        Type::Option(ty) => {
            let arbitrary = ty_to_arbitrary(ty);
            quote! {
                Some(#arbitrary)
            }
        }
        Type::Box(ty) => {
            let arbitrary = ty_to_arbitrary(ty);
            quote! { Box::new(#arbitrary) }
        }
        Type::Tuple(tys) => {
            let arbitraries = tys.iter().map(ty_to_arbitrary);

            quote! { (#(#arbitraries),*) }
        }
        Type::Vec(_)
        | Type::Std(_)
        | Type::Token(_)
        | Type::Group(_)
        | Type::Syn(_)
        | Type::Punctuated(_)
        | _ => {
            quote! {Arbitrary::arbitrary(u)?}
        }
    }
}

fn expand_impl(defs: &Definitions, node: &Node) -> TokenStream {
    let ident = Ident::new(&node.ident, Span::call_site());
    let cfg_features = cfg::features(&node.features);

    if let Data::Private = &node.data {
        return quote! {};
    }

    let body = expand_impl_body(defs, node);

    quote! {
        #cfg_features
        #[cfg_attr(doc_cfg, doc(cfg(feature = "arbitrary")))]
        impl<'a> Arbitrary<'a> for #ident {
            fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
                Ok(#body)
            }
        }
    }
}

pub fn generate(defs: &Definitions) -> Result<()> {
    let mut impls = TokenStream::new();
    for node in &defs.types {
        impls.extend(expand_impl(defs, node));
    }

    file::write(
        DEBUG_SRC,
        quote! {
            #[allow(unused_variables)]

            use crate::*;
            use arbitrary::{Arbitrary, Unstructured};
            use proc_macro2::{Ident, Literal, Span};

            #impls
        },
    )?;

    Ok(())
}
