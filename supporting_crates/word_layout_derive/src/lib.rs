use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

const SUB_WORD_TYPES: &[&str] = &["bool", "u8", "u16"];
const DYNAMIC_TYPES: &[&str] = &["Vec", "Box", "String"];

fn is_sub_word_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(p) = ty {
        if let Some(seg) = p.path.segments.last() {
            return SUB_WORD_TYPES.contains(&seg.ident.to_string().as_str());
        }
    }
    false
}

fn is_dynamic_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(p) = ty {
        if let Some(seg) = p.path.segments.last() {
            return DYNAMIC_TYPES.contains(&seg.ident.to_string().as_str());
        }
    }
    false
}

fn has_repr_c(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if attr.path().is_ident("repr") {
            let mut found = false;
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("C") {
                    found = true;
                }
                Ok(())
            });
            if found {
                return true;
            }
        }
    }
    false
}

#[proc_macro_derive(WordLayout)]
pub fn derive_word_layout(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(f) => &f.named,
            _ => panic!("WordLayout derive only supports structs with named fields"),
        },
        _ => panic!("WordLayout derive only supports structs"),
    };

    let field_names: Vec<_> = fields.iter().map(|f| &f.ident).collect();
    let field_types: Vec<_> = fields.iter().map(|f| &f.ty).collect();

    let has_sub_word = fields.iter().any(|f| is_sub_word_type(&f.ty));
    let has_dynamic = fields.iter().any(|f| is_dynamic_type(&f.ty));
    let repr_c = has_repr_c(&input.attrs);

    // WORD_COUNT: match on all field word counts
    let word_count = {
        let match_arms: Vec<_> = field_types
            .iter()
            .enumerate()
            .map(|(i, ty)| {
                let var = syn::Ident::new(&format!("n{i}"), proc_macro2::Span::call_site());
                (
                    var,
                    quote! { <#ty as zk_ee::oracle::word_layout::WordLayout>::WORD_COUNT },
                )
            })
            .collect();

        if match_arms.is_empty() {
            quote! { Some(0) }
        } else {
            let vars: Vec<_> = match_arms.iter().map(|(v, _)| v).collect();
            let exprs: Vec<_> = match_arms.iter().map(|(_, e)| e).collect();
            let patterns: Vec<_> = vars.iter().map(|v| quote! { Some(#v) }).collect();
            let sum = vars
                .iter()
                .fold(None, |acc: Option<TokenStream2>, v| {
                    Some(match acc {
                        None => quote! { #v },
                        Some(prev) => quote! { #prev + #v },
                    })
                })
                .unwrap();

            quote! {
                match ( #( #exprs ),* ) {
                    ( #( #patterns ),* ) => Some(#sum),
                    _ => None,
                }
            }
        }
    };

    // write_words: always field-by-field
    let write_body = quote! {
        #( zk_ee::oracle::word_layout::WordLayout::write_words(&self.#field_names, w); )*
    };

    // read_words: bulk or field-by-field
    let qualifies_for_bulk = !has_sub_word && !has_dynamic && !field_types.is_empty();

    let read_body = if qualifies_for_bulk && repr_c {
        // Bulk path: direct u32 store loop.
        // Only valid when struct size matches word count * 4 (no padding)
        // and alignment is >= 4.
        quote! {
            const { assert!(core::mem::align_of::<Self>() >= 4, "WordLayout bulk read: struct alignment must be >= 4") };
            const { assert!(
                core::mem::size_of::<Self>() == <Self as zk_ee::oracle::word_layout::WordLayout>::WORD_COUNT.unwrap() * 4,
                "WordLayout bulk read: struct has internal padding — size_of != WORD_COUNT * 4. Reorder fields to eliminate padding."
            ) };
            let mut result = core::mem::MaybeUninit::<Self>::uninit();
            let dst = result.as_mut_ptr() as *mut u32;
            for i in 0..(core::mem::size_of::<Self>() / 4) {
                unsafe { dst.add(i).write(r()); }
            }
            unsafe { result.assume_init() }
        }
    } else if qualifies_for_bulk && !repr_c {
        // Eligible for bulk but missing repr(C) — fall back to field-by-field.
        // This avoids forcing users to add repr(C), while still producing
        // correct (though slightly slower) code.
        quote! {
            Self {
                #( #field_names: <#field_types as zk_ee::oracle::word_layout::WordLayout>::read_words(r), )*
            }
        }
    } else {
        // Field-by-field path
        quote! {
            Self {
                #( #field_names: <#field_types as zk_ee::oracle::word_layout::WordLayout>::read_words(r), )*
            }
        }
    };

    // read_words_into: for field-by-field path, delegate to each field's read_words_into
    // For bulk path, the default (*self = read_words) is optimal
    let read_into_body = if qualifies_for_bulk && repr_c {
        // Bulk: default is fine (full overwrite)
        None
    } else {
        // Field-by-field: delegate to each field's read_words_into
        Some(quote! {
            #( zk_ee::oracle::word_layout::WordLayout::read_words_into(&mut self.#field_names, r); )*
        })
    };

    let read_into_method = read_into_body.map(|body| {
        quote! {
            fn read_words_into(&mut self, r: &mut impl FnMut() -> u32) {
                #body
            }
        }
    });

    let expanded = quote! {
        impl #impl_generics zk_ee::oracle::word_layout::WordLayout for #name #ty_generics #where_clause {
            const WORD_COUNT: Option<usize> = #word_count;

            fn write_words(&self, w: &mut impl FnMut(u32)) {
                #write_body
            }

            fn read_words(r: &mut impl FnMut() -> u32) -> Self {
                #read_body
            }

            #read_into_method
        }
    };

    expanded.into()
}
