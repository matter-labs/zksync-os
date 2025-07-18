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

    // #[ignore = "depends on init and features"]
    #[test]
    fn test_on_vector() {
        // let test =
        //     Test {
        //         input: "000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000200db34d0e438249c0ed685c949cc28776a05094e1c48691dc3f2dca5fc3356d2a0663bd376e4712839917eb9a19c670407e2c377a2de385a3ff3b52104f7f1f4e0c7bf7717fb913896693dc5edbb65b760ef1b00e42e9d8f9af17352385e1cd742c9b006c0f669995cb0bb21d28c0aced2892267637b6470d8cee0ab27fc5d42658f6e88240c31d6774aa60a7ebd25cd48b56d0da11209f1928e61005c6eb709f3e8e0aaf8d9b10f7d7e296d772264dc76897ccdddadc91efa91c1903b7232a9e4c3b941917b99a3bc0c26497dedc897c25750af60237aa67934a26a2bc491db3dcc677491944bc1f51d3e5d76b8d846a62db03dedd61ff508f91a56d71028125035c3a44cbb041497c83bf3e4ae2a9613a401cc721c547a2afa3b16a2969933d3626ed6d8a7428648f74122fd3f2a02a20758f7f693892c8fd798b39abac01d18506c45e71432639e9f9505719ee822f62ccbf47f6850f096ff77b5afaf4be7d772025791717dbe5abf9b3f40cff7d7aab6f67e38f62faf510747276e20a42127e7500c444f9ed92baf65ade9e836845e39c4316d9dce5f8e2c8083e2c0acbb95296e05e51aab13b6b8f53f06c9c4276e12b0671133218cc3ea907da3bd9a367096d9202128d14846cc2e20d56fc8473ecb07cecbfb8086919f3971926e7045b853d85a69d026195c70f9f7a823536e2a8f4b3e12e94d9b53a934353451094b8102df3143a0057457d75e8c708b6337a6f5a4fd1a06727acf9fb93e2993c62f3378b37d56c85e7b1e00f0145ebf8e4095bd723166293c60b6ac1252291ef65823c9e040ddad14969b3b340a4ef714db093a587c37766d68b8d6b5016e741587e7e6bf7e763b44f0247e64bae30f994d248bfd20541a333e5b225ef6a61199e301738b1e688f70ec1d7fb892c183c95dc543c3e12adf8a5e8b9ca9d04f9445cced3ab256f29e998e69efaa633a7b60e1db5a867924ccab0a171d9d6e1098dfa15acde9553de599eaa56490c8f411e4985111f3d40bddfc5e301edb01547b01a886550a61158f7e2033c59707789bf7c854181d0c2e2a42a93cf09209747d7082e147eb8544de25c3eb14f2e35559ea0c0f5877f2f3fc92132c0ae9da4e45b2f6c866a224ea6d1f28c05320e287750fbc647368d41116e528014cc1852e5531d53e4af938374daba6cee4baa821ed07117253bb3601ddd00d59a3d7fb2ef1f5a2fbba7c429f0cf9a5b3462410fd833a69118f8be9c559b1000cc608fd877fb43f8e65c2d1302622b944462579056874b387208d90623fcdaf93920ca7a9e4ba64ea208758222ad868501cc2c345e2d3a5ea2a17e5069248138c8a79c0251185d29ee73e5afab5354769142d2bf0cb6712727aa6bf84a6245fcdae66e4938d84d1b9dd09a884818622080ff5f98942fb20acd7e0c916c2d5ea7ce6f7e173315384518f",
        //         expected: "8a5aea5f50dcc03dc7a7a272b5aeebc040554dbc1ffe36753c4fc75f7ed5f6c2cc0de3a922bf96c78bf0643a73025ad21f45a4a5cadd717612c511ab2bff1190fe5f1ae05ba9f8fe3624de1de2a817da6072ddcdb933b50216811dbe6a9ca79d3a3c6b3a476b079fd0d05f04fb154e2dd3e5cb83b148a006f2bcbf0042efb2ae7b916ea81b27aac25c3bf9a8b6d35440062ad8eae34a83f3ffa2cc7b40346b62174a4422584f72f95316f6b2bee9ff232ba9739301c97c99a9ded26c45d72676eb856ad6ecc81d36a6de36d7f9dafafee11baa43a4b0d5e4ecffa7b9b7dcefd58c397dd373e6db4acd2b2c02717712e6289bed7c813b670c4a0c6735aa7f3b0f1ce556eae9fcc94b501b2c8781ba50a8c6220e8246371c3c7359fe4ef9da786ca7d98256754ca4e496be0a9174bedbecb384bdf470779186d6a833f068d2838a88d90ef3ad48ff963b67c39cc5a3ee123baf7bf3125f64e77af7f30e105d72c4b9b5b237ed251e4c122c6d8c1405e736299c3afd6db16a28c6a9cfa68241e53de4cd388271fe534a6a9b0dbea6171d170db1b89858468885d08fecbd54c8e471c3e25d48e97ba450b96d0d87e00ac732aaa0d3ce4309c1064bd8a4c0808a97e0143e43a24cfa847635125cd41c13e0574487963e9d725c01375db99c31da67b4cf65eff555f0c0ac416c727ff8d438ad7c42030551d68c2e7adda0abb1ca7c10",
        //         name: "nagydani_4_square",
        //         precompile_id: "0000000000000000000000000000000000000005",
        //     };
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

        super::u256::init();

        let base = hex::decode("e09ad9675465c53a109fac66a445c91b292d2bb2c5268addb30cd82f80fcb0033ff97c80a5fc6f39193ae969c6ede6710a6b7ac27078a06d90ef1c72e5c85fb5").unwrap();
        assert_eq!(base.len(), 64);

        let exp = hex::decode("02").unwrap();
        assert_eq!(exp.len(), 1);

        let modulus = hex::decode("fc9e1f6beb81516545975218075ec2af118cd8798df6e08a147c60fd6095ac2bb02c2908cf4dd7c81f11c289e4bce98f3553768f392a80ce22bf5c4f4a248c6b").unwrap();
        assert_eq!(modulus.len(), 64);

        let expected = hex::decode("60008f1614cc01dcfb6bfb09c625cf90b47d4468db81b5f8b7a39d42f332eab9b2da8f2d95311648a8f243f4bb13cfb3d8f7f2a3c014122ebb3ed41b02783adc").unwrap();
        assert_eq!(expected.len(), 64);

        let mut advisor = NaiveAdvisor;
        let allocator = Global;

        let m = BigintRepr::from_big_endian_with_double_capacity(&modulus, allocator.clone());
        assert_eq!(m.digits, 2);
        let output = if m.digits == 0 {
            Vec::new_in(allocator)
        } else {
            let min_capacity = m.capacity();
            let x = BigintRepr::from_big_endian_with_double_capacity_or_min_capacity(
                &base,
                min_capacity,
                allocator.clone(),
            );
            assert_eq!(x.digits, 2);
            assert_eq!(x.capacity(), 4);
            let x = x.modpow(&exp, m, &mut advisor, allocator.clone());
            let r = x.to_big_endian(allocator);
            r
        };

        assert_eq!(&output, &expected);
    }
}
