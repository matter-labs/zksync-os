use crate::{
    abi::overlay::{overlaid_calldata::OverlaidCalldata, Cdr},
    types::{
        ints::{U256, U256BE},
        uintx::IntX,
    },
};

#[test]
fn overlay_u32() {
    let mut calldata = instanticate_calldata(&[U256::from_hex(
        "0000000000000000000000000000000000000000000000000000000000000abc",
    )]);

    let overlay: Cdr<u32> = unsafe { Cdr::new(calldata.as_mut_ptr(), 0) };

    let result = *overlay;

    assert_eq!(0xabc, result);
}

#[test]
fn overlay_str() {
    let mut calldata = instanticate_calldata(&[
        U256::from_hex("0000000000000000000000000000000000000000000000000000000000000020"),
        U256::from_hex("0000000000000000000000000000000000000000000000000000000000000003"),
        U256::from_hex("6162630000000000000000000000000000000000000000000000000000000000"),
    ]);

    let overlay: Cdr<&str> = unsafe { Cdr::new_with_offset(calldata.as_mut_ptr(), 0, 0) };

    let result = overlay;

    assert_eq!("abc", *result);
}

#[test]
fn overlay_intx() {
    let mut calldata = instanticate_calldata(&[U256::from_hex(
        "0000000000000000000000000000000000000000000000000000000000000abc",
    )]);

    let overlay: Cdr<(&IntX<20, crate::types::uintx::LE>,)> =
        unsafe { Cdr::new(calldata.as_mut_ptr(), 0) };

    let result = overlay.0.clone();

    assert_eq!(
        IntX::<20, crate::types::uintx::LE>::from_usize(0xabc),
        **result
    );
}

#[test]
fn overlay_tuple() {
    let mut calldata = instanticate_calldata(&[
        U256::from_hex("000000000000000000000000000000000000000000000000000000000000000a"),
        U256::from_hex("0000000000000000000000000000000000000000000000000000000000000040"),
        U256::from_hex("0000000000000000000000000000000000000000000000000000000000000003"),
        U256::from_hex("6162630000000000000000000000000000000000000000000000000000000000"),
    ]);

    let cd_ptr = calldata.as_mut_ptr();

    // Using new here, because there's no redirection for root value - the first slot points to the
    // data itself.
    let overlay: Cdr<(u32, &str)> = unsafe { Cdr::new(cd_ptr, 0) };

    let (a, b) = *overlay;

    assert_eq!(0xa, *a);
    assert_eq!("abc", *b);

    let x = overlay.as_deref();

    assert_eq!(0xa, x.0);
    assert_eq!("abc", x.1);
}

#[test]
fn overlay_tuple_nested() {
    let mut calldata = instanticate_calldata(&[
        U256::from_hex("0000000000000000000000000000000000000000000000000000000000000040"),
        U256::from_hex("0000000000000000000000000000000000000000000000000000000000000080"),
        U256::from_hex("0000000000000000000000000000000000000000000000000000000000000003"),
        U256::from_hex("6162630000000000000000000000000000000000000000000000000000000000"),
        U256::from_hex("0000000000000000000000000000000000000000000000000000000000000010"),
        U256::from_hex("0000000000000000000000000000000000000000000000000000000000000200"),
    ]);

    let overlay: Cdr<(&str, (u32, u32))> = unsafe { Cdr::new(calldata.as_mut_ptr(), 0) };

    let (s, t2) = &*overlay;

    let s = *s;

    let (u1, u2) = **t2;

    let u1 = *u1;
    let u2 = *u2;

    assert_eq!("abc", *s);
    assert_eq!(0x10, u1);
    assert_eq!(0x200, u2);
}

#[test]
fn overlay_struct() {
    use code_gen::*;

    mod code_gen {
        use crate::{
            abi::{
                overlay::{Cdr, UnsafeUninit},
                Overlaid,
            },
            qol::UnsafeCellEx,
        };

        // Everything here, except `Container`, is what the macro should generate.

        pub struct Container<'a> {
            pub int: u32,
            pub string: &'a str,
        }

        pub struct ContainerReflection<'a> {
            pub int: Cdr<u32>,
            pub string: Cdr<&'a str>,
        }

        impl<'a> Overlaid for Container<'a> {
            const IS_INDIRECT: bool = true;

            type Reflection = UnsafeUninit<ContainerReflection<'a>>;
            type Deref = ContainerReflection<'a>;

            fn to_deref(cdr: &Cdr<Self>) -> &Self::Deref {
                let r = unsafe {
                    ContainerReflection {
                        int: Cdr::new_with_offset(cdr.data.base_ptr, cdr.data.ix + 0, cdr.data.ix),
                        string: Cdr::new_with_offset(
                            cdr.data.base_ptr,
                            cdr.data.ix + 1,
                            cdr.data.ix,
                        ),
                    }
                };

                unsafe { cdr.reflection.inner.u_deref_mut().write(r) }
            }

            fn decode(cdr: &Cdr<Self>) -> Self {
                let r = Self::to_deref(cdr);

                let r = Container {
                    int: r.int.decode(),
                    string: r.string.decode(),
                };

                r
            }

            fn reflection_uninit() -> Self::Reflection {
                Self::Reflection::default()
            }
        }
    }

    let mut calldata = instanticate_calldata(&[
        U256::from_hex("000000000000000000000000000000000000000000000000000000000000abcd"),
        U256::from_hex("0000000000000000000000000000000000000000000000000000000000000040"),
        U256::from_hex("0000000000000000000000000000000000000000000000000000000000000003"),
        U256::from_hex("6162630000000000000000000000000000000000000000000000000000000000"),
    ]);

    let overlay: Cdr<Container> = unsafe { Cdr::new(calldata.as_mut_ptr(), 0) };

    assert_eq!(0xabcd, *overlay.int);
    assert_eq!("abc", *overlay.string);

    let c = overlay.decode();

    assert_eq!(0xabcd, c.int);
    assert_eq!("abc", c.string);
}

fn instanticate_calldata<const N: usize>(input: &[U256; N]) -> OverlaidCalldata
where
    [(); 32 * N]:,
{
    let oc = unsafe {
        OverlaidCalldata::new(32 * N, |data| {
            let ptr_range = data.as_mut_ptr_range();

            let mut ptr = ptr_range.start as *mut U256BE;
            let ptr_e = ptr_range.end;

            for i in input {
                assert!(ptr as *mut _ < ptr_e);
                ptr.write((i).to_be());
                ptr = ptr.add(1);
            }
        })
    };

    oc.unwrap()
}
