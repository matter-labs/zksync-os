use crate::{
    abi_decode,
    codable_trait::{SolidityCodable, SolidityCodableReflectionRef},
    impls::codable_slice::Slice,
};

const LONG_DYNAMIC: &str = "
0000000000000000000000000000000000000000000000000000000000000040
0000000000000000000000000000000000000000000000000000000000000140
0000000000000000000000000000000000000000000000000000000000000002
0000000000000000000000000000000000000000000000000000000000000040
00000000000000000000000000000000000000000000000000000000000000a0
0000000000000000000000000000000000000000000000000000000000000002
0000000000000000000000000000000000000000000000000000000000000001
0000000000000000000000000000000000000000000000000000000000000002
0000000000000000000000000000000000000000000000000000000000000001
0000000000000000000000000000000000000000000000000000000000000003
0000000000000000000000000000000000000000000000000000000000000003
0000000000000000000000000000000000000000000000000000000000000060
00000000000000000000000000000000000000000000000000000000000000a0
00000000000000000000000000000000000000000000000000000000000000e0
0000000000000000000000000000000000000000000000000000000000000003
6f6e650000000000000000000000000000000000000000000000000000000000
0000000000000000000000000000000000000000000000000000000000000003
74776f0000000000000000000000000000000000000000000000000000000000
0000000000000000000000000000000000000000000000000000000000000005
7468726565000000000000000000000000000000000000000000000000000000
";

use crate::codable_trait::*;
use crate::impls::byte_ref::Bytes;
use ruint::aliases::U256;

#[allow(dead_code)]
struct DynDyn<'a> {
    double_array: Slice<'a, Slice<'a, U256>>,
    array_of_byte_arrays: Slice<'a, Bytes<'a>>,
}

impl<'a> SolidityCodable for DynDyn<'a> {
    type ReflectionRef<'this> = DynDynRef<'this> where Self: 'this;

    const HEAD_SIZE: usize =
        Slice::<'a, Slice<'a, U256>>::HEAD_SIZE + Slice::<'a, Bytes<'a>>::HEAD_SIZE;
    const IS_DYNAMIC: bool =
        Slice::<'a, Slice<'a, U256>>::IS_DYNAMIC | Slice::<'a, Bytes<'a>>::IS_DYNAMIC;

    fn extend_canonical_selector_encoding(_buff: &mut [u8], _offset: &mut usize) -> Result<(), ()> {
        todo!();
    }
}

struct DynDynRef<'this> {
    double_array: <Slice<'this, Slice<'this, U256>> as SolidityCodable>::ReflectionRef<'this>,
    array_of_byte_arrays: <Slice<'this, Bytes<'this>> as SolidityCodable>::ReflectionRef<'this>,
}

impl<'this> SolidityCodableReflectionRef<'this> for DynDynRef<'this> {
    fn parse(source: &'this [u8], head_offset: &mut usize) -> Result<Self, ()> {
        let double_array = SolidityCodableReflectionRef::<'_>::parse(source, head_offset)?;
        let array_of_byte_arrays = SolidityCodableReflectionRef::<'_>::parse(source, head_offset)?;

        let new = Self {
            double_array,
            array_of_byte_arrays,
        };

        Ok(new)
    }
}

#[test]
fn test_dyn_dyn() -> Result<(), ()> {
    let mut s = LONG_DYNAMIC.to_string();
    s.retain(|c| !c.is_whitespace());
    let input = hex::decode(&s).unwrap();
    let result = abi_decode::<DynDyn<'_>>(&input)?;
    assert_eq!(result.double_array.len(), 2);
    let a = result.double_array.get(0)?.get(0)?.read()?;
    let b = result.double_array.get(0)?.get(1)?.read()?;
    let c = result.double_array.get(1)?.get(0)?.read()?;
    dbg!((a, b, c));

    let _ = result.double_array.get(0)?.get(2).err().unwrap();
    let _ = result.double_array.get(1)?.get(1).err().unwrap();
    let _ = result.double_array.get(2).err().unwrap();

    assert_eq!(result.array_of_byte_arrays.len(), 3);
    let a = result.array_of_byte_arrays.get(0)?.read()?;
    let b = result.array_of_byte_arrays.get(1)?.read()?;
    let c = result.array_of_byte_arrays.get(2)?.read()?;

    let a = core::str::from_utf8(a.0).unwrap();
    let b = core::str::from_utf8(b.0).unwrap();
    let c = core::str::from_utf8(c.0).unwrap();
    dbg!((a, b, c));

    Ok(())
}
