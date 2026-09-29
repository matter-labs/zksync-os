//! Generation modes and the restrictions each one places on a program.
//!
//! A mode narrows what the generator is allowed to emit so the resulting
//! program is suitable for a particular kind of check downstream.

use serde::{Deserialize, Serialize};

/// How restricted a generated program is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// Avoids GAS reads and system addresses for differential testing. Nested
    /// call limits and out-of-gas behavior can still depend on the gas schedule.
    GasAgnostic,
    /// Safe to run against a plain EVM fork of the chain: the program never
    /// touches the special system and hook addresses that only exist on this
    /// chain, so a generic EVM would see the same behavior.
    HookFree,
    /// No restrictions. Used for crash and slowness hunting, where hitting the
    /// system addresses and gas-dependent paths is the point.
    Unrestricted,
    /// Like `GasAgnostic` for what the program body may contain (no gas reads, no
    /// system addresses, so a differential comparison stays sound), but the lab
    /// runs it as an EIP-7702 scenario: the generated runtime is the delegation
    /// target, an account is authorized to delegate to it, and the calls drive
    /// the delegated account so the target's code runs in the EOA's context.
    Delegation7702,
}

impl Mode {
    /// Whether the GAS opcode (and any control flow that keys off remaining
    /// gas) may appear.
    pub fn allows_gas_reads(self) -> bool {
        !matches!(self, Mode::GasAgnostic | Mode::Delegation7702)
    }

    /// Whether a call, code query or value transfer may target one of the
    /// chain's system or hook addresses.
    pub fn allows_system_addresses(self) -> bool {
        matches!(self, Mode::Unrestricted)
    }
}

/// The deterministic deployment proxy address (the widely used CREATE2
/// factory). Hook-free programs avoid routing deployments through it.
pub const CREATE2_FACTORY: [u8; 20] = [
    0x4e, 0x59, 0xb4, 0x48, 0x47, 0xb3, 0x79, 0x57, 0x85, 0x88, 0x92, 0x0c, 0xa7, 0x8f, 0xbf, 0x26,
    0xc0, 0xb4, 0x95, 0x6c,
];

/// Whether an address is one that only the chain's system may meaningfully
/// occupy, or the CREATE2 factory.
///
/// These are the addresses a hook-free program must not target so that a
/// generic EVM would reproduce its behavior. The low ranges are the system
/// hook addresses and the on-chain system contract ranges; everything above
/// the low bytes is expected to be zero for those.
pub fn is_system_or_hook_address(addr: &[u8; 20]) -> bool {
    if *addr == CREATE2_FACTORY {
        return true;
    }
    // Every system and hook address fits in its low four bytes (the largest is
    // 0x10015), so if any of the top sixteen bytes are set this cannot be one.
    if addr[..16].iter().any(|byte| *byte != 0) {
        return false;
    }
    let low = u32::from_be_bytes([addr[16], addr[17], addr[18], addr[19]]);
    matches!(low, 0x7001 | 0x7002 | 0x7004 | 0x7100 | 0x8003)
        || (0x8006..=0x800f).contains(&low)
        || (0x0001_0001..=0x0001_0015).contains(&low)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr_from_low(low: u32) -> [u8; 20] {
        let mut a = [0u8; 20];
        a[16..20].copy_from_slice(&low.to_be_bytes());
        a
    }

    #[test]
    fn gas_agnostic_forbids_gas_reads() {
        assert!(!Mode::GasAgnostic.allows_gas_reads());
        assert!(Mode::HookFree.allows_gas_reads());
        assert!(Mode::Unrestricted.allows_gas_reads());
    }

    #[test]
    fn only_unrestricted_allows_system_addresses() {
        assert!(!Mode::GasAgnostic.allows_system_addresses());
        assert!(!Mode::HookFree.allows_system_addresses());
        assert!(Mode::Unrestricted.allows_system_addresses());
    }

    #[test]
    fn hook_and_system_ranges_are_flagged() {
        for low in [0x7001u32, 0x7002, 0x7004, 0x7100, 0x8003, 0x8006, 0x800f] {
            assert!(
                is_system_or_hook_address(&addr_from_low(low)),
                "low={low:#x}"
            );
        }
        for low in 0x0001_0001u32..=0x0001_0015 {
            assert!(
                is_system_or_hook_address(&addr_from_low(low)),
                "low={low:#x}"
            );
        }
        assert!(is_system_or_hook_address(&CREATE2_FACTORY));
    }

    #[test]
    fn ordinary_addresses_are_not_flagged() {
        assert!(!is_system_or_hook_address(&addr_from_low(0x1000)));
        assert!(!is_system_or_hook_address(&addr_from_low(0x05)));
        assert!(!is_system_or_hook_address(&addr_from_low(0x8005)));
        assert!(!is_system_or_hook_address(&addr_from_low(0x0001_0016)));
    }
}
