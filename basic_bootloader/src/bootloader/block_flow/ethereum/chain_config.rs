//! Chain-specific parameters of the Ethereum STF. They are fixed at build time through
//! environment variables, so a given binary (and its program commitment) is bound to one chain
//! configuration. Defaults are Ethereum mainnet values.

pub(crate) const CHAIN_ID: u64 = parse_u64_or(option_env!("ZKSYNC_OS_ETH_CHAIN_ID"), 1);

pub(crate) const PECTRA_EL_FORK_BLOCK_NUMBER: u64 =
    parse_u64_or(option_env!("ZKSYNC_OS_ETH_PECTRA_BLOCK"), 22431084);

/// Ethereum consensus limits `extraData` to 32 bytes; PoA chains (QBFT, IBFT2, Clique) keep
/// validator data and seals there, so it is larger.
pub(crate) const MAX_EXTRA_DATA_LEN: usize =
    parse_u64_or(option_env!("ZKSYNC_OS_ETH_MAX_EXTRA_DATA_LEN"), 32) as usize;

/// Whether the block-level system calls run: EIP-4788 and EIP-2935 before transactions,
/// EIP-7002 and EIP-7251 after them. Besu skips all four on PoA chains.
pub(crate) const SYSTEM_CALLS_ENABLED: bool =
    parse_u64_or(option_env!("ZKSYNC_OS_ETH_SYSTEM_CALLS"), 1) != 0;

const fn parse_u64_or(value: Option<&str>, default: u64) -> u64 {
    let Some(value) = value else {
        return default;
    };
    let bytes = value.as_bytes();
    assert!(!bytes.is_empty());
    let mut result = 0u64;
    let mut i = 0;
    while i < bytes.len() {
        let digit = bytes[i];
        assert!(digit.is_ascii_digit());
        result = match result.checked_mul(10) {
            Some(r) => match r.checked_add((digit - b'0') as u64) {
                Some(r) => r,
                None => panic!("value overflows u64"),
            },
            None => panic!("value overflows u64"),
        };
        i += 1;
    }
    result
}
