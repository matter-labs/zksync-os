use alloc::vec::Vec;
use core::alloc::Allocator;

mod bigint;
mod u256;

use self::bigint::{BigintRepr, OracleAdvisor};

use zk_ee::{system::logger::Logger, system_io_oracle::IOOracle};

pub(super) fn modexp<O: IOOracle, L: Logger, A: Allocator + Clone>(
    base: &[u8],
    exp: &[u8],
    modulus: &[u8],
    oracle: &mut O,
    _logger: &mut L,
    allocator: A,
) -> Vec<u8, A> {
    self::u256::init();

    let mut advisor = OracleAdvisor { inner: oracle };

    let m = BigintRepr::from_big_endian_with_double_capacity(&modulus, allocator.clone());
    let output = if m.digits == 0 {
        Vec::new_in(allocator)
    } else {
        let min_capacity = m.capacity();
        let x = BigintRepr::from_big_endian_with_double_capacity_or_min_capacity(
            &base,
            min_capacity,
            allocator.clone(),
        );
        let x = x.modpow(&exp, m, &mut advisor, allocator.clone());
        let r = x.to_big_endian(allocator);
        r
    };

    output
}

#[cfg(test)]
mod test {
    use std::alloc::Global;

    use super::bigint::naive_advisor::NaiveAdvisor;
    use super::*;

    fn invoke_precompile_no_prepadding(modulus: &[u8], base: &[u8], exp: &[u8]) -> Vec<u8> {
        super::u256::init();

        let mut advisor = NaiveAdvisor;
        let allocator = Global;

        let m = BigintRepr::from_big_endian_with_double_capacity(&modulus, allocator.clone());
        let output = if m.digits == 0 {
            Vec::new_in(allocator)
        } else {
            let min_capacity = m.capacity();
            let x = BigintRepr::from_big_endian_with_double_capacity_or_min_capacity(
                &base,
                min_capacity,
                allocator.clone(),
            );
            let x = x.modpow(&exp, m, &mut advisor, allocator.clone());
            let r = x.to_big_endian(allocator);
            r
        };

        output
    }

    // #[ignore = "depends on init and features"]
    #[test]
    fn test_on_vector() {
        // let test = Test {
        //     input: "\
        //     0000000000000000000000000000000000000000000000000000000000000040\
        //     0000000000000000000000000000000000000000000000000000000000000001\
        //     0000000000000000000000000000000000000000000000000000000000000040\
        //     e09ad9675465c53a109fac66a445c91b292d2bb2c5268addb30cd82f80fcb003\
        //     3ff97c80a5fc6f39193ae969c6ede6710a6b7ac27078a06d90ef1c72e5c85fb5\
        //     02fc9e1f6beb81516545975218075ec2af118cd8798df6e08a147c60fd6095ac\
        //     2bb02c2908cf4dd7c81f11c289e4bce98f3553768f392a80ce22bf5c4f4a248c\
        //     6b",
        //     expected: "60008f1614cc01dcfb6bfb09c625cf90b47d4468db81b5f8b7a39d42f332eab9b2da8f2d95311648a8f243f4bb13cfb3d8f7f2a3c014122ebb3ed41b02783adc",
        //     name: "nagydani_1_square",
        //     precompile_id: "0000000000000000000000000000000000000005",
        // };

        let base = hex::decode("e09ad9675465c53a109fac66a445c91b292d2bb2c5268addb30cd82f80fcb0033ff97c80a5fc6f39193ae969c6ede6710a6b7ac27078a06d90ef1c72e5c85fb5").unwrap();
        assert_eq!(base.len(), 64);

        let exp = hex::decode("02").unwrap();
        assert_eq!(exp.len(), 1);

        let modulus = hex::decode("fc9e1f6beb81516545975218075ec2af118cd8798df6e08a147c60fd6095ac2bb02c2908cf4dd7c81f11c289e4bce98f3553768f392a80ce22bf5c4f4a248c6b").unwrap();
        assert_eq!(modulus.len(), 64);

        let expected = hex::decode("60008f1614cc01dcfb6bfb09c625cf90b47d4468db81b5f8b7a39d42f332eab9b2da8f2d95311648a8f243f4bb13cfb3d8f7f2a3c014122ebb3ed41b02783adc").unwrap();
        assert_eq!(expected.len(), 64);

        let output = invoke_precompile_no_prepadding(&modulus, &base, &exp);

        assert_eq!(&output, &expected);
    }

