use ruint::aliases::*;

use super::*;

macro_rules! uint_impl {
    ($ident:ident, $ref_ident:ident, $ref_ident_mut:ident, ($bytes:expr, $canonical_ident:expr)) => {
        impl SelectorCodable for $ident {
            const CANONICAL_IDENT: &'static str = $canonical_ident;
        }

        impl SolidityCodable for $ident {
            type ReflectionRef<'a> = $ref_ident<'a>;
            type ReflectionRefMut<'a> = $ref_ident_mut<'a>;
        }

        #[derive(Clone, Copy)]
        pub struct $ref_ident<'a> {
            source: &'a [u8; 32],
        }

        pub struct $ref_ident_mut<'a> {
            source: &'a mut [u8; 32],
        }

        impl<'a> SolidityCodableReflectionRef<'a> for $ref_ident<'a> {
            fn parse(source: &'a [u8], head_offset: &mut usize) -> Result<Self, ()> {
                let (_, local_head) = source.split_at_checked(*head_offset).ok_or(())?;
                if local_head.len() < 32 {
                    return Err(());
                }
                let source = local_head.as_chunks::<32>().0.iter().next().unwrap();
                let new = Self { source };
                *head_offset += 32;

                Ok(new)
            }
        }

        impl<'a> SolidityCodableReflectionRefMut<'a> for $ref_ident_mut<'a> {
            fn parse_mut(source: &'a mut [u8], head_offset: &mut usize) -> Result<Self, ()> {
                let (_, local_head) = source.split_at_mut_checked(*head_offset).ok_or(())?;
                if local_head.len() < 32 {
                    return Err(());
                }
                let source = local_head
                    .as_chunks_mut::<32>()
                    .0
                    .iter_mut()
                    .next()
                    .unwrap();
                let new = Self { source };
                *head_offset += 32;

                Ok(new)
            }
        }

        impl<'a> SolidityCodableReflectionRefReadable<'a> for $ref_ident<'a> {
            type Value = $ident;

            fn read(&self) -> Result<Self::Value, ()> {
                let value = U256::from_be_bytes(*self.source);
                // we can not ignore top bits
                if value.leading_zeros() < (32 - $bytes) * 8 {
                    return Err(());
                }
                let mut result = $ident::ZERO;
                let num_limbs = result.as_limbs().len();
                unsafe {
                    result
                        .as_limbs_mut()
                        .copy_from_slice(&value.as_limbs()[..num_limbs]);
                }

                Ok(result)
            }
        }

        impl<'a> SolidityCodableReflectionRefReadable<'a> for $ref_ident_mut<'a> {
            type Value = $ident;

            fn read(&self) -> Result<Self::Value, ()> {
                let value = U256::from_be_bytes(*self.source);
                // we can not ignore top bits
                if value.leading_zeros() < (32 - $bytes) * 8 {
                    return Err(());
                }
                let mut result = $ident::ZERO;
                let num_limbs = result.as_limbs().len();
                unsafe {
                    result
                        .as_limbs_mut()
                        .copy_from_slice(&value.as_limbs()[..num_limbs]);
                }

                Ok(result)
            }
        }

        impl<'a> SolidityCodableReflectionRefWritable<'a> for $ref_ident_mut<'a> {
            fn write(
                &mut self,
                value: &'_ <Self as SolidityCodableReflectionRefReadable<'a>>::Value,
            ) -> Result<(), ()> {
                let bytes = value.to_be_bytes::<$bytes>();
                self.source[(32 - $bytes)..].copy_from_slice(&bytes);

                Ok(())
            }
        }
    };
}

uint_impl!(U256, U256Ref, U256RefMut, (32, "uint256"));
uint_impl!(U160, U160Ref, U160RefMut, (20, "uint160"));
uint_impl!(U128, U128Ref, U128RefMut, (16, "uint128"));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Address(pub U160);

impl SelectorCodable for Address {
    const CANONICAL_IDENT: &'static str = "address";
}

impl SolidityCodable for Address {
    type ReflectionRef<'a> = AddressRef<'a>;
    type ReflectionRefMut<'a> = AddressRefMut<'a>;
}

#[derive(Clone, Copy)]
pub struct AddressRef<'a> {
    source: &'a [u8; 32],
}

pub struct AddressRefMut<'a> {
    source: &'a mut [u8; 32],
}

