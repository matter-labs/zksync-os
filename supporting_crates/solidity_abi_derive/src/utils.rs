use proc_macro2::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::{GenericParam, Generics, Ident, LifetimeParam};

pub(crate) fn format_type_params_from_generics(
    generics: &Generics,
    lifetime: Option<LifetimeParam>,
) -> TokenStream {
    let mut result = TokenStream::new();
    if let Some(lifetime) = lifetime {
        result.extend(quote! {
            #lifetime,
        });
    }

    for param in generics.type_params() {
        result.extend(quote! {
            #param.ident,
        });
    }

    for param in generics.const_params() {
        result.extend(quote! {
            #param.ident,
        });
    }

    result
}

pub(crate) fn get_reflection_ident(original_ident: &Ident) -> Ident {
    let mut witness_ident_str = original_ident.to_string();
    witness_ident_str.push_str(&"Ref");
    syn::parse_str(&witness_ident_str).unwrap()
}

pub(crate) fn get_reflection_mut_ident(original_ident: &Ident) -> Ident {
    let mut witness_ident_str = original_ident.to_string();
    witness_ident_str.push_str(&"RefMut");
    syn::parse_str(&witness_ident_str).unwrap()
}

pub(crate) fn get_type_params_from_generics_output_params<P: Clone + Default>(
    generics: &Generics,
    punc: &P,
    lifetime: &LifetimeParam,
) -> Punctuated<GenericParam, P> {
    let type_params = generics.type_params();
    let const_params = generics.const_params();

    let mut idents = Punctuated::new();
    idents.push(GenericParam::Lifetime(lifetime.clone()));
    idents.push_punct(punc.clone());

    for param in type_params.into_iter() {
        idents.push(GenericParam::Type(param.clone()));
        idents.push_punct(punc.clone());
    }

    for param in const_params.into_iter() {
        idents.push(GenericParam::Const(param.clone()));
        idents.push_punct(punc.clone());
    }

    idents
}
