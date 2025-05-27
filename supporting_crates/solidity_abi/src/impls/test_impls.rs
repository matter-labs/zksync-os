use crate::codable_trait::*;
use crate::impls::codable_array::format_short_integer;
use crate::impls::*;
use ruint::aliases::U256;
use solidity_abi_derive::compute_selector;
use solidity_abi_derive::SolidityCodable;

#[allow(dead_code)]
#[derive(SolidityCodable)]
pub struct TestStructWithLifetime<'a> {
    pub slice: Slice<'a, U256>,
}

#[allow(dead_code)]
#[derive(SolidityCodable)]
struct TestStructNoLifetime {
    pub a: U256,
    pub b: U256,
    pub c: U256,
}

#[allow(dead_code)]
#[derive(SolidityCodable)]
pub struct TestStructWithArrays<'a> {
    pub a: Array<'a, U256, 3>,
}

#[allow(dead_code)]
#[derive(SolidityCodable)]
pub struct TestStructWithDoubleSlice<'a> {
    pub slice: Slice<'a, Slice<'a, U256>>,
}

#[allow(dead_code)]
#[derive(SolidityCodable)]
pub struct TestStructWithSliceOverArray<'a> {
    pub slice: Slice<'a, Array<'a, U256, 4>>,
}

const _: () = const {
    assert!(TestStructNoLifetime::IS_DYNAMIC == false);
    assert!(TestStructWithLifetime::<'static>::IS_DYNAMIC == true);
    assert!(TestStructWithArrays::<'static>::IS_DYNAMIC == false);
    assert!(TestStructWithDoubleSlice::<'static>::IS_DYNAMIC == true);
    assert!(TestStructWithSliceOverArray::<'static>::IS_DYNAMIC == true);

    assert!(TestStructNoLifetime::HEAD_SIZE == 32 * 3);
    assert!(TestStructWithLifetime::<'static>::HEAD_SIZE == 32);
    assert!(TestStructWithArrays::<'static>::HEAD_SIZE == 32 * 3);
    assert!(TestStructWithDoubleSlice::<'static>::HEAD_SIZE == 32);
    assert!(TestStructWithSliceOverArray::<'static>::HEAD_SIZE == 32);
};

#[compute_selector]
fn transfer(&mut self, _to: Address, _amount: U256) -> () {}

#[compute_selector]
fn chainId(&self) -> () {}

#[compute_selector]
fn array_work(&self, _input: Array<'_, U256, 100>) -> () {}

#[cfg(test)]
mod test {
    use std::alloc::Global;

    use ruint::aliases::U160;

    use super::*;

    #[allow(dead_code)]
    #[derive(SolidityCodable)]
    struct ERC20TransferParams {
        pub to: Address,
        pub amount: U256,
    }

    #[allow(dead_code)]
    #[derive(SolidityCodable)]
    struct SwapParams<'a> {
        pub aggregator_id: SolidityString<'a>,
        pub token_from: Address,
        pub amount: U256,
        pub data: Bytes<'a>,
    }

    #[test]
    fn show_selector() {
        let mut buffer = vec![0u8; 1024];
        let mut is_first = true;
        let mut offset = 0;
        ERC20TransferParams::append_to_selector(&mut buffer, &mut offset, &mut is_first).unwrap();
        assert_eq!(
            core::str::from_utf8(&buffer[..offset]).unwrap(),
            "address,uint256"
        );
    }

    #[test]
    fn test_nested_selectors() {
        #[compute_selector]
        fn foo(&self, a: Array<'_, Array<'_, U256, 100>, 64>) -> () {}

        #[compute_selector]
        fn foo2(
            &mut self,
            a: Slice<'_, (U256, U256, U256)>,
            b: (Slice<'_, U256>, Array<'_, U256, 100>, Vec<U256>),
        ) -> () {
        }

        assert_eq!(
            FOO_SOLIDITY_ABI_SELECTOR.to_be_bytes(),
            compute_selector::<Array<'_, Array<'_, U256, 100>, 64>, _>("foo", 2048, Global)
                .unwrap()
        );

        assert_eq!(
            FOO2_SOLIDITY_ABI_SELECTOR.to_be_bytes(),
            compute_selector::<
                (
                    Slice<'_, (U256, U256, U256)>,
                    (Slice<'_, U256>, Array<'_, U256, 100>, Vec<U256>)
                ),
                _,
            >("foo2", 2048, Global)
            .unwrap()
        );
    }

    #[test]
    fn test_selectors() {
        use std::alloc::Global;

        assert_eq!(
            compute_selector::<ERC20TransferParams, _>("transfer", 1024, Global).unwrap(),
            0xa9059cbbu32.to_be_bytes(),
        );

        assert_eq!(TRANSFER_SOLIDITY_ABI_SELECTOR, 0xa9059cbbu32);

        assert_eq!(
            compute_selector::<SwapParams<'_>, _>("swap", 1024, Global).unwrap(),
            0x5f575529u32.to_be_bytes(),
        );

        use solidity_abi_derive::derive_simple_func_selector;

        assert_eq!(
            CHAINID_SOLIDITY_ABI_SELECTOR,
            derive_simple_func_selector!("chainId", ""),
        )
    }

    fn hash(s: &str) -> u32 {
        use const_keccak256::keccak256_digest;

        let digest_input = s.as_bytes();
        let output = keccak256_digest(digest_input);
        let selector = [output[0], output[1], output[2], output[3]];

        u32::from_be_bytes(selector)
    }

    #[test]
    fn test_erc20() {
        #[compute_selector]
        fn totalSupply(&self) -> U256 {}

        assert_eq!(TOTALSUPPLY_SOLIDITY_ABI_SELECTOR, hash("totalSupply()"));

        #[compute_selector]
        fn balanceOf(&self, account: Address) -> U256 {}

        assert_eq!(BALANCEOF_SOLIDITY_ABI_SELECTOR, hash("balanceOf(address)"));

        #[compute_selector]
        fn allowance(&self, owner: Address, spender: Address) -> U256 {}

        assert_eq!(
            ALLOWANCE_SOLIDITY_ABI_SELECTOR,
            hash("allowance(address,address)")
        );

        #[compute_selector]
        fn approve(&mut self, spender: Address, value: U256) -> bool {}

        assert_eq!(
            APPROVE_SOLIDITY_ABI_SELECTOR,
            hash("approve(address,uint256)")
        );

        #[compute_selector]
        fn transferFrom(&mut self, from: Address, to: Address, value: U256) -> bool {}

        assert_eq!(
            TRANSFERFROM_SOLIDITY_ABI_SELECTOR,
            hash("transferFrom(address,address,uint256)")
        )
    }

    #[test]
    fn test_write_access() {
        let mut input = hex::decode("a9059cbb000000000000000000000000ac7d58911d4cac710b2f3983d755580dcb6c898e000000000000000000000000000000000000000000000000000000026e37dd39").unwrap();
        let mut offset = 0;
        let mut parsed = <ERC20TransferParams as SolidityCodable>::ReflectionRefMut::parse_mut(
            &mut input[4..],
            &mut offset,
        )
        .unwrap();
        let new_value = Address(U160::from_str_radix("1234", 16).unwrap());
        parsed.to.write(&new_value).unwrap();
        assert_eq!(
            &input,
            &hex::decode("a9059cbb0000000000000000000000000000000000000000000000000000000000001234000000000000000000000000000000000000000000000000000000026e37dd39").unwrap()
        );
    }
}
