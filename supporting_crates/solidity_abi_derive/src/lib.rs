#![allow(clippy::new_without_default)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::bool_comparison)]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::result_unit_err)]
#![allow(clippy::double_must_use)]
#![allow(clippy::explicit_auto_deref)]

use proc_macro::TokenStream;

mod derive_selector;
mod simple_func_selector;
mod solidity_codable_derive;
mod utils;

#[proc_macro_derive(SolidityCodable)]
// #[proc_macro_derive(SolidityCodable, attributes(CSSelectableBound))]
#[proc_macro_error2::proc_macro_error]
pub fn derive_solidity_codable(input: TokenStream) -> TokenStream {
    self::solidity_codable_derive::derive(input)
}

#[proc_macro]
#[proc_macro_error2::proc_macro_error]
pub fn derive_simple_func_selector(input: TokenStream) -> TokenStream {
    self::simple_func_selector::derive(input)
}

#[proc_macro_attribute]
#[proc_macro_error2::proc_macro_error]
pub fn compute_selector(attr: TokenStream, item: TokenStream) -> TokenStream {
    self::derive_selector::derive(attr, item)
}
