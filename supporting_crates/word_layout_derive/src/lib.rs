use proc_macro::TokenStream;

#[proc_macro_derive(WordLayout)]
pub fn derive_word_layout(input: TokenStream) -> TokenStream {
    TokenStream::new() // stub — implemented in Task 4
}
