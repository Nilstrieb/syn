use crate::{cfg, file, lookup};
use anyhow::Result;
use proc_macro2::{Ident, Span, TokenStream};
use quote::{format_ident, quote};
use std::convert::TryFrom;
use syn_codegen::{Data, Definitions, Node, Type};

const DEBUG_SRC: &str = "../src/gen/arbitrary.rs";

fn expand_impl_body(defs: &Definitions, node: &Node) -> TokenStream {
    let type_name = &node.ident;
    let ident = Ident::new(type_name, Span::call_site());

    match &node.data {
        Data::Enum(variants) => {
            let count = u8::try_from(variants.len()).unwrap();

            let arms = variants
                .iter()
                .enumerate()
                .map(|(idx, (variant_name, fields))| {
                    let idx = u8::try_from(idx).unwrap();
                    let variant = Ident::new(variant_name, Span::call_site());
                    if fields.is_empty() {
                        quote! {
                            #idx => #ident::#variant,
                        }
                    } else {
                        let mut arbitraries = Vec::new();
                        for i in 0..fields.len() {
                            arbitraries.push(quote!(Arbitrary::arbitrary(u)?));
                        }
                        quote! {
                            #idx => #ident::#variant(#(#arbitraries),*),
                        }
                    }
                })
                .collect::<Vec<_>>();

            quote! {{
                let index = (0..#count).arbitrary(u)?;
                match index {
                    #(#arms)*
                }
            }}
        }
        Data::Struct(fields) => {
            let fields = fields.keys().map(|f| {
                let ident = Ident::new(f, Span::call_site());
                quote! {
                    #ident: Arbitrary::arbitrary(u)?,
                }
            });
            quote!(#ident { #(#fields)* })
        }
        Data::Private => {
            unreachable!()
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
            fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
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
            use crate::*;
            use arbitrary::{Arbitrary, Unstructured};

            #impls
        },
    )?;

    Ok(())
}
