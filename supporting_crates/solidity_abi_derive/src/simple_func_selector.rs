use quote::quote;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::{parse_macro_input, LitStr, Token};

struct SignatureComputationInput {
    arg1: LitStr,       // Ident parses an identifier
    _comma1: Token![,], // Token![,] parses a comma
    arg2: LitStr,       // LitStr parses a string literal
}

impl Parse for SignatureComputationInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        Ok(SignatureComputationInput {
            arg1: input.parse()?,
            _comma1: input.parse()?,
            arg2: input.parse()?,
        })
    }
}

pub(crate) fn derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as SignatureComputationInput);
    // we expect two strings - one for function name, another for params
    let mut buffer = Vec::new();
    buffer.extend_from_slice(input.arg1.value().as_bytes());
    buffer.extend(b"(");
    buffer.extend_from_slice(input.arg2.value().as_bytes());
    buffer.extend(b")");

    use sha3::Digest;
    let mut hasher = sha3::Keccak256::new();
    hasher.update(&buffer[..]);
    let hash = hasher.finalize();

    let mut result = [0u8; 4];
    #[allow(deprecated)]
    result.copy_from_slice(&hash.as_slice()[..4]);
    let result = u32::from_be_bytes(result);

    quote! {
        #result
    }
    .into()
}
