use proc_macro2::Ident;

#[allow(dead_code)]
pub(crate) trait HasAttr {
    fn has_attr(&self, attr: &Ident) -> bool;
}

impl HasAttr for syn::ItemFn {
    fn has_attr(&self, attr: &Ident) -> bool {
        for a in self.attrs.iter() {
            match &a.meta {
                syn::Meta::Path(x) => {
                    if x.is_ident(attr) {
                        return true;
                    }
                }
                syn::Meta::List(l) => {
                    if l.path.is_ident(attr) {
                        return true;
                    }
                }
                syn::Meta::NameValue(n) => {
                    if n.path.is_ident(attr) {
                        return true;
                    }
                }
            }
        }

        false
    }
}

pub(crate) mod contract {
    use proc_macro2::TokenStream;

    pub enum DecoratedElem {
        Impl,
    }

    pub(crate) fn decorated_elem(item: TokenStream) -> DecoratedElem {
        let elem = item.into_iter().next().expect("Unexpected EOF.");

        match elem {
            proc_macro2::TokenTree::Ident(x) if x.to_string().as_str() == "impl" => {
                DecoratedElem::Impl
            }
            _ => panic!("Unsupported decorated element."),
        }
    }
}
