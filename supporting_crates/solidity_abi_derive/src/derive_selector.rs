use proc_macro2::{Literal, Span};
use proc_macro_error2::abort_call_site;
use quote::quote;
use syn::{
    parse_macro_input, token::Paren, AngleBracketedGenericArguments, FnArg, GenericArgument, Ident,
    ImplItemFn, PathArguments, Signature, Type, TypePath, TypeReference, TypeTuple,
};

pub(crate) fn derive(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let item_clone = item.clone();
    let derived_input = parse_macro_input!(item_clone as ImplItemFn);

    let ImplItemFn { sig, .. } = derived_input;

    let Signature {
        constness,
        asyncness,
        unsafety,
        abi,
        fn_token: _,
        ident,
        generics,
        paren_token: _,
        inputs,
        variadic,
        output: _,
    } = sig;

    if constness.is_some() {
        abort_call_site!("const functions are not supported");
    }

    if asyncness.is_some() {
        abort_call_site!("async functions are not supported");
    }

    if unsafety.is_some() {
        abort_call_site!("unsafe functions are not supported");
    }

    if variadic.is_some() {
        abort_call_site!("variadic functions are not supported");
    }

    if abi.is_some() {
        abort_call_site!("functions with special ABIs are not supported");
    }

    let num_generics = generics.type_params().count();
    let num_const_generics = generics.const_params().count();
    if num_generics != 0 || num_const_generics != 0 {
        abort_call_site!(
            "functions with generic parameters other than single lifetime are not supported"
        );
    }

    #[allow(clippy::len_zero)]
    if inputs.len() == 0 {
        abort_call_site!("function must have at least one argument to derive ABI, fallback function is not supported and doesn't have an ABI");
    }

    let const_identifier = format!("{}_SOLIDITY_ABI_SELECTOR", ident.to_string().to_uppercase());
    let const_ident = Ident::new_raw(&const_identifier, Span::call_site());
    let fn_literal = Literal::string(&ident.to_string());

    let mut result = quote! {
            let mut buffer = [0u8; 2048];
            let mut offset = 0;
            let mut is_first = true;
            match append_ascii_str(&mut buffer, &mut offset, & #fn_literal) {
                Err(_) => panic!("Failed to compute the ABI"),
                Ok(_) => {}
            };
            match append_ascii_str(&mut buffer, &mut offset, "(") {
                Err(_) => panic!("Failed to compute the ABI"),
                Ok(_) => {}
            }

    };

    fn append_to_selector(typed: Type) -> proc_macro2::TokenStream {
        let typed_clone = typed.clone();
        let mut result = proc_macro2::TokenStream::new();

        match typed {
            Type::Path(TypePath { path, .. }) => {
                let last = path.segments.last().unwrap().to_owned();
                match last.ident.to_string().as_str() {
                    "Array" => {
                        if let PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                            args,
                            ..
                        }) = last.arguments
                        {
                            let inner_type = args
                                .iter()
                                .filter_map(|arg| match arg {
                                    GenericArgument::Type(inner_type) => Some(inner_type),
                                    _ => None,
                                })
                                .next()
                                .unwrap();

                            result.extend(append_to_selector(inner_type.clone()));

                            let size = args
                                .iter()
                                .filter_map(|arg| match arg {
                                    GenericArgument::Const(size) => Some(size),
                                    _ => None,
                                })
                                .next()
                                .unwrap();

                            result.extend(quote! {
                                match format_short_integer(&mut buffer, &mut offset, #size) {
                                    Err(_) => panic!("Failed to compute ABI"),
                                    Ok(_) => {}
                                }
                            });
                        };
                    }
                    "Slice" | "Vec" => {
                        if let PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                            args,
                            ..
                        }) = last.arguments
                        {
                            let type_inner = args
                                .iter()
                                .filter_map(|arg| match arg {
                                    GenericArgument::Type(typed) => Some(typed.clone()),
                                    _ => None,
                                })
                                .next()
                                .unwrap();
                            result.extend(append_to_selector(type_inner));

                            result.extend(quote! {
                                match append_ascii_str(&mut buffer, &mut offset, <#typed_clone as SelectorCodable>::CANONICAL_IDENT) {
                                    Err(_) => panic!("Failed to compute ABI"),
                                    Ok(_) => {}
                                }
                            })
                        };
                    }
                    _ => {
                        result.extend(quote! {
                            if !is_first {
                                match append_ascii_str(&mut buffer, &mut offset, ",") {
                                    Err(_) => panic!("Failed to compute the ABI"),
                                    Ok(_) => {}
                                }
                            } else {
                                is_first = false;
                            }
                        });
                        result.extend(quote! {
                            match append_ascii_str(&mut buffer, &mut offset, <#typed_clone as SelectorCodable>::CANONICAL_IDENT) {
                                Err(_) => panic!("Failed to compute the ABI"),
                                Ok(_) => {}
                            }
                        });
                    }
                }
            }
            Type::Reference(TypeReference { elem, .. }) => result.extend(append_to_selector(*elem)),
            Type::Tuple(TypeTuple { elems, .. }) => {
                for elem in elems {
                    result.extend(append_to_selector(elem));
                }
            }
            _ => panic!("Unsupported parameter type"),
        };

        result
    }

    let inputs = Type::Tuple(TypeTuple {
        paren_token: Paren(Span::call_site()),
        elems: inputs
            .iter()
            .filter_map(|src| match src {
                FnArg::Receiver(_) => None,
                FnArg::Typed(pat_type) => Some(*pat_type.clone().ty),
            })
            .collect(),
    });

    result.extend(append_to_selector(inputs));

    result.extend(quote! {
        match append_ascii_str(&mut buffer, &mut offset, ")") {
            Err(_) => panic!("Failed to compute the ABI"),
            Ok(_) => {}
        }

        use const_keccak256::keccak256_digest;
        assert!(offset < buffer.len());

        let digest_input = unsafe { core::slice::from_raw_parts(buffer.as_ptr(), offset)};
        let output = keccak256_digest(digest_input);
        let selector = [output[0], output[1], output[2], output[3]];

        u32::from_be_bytes(selector)
    });

    quote! {
        pub const #const_ident: u32 = const { #result };
    }
    .into()
}
