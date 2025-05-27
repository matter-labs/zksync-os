//! Generic emit utilities.

/// Creates a const identifier for abi.
/// TODO: Convert to a generic prefix/affix.
pub(crate) fn ident_for_entry_abi(ident: &syn::Ident) -> proc_macro2::Ident {
    let mut const_ident = "ABI_FOR_".to_string();
    const_ident.push_str(ident.to_string().as_str());

    let const_ident = proc_macro2::Ident::new(const_ident.as_str(), proc_macro2::Span::call_site());

    const_ident
}

// pub(crate) fn reduce_iter<T: IntoIterator<Item = TokenStream>>(list: T) -> TokenStream {
//     let mut r = TokenStream::new();
//
//     for i in list.into_iter() {
//         r.extend(i);
//     }
//
//     r
// }