    #[test]
    fn test_zero_output() {
        let base = hex::decode("5442ddc2b70f66c1f6d2b296c0a875be7eddd0a80958cbc7425f1899ccf90511a5c318226e48ee23f130b44dc17a691ce66be5da18b85ed7943535b205aa125e9f59294a00f05155c23e97dac6b3a00b0c63c8411bf815fc183b420b4d9dc5f715040d5c").unwrap();
        assert_eq!(base.len(), 0x64);

        let exp = hex::decode("60957f52d334b843197adec58c131c907cd96059fc5adce9dda351b5df3d666fcf3eb63c46851c1816e323f2119ebdf5ef35").unwrap();
        assert!(exp.len() < 0x64);
        let mut exp_prepadded = vec![0u8; 0x64 - exp.len()];
        exp_prepadded.extend(exp);
        assert_eq!(exp_prepadded.len(), 0x64);

        let modulus = vec![0u8; 100];

        let output = invoke_precompile_no_prepadding(&modulus, &base, &exp_prepadded);

        assert!(output.is_empty());
    }

    #[test]
    fn test_3() {
        // Test {
        //     input: "\
        //     0000000000000000000000000000000000000000000000000000000000000001\
        //     0000000000000000000000000000000000000000000000000000000000000020\
        //     0000000000000000000000000000000000000000000000000000000000000020\
        //     03\
        //     fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2e\
        //     ffffffffffffffffffffffffffffffffffffffffff2f",
        //     expected: "162ead82cadefaeaf6e9283248fdf2f2845f6396f6f17c4d5a39f820b6f6b5f9",
        //     name: "eth_tests_create2callPrecompiles_test0_berlin",
        //     precompile_id: "0000000000000000000000000000000000000005",
        // }

        let base = hex::decode("03").unwrap();
        assert_eq!(base.len(), 1);

        let exp = hex::decode("fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2e")
            .unwrap();
        assert_eq!(exp.len(), 32);

        let encoding = "ffffffffffffffffffffffffffffffffffffffffff2f";
        let mut modulus = hex::decode(encoding).unwrap();
        modulus.resize(32, 0u8);

        let output = invoke_precompile_no_prepadding(&modulus, &base, &exp);

        let expected =
            hex::decode("162ead82cadefaeaf6e9283248fdf2f2845f6396f6f17c4d5a39f820b6f6b5f9")
                .unwrap();

        assert_eq!(output, expected);
    }

    #[test]
    fn test_4() {
        // 0000000000000000000000000000000000000000000000000000000000000020
        // 0000000000000000000000000000000000000000000000000000000000000080
        // 0000000000000000000000000000000000000000000000000000000000000020
        // 6ea6c150792130fbfb05b72aacba79157f9b86e05c975cb1585e68fb663801da
        // 0000000000000000000000000000000000000000000000000000000000000000
        // 0000000000000000000000000000000000000000000000000000000000000000
        // 0000000000000000000000000000000000000000000000000000000000000000
        // 000000000000000000000000000000000000000000000000000000000000ffff
        // 148f0b9e252c56e138f8c65a832ebca75241a386a918c14f466fb84a22f8b771

        let base = hex::decode("6ea6c150792130fbfb05b72aacba79157f9b86e05c975cb1585e68fb663801da")
            .unwrap();
        assert_eq!(base.len(), 32);

        let exp = hex::decode("000000000000000000000000000000000000000000000000000000000000ffff")
            .unwrap();

        let modulus =
            hex::decode("148f0b9e252c56e138f8c65a832ebca75241a386a918c14f466fb84a22f8b771")
                .unwrap();
        assert_eq!(modulus.len(), 32);

        let output = invoke_precompile_no_prepadding(&modulus, &base, &exp);

        let expected =
            hex::decode("08d8fab720b60be2e3af8437e15e467c625cd8704c2382449e7a50437355c6be")
                .unwrap();

        assert_eq!(output, expected);
    }

    #[test]
    fn test_5() {
        // 0^1 mod 2 - somewhat degenerate

        let base = hex::decode("00").unwrap();

        let exp = hex::decode("01").unwrap();

        let modulus = hex::decode("02").unwrap();

        let output = invoke_precompile_no_prepadding(&modulus, &base, &exp);

        let expected = hex::decode("").unwrap();

        assert_eq!(output, expected);
    }

    #[test]
    fn test_6() {
        // 0^0 mod 2 - somewhat degenerate

        let base = hex::decode("00").unwrap();

        let exp = hex::decode("00").unwrap();

        let modulus = hex::decode("02").unwrap();

        let output = invoke_precompile_no_prepadding(&modulus, &base, &exp);

        let expected =
            hex::decode("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap();

        assert_eq!(output, expected);
    }
}
