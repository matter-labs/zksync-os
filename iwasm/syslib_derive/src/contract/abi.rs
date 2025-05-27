use syn::{spanned::Spanned, FnArg, ImplItemFn, LitInt, Meta, PatType, Path, Type, TypeReference};

use crate::{abi::{FnInfo, FnParamInfo, TypeInfo}, qol::PipeOp};

pub(crate) struct ImplItemFnInfo<'a> {
    item: &'a ImplItemFn,
    selector_override: Option<LitInt>,
}

impl<'a> ImplItemFnInfo<'a> {
    pub fn new(item: &'a ImplItemFn) -> Result<Self, syn::Error> {
        let selector_override = item.attrs.iter().find_map(|x| {
            match x.meta {
                Meta::List(ref x) if x.path.name() == "selector" => { 
                    syn::parse2::<syn::LitInt>(x.tokens.clone())
                        .map_err(|e| syn::Error::new(x.span(), e.to_string()))
                        .to(Some)
                },
                _ => None,
            } 
        })
            .map_or(Ok(None), |x| x.map(|x| Some(x)))?;

        Ok(Self { item, selector_override })
    }
}

impl<'a> FnInfo for ImplItemFnInfo<'a> {
    fn ident(&self) -> syn::Ident {
        self.item.sig.ident.clone()
    }

    fn name(&self) -> String {
        self.item.sig.ident.to_string()
    }

    fn selector(&self) -> LitInt {
        let hash = match &self.selector_override {
            Some(x) => x.clone(),
            None => {
                let hash = u32::from_be_bytes(crate::abi::encode(self));
                let hash = format!("0x{:X}", hash);
                let hash: LitInt = syn::parse_str(hash.as_str()).unwrap();
                hash
            }
        };

        hash
    }

    fn params(&self) -> Vec<&dyn FnParamInfo> {
        let mut r: Vec<&dyn FnParamInfo> = Vec::with_capacity(self.item.sig.inputs.len());

        for i in &self.item.sig.inputs {
            match i {
                FnArg::Receiver(_) => continue,
                FnArg::Typed(t) => r.push(t),
            }
        }

        r
    }
}

impl FnParamInfo for PatType {
    fn name(&self) -> String {
        todo!("FnParamInfo for PatType :: name");
    }

    fn type_info(&self) -> &dyn crate::abi::TypeInfo {
        match *self.ty {
            Type::Path(ref x) => &x.path,
            Type::Reference(ref x) => x,
            _ => panic!("Unsupported type declaration: {:#?}", self.ty),
        }
    }
}

impl TypeInfo for Path {
    fn name(&self) -> String {
        if self.segments.len() == 1 {
            self.segments[0].ident.to_string()
        } else {
            panic!("multisegment name")
        }
    }

    fn generic_params(&self) -> &[&dyn TypeInfo] {
        &[]
    }

    // fn ident(&self) -> syn::buffer::TokenBuffer{
    //     if self.segments.len() == 1 {
    //         todo!()
    //         // self.segments[0].ident.to_token_stream()
    //     } else {
    //         panic!("Complex type paths aren't supported yet.")
    //     }
    // }
}

impl TypeInfo for TypeReference {
    // fn ident(&self) -> syn::buffer::TokenBuffer {
    //     match *self.elem {
    //         Type::Path(ref x) => x.path.ident(),
    //         Type::Reference(ref x) => x.ident(),
    //         _ => panic!("Unsupported type declaration in TypeReference:\n{:#?}", self.elem)
    //     }
    // }

    fn name(&self) -> String {
        match *self.elem {
            Type::Path(ref x) => x.path.name(),
            Type::Reference(ref x) => x.name(),
            _ => panic!(
                "Unsupported type declaration in TypeReference:\n{:#?}",
                self.elem
            ),
        }
    }

    fn generic_params(&self) -> &[&dyn TypeInfo] {
        &[]
    }
}
