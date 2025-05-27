pub trait SelectorCodable {
    const CANONICAL_IDENT: &'static str;

    fn append_to_selector(
        buffer: &mut [u8],
        offset: &mut usize,
        is_first: &mut bool,
    ) -> Result<(), ()> {
        assert!(Self::CANONICAL_IDENT.is_ascii());

        if *is_first == false {
            match append_ascii_str(buffer, offset, ",") {
                Ok(_) => {}
                Err(_) => {
                    return Err(());
                }
            }
        } else {
            *is_first = false;
        }
        match append_ascii_str(buffer, offset, Self::CANONICAL_IDENT) {
            Ok(_) => {}
            Err(_) => {
                return Err(());
            }
        }

        Ok(())
    }
}

macro_rules! impl_selector_codable_for_tuple {
    ($($t:ident),*) => {
        #[allow(non_snake_case, unused_variables)]
        impl<$($t),*> SelectorCodable for ($($t,)*)
        where
            $($t: SelectorCodable,)*
        {
            const CANONICAL_IDENT: &'static str = "";

            fn append_to_selector(buffer: &mut [u8], offset: &mut usize, is_first: &mut bool) -> Result<(), ()> {
                $(match $t::append_to_selector(buffer, offset, is_first) {
                    Ok(_) => {},
                    Err(_) => {
                        return Err(());
                    }
                };)*

                Ok(())
            }
        }
    };
}

impl_selector_codable_for_tuple! {}
impl_selector_codable_for_tuple! { A }
impl_selector_codable_for_tuple! { A, B }
impl_selector_codable_for_tuple! { A, B, C }
impl_selector_codable_for_tuple! { A, B, C, D }
impl_selector_codable_for_tuple! { A, B, C, D, E }
impl_selector_codable_for_tuple! { A, B, C, D, E, F }
impl_selector_codable_for_tuple! { A, B, C, D, E, F, G }
impl_selector_codable_for_tuple! { A, B, C, D, E, F, G, H }
impl_selector_codable_for_tuple! { A, B, C, D, E, F, G, H, I }
impl_selector_codable_for_tuple! { A, B, C, D, E, F, G, H, I, J }
impl_selector_codable_for_tuple! { A, B, C, D, E, F, G, H, I, J, K }
impl_selector_codable_for_tuple! { A, B, C, D, E, F, G, H, I, J, K, L }
impl_selector_codable_for_tuple! { A, B, C, D, E, F, G, H, I, J, K, L, M }
impl_selector_codable_for_tuple! { A, B, C, D, E, F, G, H, I, J, K, L, M, N }
impl_selector_codable_for_tuple! { A, B, C, D, E, F, G, H, I, J, K, L, M, N, O }
impl_selector_codable_for_tuple! { A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P }

pub(crate) const fn append_ascii_str(
    buffer: &mut [u8],
    offset: &mut usize,
    string: &str,
) -> Result<(), ()> {
    assert!(string.is_ascii());
    if buffer.len() < *offset {
        return Err(());
    }
    let to_append = string.len();
    if buffer.len() < *offset + to_append {
        return Err(());
    }
    let mut i = 0;
    let src = string.as_bytes();
    while i < to_append {
        buffer[*offset] = src[i];
        *offset += 1;
        i += 1;
    }
    // unsafe {
    //     core::ptr::copy_nonoverlapping(
    //         string.as_bytes().as_ptr(),
    //         buffer.as_mut_ptr().add(*offset),
    //         to_append)
    // }
    // *offset += to_append;

    Ok(())
}

pub trait SolidityCodable: Sized {
    type ReflectionRef<'a>: SolidityCodableReflectionRef<'a>
    where
        Self: 'a;

    type ReflectionRefMut<'a>: SolidityCodableReflectionRefMut<'a>
    where
        Self: 'a;

    const HEAD_SIZE: usize = 32;
    const IS_DYNAMIC: bool = false;
}

pub trait SolidityDecodable: SolidityCodable {}

impl<T: SolidityCodable> SolidityDecodable for T {}

pub trait SolidityCodableReflectionRef<'a>: Sized {
    fn parse(source: &'a [u8], head_offset: &mut usize) -> Result<Self, ()>;
}

pub trait SolidityCodableReflectionRefMut<'a>: Sized {
    fn parse_mut(source: &'a mut [u8], head_offset: &mut usize) -> Result<Self, ()>;
}

pub trait SolidityCodableReflectionRefReadable<'a> {
    type Value;
    fn read(&'a self) -> Result<Self::Value, ()>;
}

pub trait SolidityCodableReflectionRefWritable<'a>:
    SolidityCodableReflectionRefReadable<'a>
{
    fn write(
        &'a mut self,
        value: &'_ <Self as SolidityCodableReflectionRefReadable<'a>>::Value,
    ) -> Result<(), ()>;
}

pub trait SolidityCodableReflectionRefAddressable<'a> {
    type Value;
    fn get(&'a self, index: usize) -> Result<Self::Value, ()>;
    fn len(&self) -> usize;
}

pub trait SolidityCodableReflectionRefAddressableMut<'a>:
    SolidityCodableReflectionRefAddressable<'a>
{
    type ValueMut;
    fn get_mut(&'a mut self, index: usize) -> Result<Self::ValueMut, ()>;
}
