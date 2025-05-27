use core::iter::*;
use quote::quote;
use syn::LitInt;

use crate::{
    abi::{FnInfo, FnParamInfo},
    qol::PipeOp,
};

pub(crate) fn emit_entry_call(fn_info: &dyn FnInfo) -> proc_macro2::TokenStream {
    let hash = fn_info.selector();

    let fn_ident = fn_info.ident();

    let overlay_type = emit_overlay_type(&fn_info.params());

    let args = (0..fn_info.params().len())
        .map(|x| quote! { overlay.#x.clone() })
        .to(|x| quote! { #(#x),* });

    let overlay = if fn_info.params().is_empty() {
        quote! {}
    } else {
        quote! { let overlay = unsafe { syslib::abi::overlay::Cdr::<#overlay_type>::new(calldata.as_mut_ptr(), 0) }; }
    };

    let r = quote! {
        #hash => {
            #overlay
            let r = instance.#fn_ident(#args) ;

            r.map(|r| {
                let mut encoder = syslib::abi::Encoder::new(r.encoded_size());
                r.encode(&mut encoder);
                encoder.finalize()
            })
        }
    };

    r
}

fn emit_overlay_type(params: &[&dyn FnParamInfo]) -> proc_macro2::TokenStream {
    let types = params
        .iter()
        .map(|x| match x.type_info().generic_params().is_empty() {
            true => x.type_info(),
            false => panic!("Nested types aren't supported."),
        });

    quote! {
        (#(#types,)*)
    }
}

#[allow(dead_code)]
fn emit_entry_signature() {}
