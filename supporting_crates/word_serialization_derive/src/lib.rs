use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, parse_quote, Data, DeriveInput, Field, Fields, Generics, Index, Member,
};

#[proc_macro_derive(WordSerializable)]
pub fn derive_word_serializable(input: TokenStream) -> TokenStream {
    derive_word_serializable_impl(parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(WordDeserializable)]
pub fn derive_word_deserializable(input: TokenStream) -> TokenStream {
    derive_word_deserializable_impl(parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn derive_word_serializable_impl(input: DeriveInput) -> syn::Result<TokenStream2> {
    let ident = input.ident;
    let fields = extract_struct_fields(&input.data)?;
    let members = field_members(&fields);

    let mut generics = input.generics;
    add_field_bounds(
        &mut generics,
        &fields,
        parse_quote!(::zk_ee::oracle::word_serialization::WordSerializable),
    );
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics ::zk_ee::oracle::word_serialization::WordSerializable
            for #ident #ty_generics
        #where_clause
        {
            fn word_len(&self) -> usize {
                0 #( + ::zk_ee::oracle::word_serialization::WordSerializable::word_len(&self.#members) )*
            }

            fn write_words(
                &self,
                out: &mut impl ::zk_ee::oracle::word_serialization::WordSink,
            ) {
                #(
                    ::zk_ee::oracle::word_serialization::WordSerializable::write_words(&self.#members, out);
                )*
            }
        }
    })
}

fn derive_word_deserializable_impl(input: DeriveInput) -> syn::Result<TokenStream2> {
    let ident = input.ident;
    let fields = extract_struct_fields(&input.data)?;
    let constructor = constructor_tokens(&ident, &input.data)?;
    let field_bindings: Vec<_> = fields
        .iter()
        .enumerate()
        .map(|(idx, _)| format_ident!("field_{idx}", span = Span::call_site()))
        .collect();

    let mut generics = input.generics;
    add_field_bounds(
        &mut generics,
        &fields,
        parse_quote!(::zk_ee::oracle::word_serialization::WordDeserializable),
    );
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics ::zk_ee::oracle::word_serialization::WordDeserializable
            for #ident #ty_generics
        #where_clause
        {
            fn read_words(
                src: &mut impl ExactSizeIterator<Item = usize>,
            ) -> Result<Self, ::zk_ee::system::errors::internal::InternalError> {
                #(
                    let #field_bindings =
                        ::zk_ee::oracle::word_serialization::WordDeserializable::read_words(src)?;
                )*

                Ok(#constructor)
            }
        }
    })
}

fn extract_struct_fields(data: &Data) -> syn::Result<Vec<&Field>> {
    match data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields) => Ok(fields.named.iter().collect()),
            Fields::Unnamed(fields) => Ok(fields.unnamed.iter().collect()),
            Fields::Unit => Ok(Vec::new()),
        },
        _ => Err(syn::Error::new(
            Span::call_site(),
            "Word serialization derives support structs only",
        )),
    }
}

fn add_field_bounds(generics: &mut Generics, fields: &[&Field], trait_bound: syn::Path) {
    let where_clause = generics.make_where_clause();
    for field in fields {
        let ty = &field.ty;
        where_clause
            .predicates
            .push(parse_quote!(#ty: #trait_bound));
    }
}

fn field_members(fields: &[&Field]) -> Vec<Member> {
    fields
        .iter()
        .enumerate()
        .map(|(idx, field)| {
            field
                .ident
                .clone()
                .map(Member::Named)
                .unwrap_or_else(|| Member::Unnamed(Index::from(idx)))
        })
        .collect()
}

fn constructor_tokens(ident: &syn::Ident, data: &Data) -> syn::Result<TokenStream2> {
    let fields = extract_struct_fields(data)?;
    let bindings: Vec<_> = fields
        .iter()
        .enumerate()
        .map(|(idx, _)| format_ident!("field_{idx}", span = Span::call_site()))
        .collect();

    match data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(named) => {
                let idents: Vec<_> = named
                    .named
                    .iter()
                    .map(|field| field.ident.as_ref().expect("named field"))
                    .collect();
                Ok(quote!(#ident { #( #idents: #bindings ),* }))
            }
            Fields::Unnamed(_) => Ok(quote!(#ident( #( #bindings ),* ))),
            Fields::Unit => Ok(quote!(#ident)),
        },
        _ => Err(syn::Error::new(
            Span::call_site(),
            "Word serialization derives support structs only",
        )),
    }
}
