pub fn to_abi_name(native_name: &str) -> String {
    match native_name {
        "u32" => "uint32".to_string(),
        "u64" => "uint64".to_string(),
        "Address" => "address".to_string(),
        "U256" => "uint256".to_string(),
        x => panic!("Unsupported type {x}"),
    }
}
