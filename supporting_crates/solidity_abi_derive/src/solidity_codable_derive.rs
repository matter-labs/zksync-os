use proc_macro2::{Ident, Span, TokenStream};
use proc_macro_error2::abort_call_site;
use quote::quote;
use syn::{
    parse_macro_input, parse_str, token::Comma, Data, DeriveInput, Fields, Generics, Lifetime,
    LifetimeParam, Type, TypePath,
};

use crate::utils::*;

pub(crate) fn derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let derived_input = parse_macro_input!(input as DeriveInput);

    let DeriveInput {
        ident,
        data,
        generics,
        ..
    } = derived_input.clone();

    let mut parse_lines = TokenStream::new();
    let mut parse_mut_lines = TokenStream::new();
    let mut initializations = TokenStream::new();
    let mut head_size = TokenStream::new();
    let mut is_dynamic = TokenStream::new();
    let mut format_lines = TokenStream::new();
    let mut selector_codable_lines = TokenStream::new();

    let num_lifetimes = generics.lifetimes().count();
    if num_lifetimes > 1 {
        abort_call_site!("Either single lifetime or none are allowed");
    }
    let lifetime_in_derived_ref = Lifetime::new("'__derived_ref", Span::call_site());
    let ref_trait: Ident = parse_str("SolidityCodableReflectionRef").unwrap();
    let ref_mut_trait: Ident = parse_str("SolidityCodableReflectionRefMut").unwrap();

    let lifetime_of_original_decl = generics.lifetimes().next().cloned();

    let lifetime_for_ref_ident = if let Some(lifetime) = generics.lifetimes().next() {
        lifetime.clone()
    } else {
        LifetimeParam::new(lifetime_in_derived_ref.clone())
    };

    let lifetime_param_for_ref = lifetime_for_ref_ident.lifetime.clone();

    // create generics for reflection trait impl
    let comma = Comma(Span::call_site());
    let punc_generic_params =
        get_type_params_from_generics_output_params(&generics, &comma, &lifetime_for_ref_ident);

    let new_generics = Generics {
        lt_token: generics.lt_token,
        params: punc_generic_params,
        gt_token: generics.gt_token,
        where_clause: generics.where_clause.clone(),
    };

    let reflection_def = {
        let DeriveInput {
            attrs: _attrs,
            vis,
            ident,
            generics: _,
            mut data,
            ..
        } = derived_input.clone();

        match data {
            Data::Struct(ref mut struct_data) => {
                match struct_data.fields {
                    // we only use named fields for now
                    Fields::Named(ref mut fields) => {
                        for field in fields.named.iter_mut() {
                            let new_ty = get_equivalent_type_recursive(
                                &field.ty,
                                &lifetime_for_ref_ident.lifetime,
                            );
                            field.ty = new_ty;
                        }
                    }
                    _ => abort_call_site!("only named fields are allowed"),
                }
            }
            _ => abort_call_site!("only structs are allowed"),
        };

        let witness_ident = get_reflection_ident(&ident);

        DeriveInput {
            attrs: vec![],
            vis,
            ident: witness_ident,
            generics: new_generics.clone(),
            data,
        }
    };

    let reflection_def_mut = {
        let DeriveInput {
            attrs: _attrs,
            vis,
            ident,
            generics: _,
            mut data,
            ..
        } = derived_input;

        match data {
            Data::Struct(ref mut struct_data) => {
                match struct_data.fields {
                    // we only use named fields for now
                    Fields::Named(ref mut fields) => {
                        for field in fields.named.iter_mut() {
                            let new_ty = get_equivalent_mut_type_recursive(
                                &field.ty,
                                &lifetime_for_ref_ident.lifetime,
                            );
                            field.ty = new_ty;
                        }
                    }
                    _ => abort_call_site!("only named fields are allowed"),
                }
            }
            _ => abort_call_site!("only structs are allowed"),
        };

        let witness_ident = get_reflection_mut_ident(&ident);

        DeriveInput {
            attrs: vec![],
            vis,
            ident: witness_ident,
            generics: new_generics.clone(),
            data,
        }
    };

    match data {
        syn::Data::Struct(ref struct_data) => match struct_data.fields {
            syn::Fields::Named(ref named_fields) => {
                let num_fields = named_fields.named.iter().count();
                for (i, field) in named_fields.named.iter().enumerate() {
                    let is_last = i == num_fields - 1;
                    let field_ident = field.ident.clone().expect("a field ident");

                    match field.ty {
                        Type::Path(ref path_ty) => {
                            parse_lines.extend(quote! {
                                let #field_ident = SolidityCodableReflectionRef::<#lifetime_for_ref_ident>::parse(source, head_offset)?;
                            });

                            parse_mut_lines.extend(quote! {
                                // unsafe, but fine for us
                                let local_source = unsafe {
                                    core::slice::from_raw_parts_mut(source.as_mut_ptr(), source.len())
                                };
                                let #field_ident = SolidityCodableReflectionRefMut::<#lifetime_for_ref_ident>::parse_mut(local_source, head_offset)?;
                            });

                            if head_size.is_empty() == false {
                                head_size.extend(quote! { + })
                            }
                            head_size.extend(quote! {
                                <#path_ty as SolidityCodable>::HEAD_SIZE
                            });

                            if is_dynamic.is_empty() == false {
                                is_dynamic.extend(quote! { | })
                            }
                            is_dynamic.extend(quote! {
                                <#path_ty as SolidityCodable>::IS_DYNAMIC
                            });

                            format_lines.extend(quote! {
                                <#path_ty as SolidityCodable>::extend_canonical_selector_encoding(buff, offset)?;
                            });
                            if is_last == false {
                                format_lines.extend(quote! {
                                    let (_, dst) = buff.split_at_mut_checked(*offset).ok_or(())?;
                                    if dst.len() == 0 {
                                        return Err(());
                                    }
                                    dst[0] = b","[0];
                                    *offset += 1;
                                });
                            }

                            selector_codable_lines.extend(quote! {
                                match <#path_ty as SelectorCodable>::append_to_selector(buffer, offset, is_first) {
                                    Ok(_) => {},
                                    Err(_) => {
                                        return Err(());
                                    }
                                }
                            });
                        }
                        Type::Array(ref arr_ty) => {
                            parse_lines.extend(quote! {
                                let #field_ident = SolidityCodableReflectionRef::<#lifetime_for_ref_ident>::parse(source, head_offset)?;
                            });

                            parse_mut_lines.extend(quote! {
                                // unsafe, but fine for us
                                let local_source = unsafe {
                                    core::slice::from_raw_parts_mut(source.as_mut_ptr(), source.len())
                                };
                                let #field_ident = SolidityCodableReflectionRefMut::<#lifetime_for_ref_ident>::parse_mut(local_source, head_offset)?;
                            });

                            if head_size.is_empty() == false {
                                head_size.extend(quote! { + })
                            }
                            head_size.extend(quote! {
                                <#arr_ty as SolidityCodable>::HEAD_SIZE
                            });

                            if is_dynamic.is_empty() == false {
                                is_dynamic.extend(quote! { | })
                            }
                            is_dynamic.extend(quote! {
                                <#arr_ty as SolidityCodable>::IS_DYNAMIC
                            });

                            format_lines.extend(quote! {
                                <#arr_ty as SolidityCodable>::extend_canonical_selector_encoding(buff, offset)?;
                            });
                            if is_last == false {
                                format_lines.extend(quote! {
                                    let (_, dst) = buff.split_at_mut_checked(*offset).ok_or(())?;
                                    if dst.len() == 0 {
                                        return Err(());
                                    }
                                    dst[0] = b","[0];
                                    *offset += 1;
                                });
                            }

                            selector_codable_lines.extend(quote! {
                                match <#arr_ty as SelectorCodable>::append_to_selector(buffer, offset, is_first) {
                                    Ok(_) => {},
                                    Err(_) => {
                                        return Err(());
                                    }
                                }
                            });
                        }
                        _ => abort_call_site!("only array and path types are allowed"),
                    };

                    initializations.extend(quote! {
                        #field_ident,
                    });
                }
            }
            _ => abort_call_site!("only named fields are allowed"),
        },
        _ => abort_call_site!("only data structs are allowed"),
    }

    let type_params_of_struct =
        format_type_params_from_generics(&generics, lifetime_of_original_decl);

    let where_clause = if let Some(clause) = generics.where_clause.as_ref() {
        quote! {
            #clause
        }
    } else {
        quote! {}
    };

    let reflection_ident = get_reflection_ident(&ident);
    let reflection_mut_ident = get_reflection_mut_ident(&ident);

    let type_def = if type_params_of_struct.is_empty() {
        quote! {
            #ident
        }
    } else {
        quote! {
            #ident<#type_params_of_struct>
        }
    };

    let type_params_for_ref_impl = if num_lifetimes == 0 {
        quote! {
            #lifetime_param_for_ref, #type_params_of_struct
        }
    } else {
        type_params_of_struct
    };

    let expanded = quote! {
        impl #generics SelectorCodable for #type_def #where_clause {
            const CANONICAL_IDENT: &'static str = "";

            fn append_to_selector(buffer: &mut [u8], offset: &mut usize, is_first: &mut bool) -> Result<(), ()> {
                #selector_codable_lines

                Ok(())
            }
        }

        #[derive(Clone, Copy)]
        #reflection_def

        #reflection_def_mut

        impl #generics SolidityCodable for #type_def #where_clause {
            type ReflectionRef<'__derived_ref> = #reflection_ident<'__derived_ref> where Self: '__derived_ref;
            type ReflectionRefMut<'__derived_ref> = #reflection_mut_ident<'__derived_ref> where Self: '__derived_ref;

            const HEAD_SIZE: usize = #head_size;
            const IS_DYNAMIC: bool = #is_dynamic;

            // fn extend_canonical_selector_encoding(buff: &mut [u8], offset: &mut usize) -> Result<(), ()> {
            //     #format_lines

            //     Ok(())
            // }
        }

        impl #new_generics #ref_trait<#lifetime_param_for_ref> for #reflection_ident <#type_params_for_ref_impl> {
            fn parse(source: & #lifetime_param_for_ref [u8], head_offset: &mut usize) -> Result<Self, ()> {
                #parse_lines

                let new = Self {
                    #initializations
                };

                Ok(new)
            }
        }

        impl #new_generics #ref_mut_trait<#lifetime_param_for_ref> for #reflection_mut_ident <#type_params_for_ref_impl> {
            fn parse_mut(source: & #lifetime_param_for_ref mut [u8], head_offset: &mut usize) -> Result<Self, ()> {
                #parse_mut_lines

                let new = Self {
                    #initializations
                };

                Ok(new)
            }
        }
    };

    proc_macro::TokenStream::from(expanded)
}

