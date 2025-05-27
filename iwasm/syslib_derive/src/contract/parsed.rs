use crate::{abi::FnInfo, qol::PipeOp};
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse2, ImplItem, ItemImpl};

use super::abi::ImplItemFnInfo;

pub(crate) trait Contract {
    #[allow(dead_code)]
    fn name(&self) -> &str;

    fn collect_fns(&self) -> Result<Vec<Box<dyn FnInfo + '_>>, syn::Error>;
}

/// Contract sourced from `impl` block
pub(crate) struct ContractImpl {
    ast: ItemImpl,
}

impl ContractImpl {
    pub(crate) fn from_tokens(item: TokenStream) -> Self {
        let r = Self {
            ast: parse2::<ItemImpl>(item).expect("Couldn't parse `impl` block."),
        };

        if crate::DEV {
            println!("Parsed tree:\n{:#?}", r.ast);
        }

        r
    }

    pub(crate) fn emit_impl(&mut self) -> proc_macro2::TokenStream {
        self.ast
            .items
            .iter_mut()
            .filter_map(|x| {
                if let ImplItem::Fn(f) = x {
                    Some(f)
                } else {
                    None
                }
            })
            .flat_map(|x| x.sig.inputs.iter_mut())
            .for_each(|param| {
                if let syn::FnArg::Typed(t) = param {
                    let orig = &t.ty;
                    let new = quote! { syslib::abi::overlay::Cdr<#orig> };
                    t.ty = syn::parse2(new).unwrap();
                }
            });

        self.ast.to_token_stream()
    }
}

impl Contract for ContractImpl {
    fn name(&self) -> &str {
        todo!()
    }

    fn collect_fns(&self) -> Result<Vec<Box<dyn FnInfo + '_>>, syn::Error> {
        let mut r: Vec<Box<dyn FnInfo>> = Vec::with_capacity(self.ast.items.len());

        for item in &self.ast.items {
            if let ImplItem::Fn(f) = item {
                let nfo = f.to(ImplItemFnInfo::new)?.to(Box::new);
                r.push(nfo);
            }
        }

        Ok(r)
    }
}
