#![feature(proc_macro_diagnostic)]
use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse_macro_input, LitByte};

mod abi;
mod analysis;
mod contract;
mod emit;
pub(crate) mod qol;

// const DEV: bool = if cfg!(debug_assertions) { false } else { false };
const DEV: bool = false;

// #[proc_macro_attribute]
// pub fn entry(_attr: TokenStream, item: TokenStream) -> TokenStream {
//     let mut input = syn::parse_macro_input!(item as syn::ItemFn);
//     let name = &input.sig.ident;
//     let selector = u32::from_be_bytes(abi::encode_signature(&input.sig));
//
//     let const_ident = emit::ident_for_entry_abi(name);
//
//     input.vis = syn::parse2("pub(crate)".parse().unwrap()).unwrap();
//
//     let mut r = quote! {
//         pub(crate) const #const_ident: u32 = #selector;
//     };
//
//     r.extend(input.to_token_stream());
//     r.into()
// }

#[proc_macro_attribute]
pub fn contract(attr: TokenStream, item: TokenStream) -> TokenStream {
    match contract::apply(attr.into(), item.into()) {
        Ok(x) => x,
        Err(x) => x.into_compile_error()
    }.into()
}

#[proc_macro_attribute]
pub fn selector(attr: TokenStream, item: TokenStream) -> TokenStream {
    let x = parse_macro_input!(attr as syn::LitInt);
    item
}

#[proc_macro]
pub fn tuple_overlay_derive(input: TokenStream) -> TokenStream {
    let count = parse_macro_input!(input as syn::LitInt)
        .base10_parse()
        .unwrap();

    let idents = (0..count)
        .map(|x| format!("T{}", x))
        .map(|x| proc_macro2::Ident::new(x.as_str(), proc_macro2::Span::call_site()));

    quote! { tuple_overlay_impl!(#(#idents),*); }.into()
}

#[proc_macro]
pub fn tuple_overlay_derive_bulk(input: TokenStream) -> TokenStream {
    let count = parse_macro_input!(input as syn::LitInt)
        .base10_parse()
        .unwrap();

    let lits = (1..=count).map(proc_macro2::Literal::usize_unsuffixed);

    quote! { #(tuple_overlay_derive!(#lits);)* }.into()
}
