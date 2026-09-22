use rig::alloy::primitives::{address, keccak256};
use rig::constants::{CONTRACT_DEPLOYER, DEFAULT_BALANCE};
use rig::crypto::{blake2s::Blake2s256, MiniDigest};
use rig::ruint::aliases::U256;
use rig::utils::L1TxBuilder;
use rig::zk_ee::system::metadata::chain_config::{
    DEFAULT_MAX_CODE_SIZE, DEFAULT_MAX_TX_GAS_LIMIT, LARGE_MAX_CODE_SIZE,
};
use rig::zk_ee::utils::Bytes32;
use rig::{assert_tx_reverted, assert_tx_success, TestingFramework};

#[test]
fn bytecode_hooks_enforce_code_size_option() {
    let upgrader = address!("000000000000000000000000000000000000800f");
    let hook = address!("0000000000000000000000000000000000007002");
    let target = address!("1000000000000000000000000000000000000000");
    for large in [false, true] {
        let limit = if large {
            LARGE_MAX_CODE_SIZE
        } else {
            DEFAULT_MAX_CODE_SIZE
        };
        for size in [limit, limit + 1] {
            for temporary_deployer in [false, true] {
                let code = vec![0u8; size as usize];
                let hash = Bytes32::from_array(Blake2s256::digest(&code));
                let observable_hash = keccak256(&code);
                let mut calldata = Vec::new();
                if temporary_deployer {
                    calldata.extend_from_slice(
                        &keccak256("setBytecodeDetailsEVM(address,bytes32,uint32,bytes32)")[..4],
                    );
                }
                calldata.extend_from_slice(&[0u8; 12]);
                calldata.extend_from_slice(target.as_slice());
                calldata.extend_from_slice(hash.as_u8_ref());
                calldata.extend_from_slice(&U256::from(size).to_be_bytes::<32>());
                calldata.extend_from_slice(observable_hash.as_slice());

                let mut tester = TestingFramework::new()
                    .with_preimage(hash, &code)
                    .with_balance(upgrader, U256::from(DEFAULT_BALANCE));
                tester.set_chain_config(tester.chain_config().with_large_contracts(large));
                if large {
                    // The pinned REVM system hooks still hard-code 24 KiB, independently
                    // of its configurable EVM limit. Keep forward/RISC-V checks here.
                    tester.disable_revm_consistency_check();
                }
                let tx = L1TxBuilder::new()
                    .from(upgrader)
                    .to(if temporary_deployer {
                        CONTRACT_DEPLOYER
                    } else {
                        hook
                    })
                    .input(calldata)
                    .gas_limit(u128::from(DEFAULT_MAX_TX_GAS_LIMIT))
                    .build();
                let output = tester.execute_block(vec![tx]);
                let account = tester.get_account_properties(&target);
                if size <= limit {
                    assert_tx_success!(output, 0);
                    assert_eq!(account.observable_bytecode_len, size);
                    assert_eq!(
                        account.observable_bytecode_hash.as_u8_array(),
                        observable_hash.0
                    );
                } else {
                    assert_tx_reverted!(output, 0);
                    assert_eq!(account.observable_bytecode_len, 0);
                    assert!(account.bytecode_hash.is_zero());
                }
            }
        }
    }
}
