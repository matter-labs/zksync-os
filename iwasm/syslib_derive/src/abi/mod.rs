pub mod overlay;
mod types;

use core::panic;

use sha3::{digest::Update, Digest};
use syn::{spanned::Spanned, FnArg, Signature, Type};

pub(crate) type FnSelector = [u8; 4];

#[allow(unused_imports)]
pub use type_info::*;

mod type_info {
    use quote::ToTokens;
    use syn::{Ident, LitInt};

    pub(crate) trait TypeInfo: ToTokens {
        fn name(&self) -> String;
        fn generic_params(&self) -> &[&dyn TypeInfo];
    }

    pub(crate) trait FnParamInfo {
        #[allow(dead_code)]
        fn name(&self) -> String;
        fn type_info(&self) -> &dyn TypeInfo;
    }

    pub(crate) trait FnInfo {
        fn ident(&self) -> Ident;
        fn name(&self) -> String;

        fn selector(&self) -> LitInt;

        /// The function parameters excluding the receiver.
        fn params(&self) -> Vec<&dyn FnParamInfo>;
    }
}

pub(crate) fn encode(f: &dyn type_info::FnInfo) -> FnSelector {
    if crate::DEV {
        println!("   f: {}", f.name())
    }

    let mut sig = Vec::with_capacity(1024);

    fn append_params(sig: &mut Vec<u8>, params: &[&dyn type_info::FnParamInfo]) {
        for i in params {
            if crate::DEV {
                println!("   - p: {}", i.type_info().name())
            }

            let type_name = types::to_abi_name(i.type_info().name().as_str());
            sig.extend_from_slice(type_name.as_bytes());
            sig.push(b',')
        }

        if !params.is_empty() {
            // Remove the last comma.
            sig.pop();
        }
    }

    sig.extend_from_slice(f.name().as_bytes());
    sig.push(b'(');
    append_params(&mut sig, &f.params());
    sig.push(b')');

    if crate::DEV {
        println!(
            "Hasher input: {:?}",
            core::str::from_utf8(&sig).ok().unwrap()
        );
    }

    let mut hasher = sha3::Keccak256::new();
    Update::update(&mut hasher, sig.as_slice());
    let hash = hasher.finalize();

    hash[0..4].try_into().expect("Converting 32 words to 1")
}

pub fn encode_signature(sig: &Signature) -> [u8; 4] {
    let mut buf = Vec::with_capacity(1024);

    buf.extend_from_slice(sig.ident.to_string().as_bytes());
    buf.extend_from_slice(b"(");

    for i in 0..sig.inputs.len() {
        let arg = &sig.inputs[i];

        match arg {
            FnArg::Receiver(_r) => {
                panic!("Can't be applied to methods.")
            }
            FnArg::Typed(p) => match *p.ty {
                Type::Array(ref _a) => {
                    println!("arg: array")
                }
                Type::BareFn(ref _a) => {
                    println!("arg: BareFn")
                }
                Type::Group(ref _a) => {
                    println!("arg: Group")
                }
                Type::ImplTrait(ref _a) => {
                    println!("arg: ImplTrait")
                }
                Type::Infer(ref _a) => {
                    println!("arg: Infer")
                }
                Type::Macro(ref _a) => {
                    println!("arg: Macro")
                }
                Type::Never(ref _a) => {
                    println!("arg: Never")
                }
                Type::Paren(ref _a) => {
                    println!("arg: Paren")
                }
                Type::Path(ref p) => {
                    if p.qself.is_some() {
                        panic!("Self-type specializations aren't supported.")
                    }

                    if p.path.segments.len() != 1 {
                        panic!("Paths in types aren't yet supported.")
                    }

                    for s in &p.path.segments {
                        println!("arg: Path seg {:?}", s.ident);

                        buf.extend_from_slice(s.ident.to_string().as_bytes());
                    }
                }
                Type::Ptr(ref _a) => {
                    println!("arg: Ptr")
                }
                Type::Reference(ref _a) => {
                    println!("arg: Reference")
                }
                Type::Slice(ref _a) => {
                    println!("arg: Slice")
                }
                Type::TraitObject(ref _a) => {
                    println!("arg: TraitObject")
                }
                Type::Tuple(ref _a) => {
                    println!("arg: Tuple")
                }
                Type::Verbatim(ref s) => {
                    println!("arg: {:?}", s.span().source_text());
                }
                _ => {
                    panic!("arg: unknown type")
                }
            },
        }
    }

    buf.extend_from_slice(b")");

    for c in buf.iter() {
        print!("{}", char::from_u32(*c as u32).unwrap());
    }

    let mut hasher = sha3::Keccak256::new();
    Update::update(&mut hasher, buf.as_slice());

    let hash = hasher.finalize();

    hash[0..4].try_into().expect("Converting 32 words to 1")
}

mod defs {
    #[allow(dead_code)]
    const fn p_type(key: &str) -> &'static str {
        match key.as_bytes() {
            b"u32" => "uint32",
            _ => panic!("Unknown type."),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    #[test]
    /// Canon:
    /// web3.eth.abi.encodeFunctionSignature({ type: "function", name: "name", inputs: [] })
    /// "0x06fdde03"
    fn selector_hash_empty() {
        let input = "pub fn name(&self) { }";

        let ts = proc_macro2::TokenStream::from_str(input).unwrap();

        let ast = syn::parse2::<syn::ImplItemFn>(ts).unwrap();

        let hash = super::encode(&ast);

        assert_eq!([0x06u8, 0xfdu8, 0xdeu8, 0x03u8], hash);

        let hash_proj = unsafe { &*(&hash as *const _ as *const u32) };

        assert_eq!(hash_proj, &0x03defd06);
        assert_eq!(u32::from_le_bytes(hash), *hash_proj);
    }

    #[test]
    /// Canon:
    /// web3.eth.abi.encodeFunctionSignature({ type: "function", name: "name", inputs: [ { type: "uint64", name: "input" } ] })
    /// "0x20fc811e"
    fn selector_hash_u64() {
        let input = "pub fn name(&self, input: u64) { }";

        let ts = proc_macro2::TokenStream::from_str(input).unwrap();

        let ast = syn::parse2::<syn::ImplItemFn>(ts).unwrap();

        let hash = super::encode(&ast);

        assert_eq!(
            u32::from_str_radix("20fc811e", 16).unwrap(),
            u32::from_be_bytes(hash)
        );
    }
}
