#![allow(clippy::all)]
#![allow(warnings)]
use proc_macro::{Span, TokenStream};
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span as Span2, TokenStream as TokenStream2};
use quote::{ToTokens, quote};
use syn::{DeriveInput, Ident, Item, ReturnType, Signature, parse_macro_input};

use crate::util::{InputType, parse_input_type};

mod handler;
mod util;

#[proc_macro_attribute]
pub fn handler(_args: TokenStream, input: TokenStream) -> TokenStream {
    let item = parse_macro_input!(input);
    match generate(item) {
        Ok(stream) => stream.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

pub(crate) fn generate(input: Item) -> syn::Result<TokenStream2> {
    let this = ident_crate();

    match input {
        Item::Fn(mut item_fn) => {
            let attrs = item_fn
                .attrs
                .iter()
                .filter(|a| !a.path().is_ident("handler"))
                .collect::<Vec<_>>();
            let vis = &item_fn.vis;
            let sig = &mut item_fn.sig;
            let body = &item_fn.block;
            let name = &sig.ident;
            let docs = item_fn
                .attrs
                .iter()
                .filter(|a| !a.path().is_ident("doc"))
                .collect::<Vec<_>>();
            let struct_impl = quote! {
                #(#docs)*
                #[allow(non_camel_case_types)]
                #[derive(Debug)]
                #vis struct #name;
                impl #name {
                    #(#attrs)*
                    #sig {
                        #body
                    }
                }
            };

            let handler_impl = gen_handler_impl(&this, sig)?;

            Ok(quote! {
                #struct_impl
                #[#this::async_trait]
                impl #this::Handler for #name {
                    #handler_impl
                }

            })
        }
        _ => Err(syn::Error::new_spanned(
            input,
            "#[handler] must added to `impl` or `fn`",
        )),
    }
}

fn gen_handler_impl(this: &Ident, sig: &Signature) -> syn::Result<TokenStream2> {
    let name = &sig.ident;
    let mut call_args: Vec<Ident> = Vec::with_capacity(sig.inputs.len());

    for input in &sig.inputs {
        match parse_input_type(input) {
            InputType::Receiver(_) => call_args.push(Ident::new("self", Span2::call_site())),
            InputType::Request(_) => call_args.push(Ident::new("req", Span2::call_site())),
            InputType::Response(_) => call_args.push(Ident::new("resp", Span2::call_site())),
            InputType::Unknown => {
                let msg = "the inputs parameters must be Request, Response";
                return Err(syn::Error::new_spanned(&sig.inputs, msg));
            }
            _ => {
                let msg = "the inputs parameters must be Request, Response";
                return Err(syn::Error::new_spanned(&sig.inputs, msg));
            }
        }
    }

    match sig.output {
        ReturnType::Default => {
            if sig.asyncness.is_none() {
                Ok(quote! {
                    async fn handle(
                        &self,
                        req: &mut #this::Request,
                        resp: &mut #this::Response,
                    ) {
                        Self::#name(#(#call_args), *)
                    }
                })
            } else {
                Ok(quote! {
                    async fn handle(
                        &self,
                        req: &mut #this::Request,
                        resp: &mut #this::Response,
                    ) {
                        Self::#name(#(#call_args), *).await
                    }
                })
            }
        }
        ReturnType::Type(..) => {
            if sig.asyncness.is_none() {
                Ok(quote! {
                    async fn handle(
                        &self,
                        req: &mut #this::Request,
                        resp: &mut #this::Response,
                    ) {
                        #this::Writer::write(Self::#name(#(#call_args),*), req, resp).await;
                    }
                })
            } else {
                Ok(quote! {
                    async fn handle(
                        &self,
                        req: &mut #this::Request,
                        resp: &mut #this::Response,
                    ) {
                        Self::#name(#(#call_args), *).await
                        #this::Writer::write(Self::#name(#(#call_args),*).await, req, resp).await;
                    }
                })
            }
        }
    }
}

pub(crate) fn ident_crate() -> Ident {
    match crate_name("monet") {
        Ok(monet) => match monet {
            FoundCrate::Itself => Ident::new("monet", Span2::call_site()),
            FoundCrate::Name(name) => Ident::new(&name, Span2::call_site()),
        },
        Err(_) => Ident::new("monet", Span2::call_site()),
    }
}