impl<'a> SolidityCodableReflectionRef<'a> for AddressRef<'a> {
    fn parse(source: &'a [u8], head_offset: &mut usize) -> Result<Self, ()> {
        let (_, local_head) = source.split_at_checked(*head_offset).ok_or(())?;
        if local_head.len() < 32 {
            return Err(());
        }
        let source = local_head.as_chunks::<32>().0.iter().next().unwrap();
        let new = Self { source };
        *head_offset += 32;

        Ok(new)
    }
}

impl<'a> SolidityCodableReflectionRefMut<'a> for AddressRefMut<'a> {
    fn parse_mut(source: &'a mut [u8], head_offset: &mut usize) -> Result<Self, ()> {
        let (_, local_head) = source.split_at_mut_checked(*head_offset).ok_or(())?;
        if local_head.len() < 32 {
            return Err(());
        }
        let source = local_head
            .as_chunks_mut::<32>()
            .0
            .iter_mut()
            .next()
            .unwrap();
        let new = Self { source };
        *head_offset += 32;

        Ok(new)
    }
}

impl<'a> SolidityCodableReflectionRefReadable<'a> for AddressRef<'a> {
    type Value = Address;

    fn read(&self) -> Result<Self::Value, ()> {
        let value = U256::from_be_bytes(*self.source);
        // we can not ignore top bits
        if value.leading_zeros() < (32 - 20) * 8 {
            return Err(());
        }
        let mut result = U160::ZERO;
        let num_limbs = result.as_limbs().len();
        unsafe {
            result
                .as_limbs_mut()
                .copy_from_slice(&value.as_limbs()[..num_limbs]);
        }

        Ok(Address(result))
    }
}

impl<'a> SolidityCodableReflectionRefReadable<'a> for AddressRefMut<'a> {
    type Value = Address;

    fn read(&self) -> Result<Self::Value, ()> {
        let value = U256::from_be_bytes(*self.source);
        // we can not ignore top bits
        if value.leading_zeros() < (32 - 20) * 8 {
            return Err(());
        }
        let mut result = U160::ZERO;
        let num_limbs = result.as_limbs().len();
        unsafe {
            result
                .as_limbs_mut()
                .copy_from_slice(&value.as_limbs()[..num_limbs]);
        }

        Ok(Address(result))
    }
}

impl<'a> SolidityCodableReflectionRefWritable<'a> for AddressRefMut<'a> {
    fn write(
        &mut self,
        value: &'_ <Self as SolidityCodableReflectionRefReadable<'a>>::Value,
    ) -> Result<(), ()> {
        let bytes = value.0.to_be_bytes::<20>();
        self.source[12..].copy_from_slice(&bytes);

        Ok(())
    }
}

// impl SolidityCodable for U256 {
//     type ReflectionRef<'a> = U256Ref<'a>;

//     fn extend_canonical_selector_encoding(buff: &mut [u8], offset: &mut usize) -> Result<(), ()> {
//         const CANONICAL_IDENT: &[u8] = b"uint256";

//         let (_, dst) = buff.split_at_mut_checked(*offset).ok_or(())?;
//         if dst.len() < CANONICAL_IDENT.len() {
//             return Err(());
//         }
//         dst[..CANONICAL_IDENT.len()].copy_from_slice(CANONICAL_IDENT);
//         *offset += CANONICAL_IDENT.len();

//         Ok(())
//     }
// }

// #[derive(Clone, Copy)]
// pub struct U256Ref<'a> {
//     source: &'a [u8; 32],
// }

// impl<'a> SolidityCodableReflectionRef<'a> for U256Ref<'a> {
//     fn parse(source: &'a [u8], head_offset: &mut usize) -> Result<Self, ()> {
//         let (_, local_head) = source.split_at_checked(*head_offset).ok_or(())?;
//         if local_head.len() < 32 {
//             return Err(());
//         }
//         let source = local_head.as_chunks::<32>().0.next().unwrap();
//         let new = Self { source };
//         *head_offset += 32;

//         Ok(new)
//     }
// }

// impl<'a> SolidityCodableReflectionRefReadable<'a> for U256Ref<'a> {
//     type DecodedValue = U256;

//     fn read(&self) -> Result<Self::DecodedValue, ()> {
//         let value = U256::from_be_bytes(*self.source);

//         Ok(value)
//     }
// }