// we assume that every type implements a trait
pub(crate) fn get_equivalent_type_recursive(original_ty: &Type, lifetime: &Lifetime) -> Type {
    match original_ty {
        Type::Array(ty) => {
            let ts = quote! {
                <#ty as SolidityCodable>::ReflectionRef<#lifetime>
            };
            let ts = proc_macro::TokenStream::from(ts);

            Type::Path(syn::parse::<TypePath>(ts).unwrap())
        }
        Type::Path(ty) => {
            let ts = quote! {
                <#ty as SolidityCodable>::ReflectionRef<#lifetime>
            };
            let ts = proc_macro::TokenStream::from(ts);
            Type::Path(syn::parse::<TypePath>(ts).unwrap())
        }
        _ => abort_call_site!("only array and path types are allowed"),
    }
}

// we assume that every type implements a trait
pub(crate) fn get_equivalent_mut_type_recursive(original_ty: &Type, lifetime: &Lifetime) -> Type {
    match original_ty {
        Type::Array(ty) => {
            let ts = quote! {
                <#ty as SolidityCodable>::ReflectionRefMut<#lifetime>
            };
            let ts = proc_macro::TokenStream::from(ts);

            Type::Path(syn::parse::<TypePath>(ts).unwrap())
        }
        Type::Path(ty) => {
            let ts = quote! {
                <#ty as SolidityCodable>::ReflectionRefMut<#lifetime>
            };
            let ts = proc_macro::TokenStream::from(ts);
            Type::Path(syn::parse::<TypePath>(ts).unwrap())
        }
        _ => abort_call_site!("only array and path types are allowed"),
    }
}
