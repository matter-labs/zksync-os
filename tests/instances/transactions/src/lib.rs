//!
//! These tests are focused on different tx types.
//!
#![cfg(test)]
use alloy::consensus::{TxEip1559, TxEip2930, TxLegacy};
use alloy::primitives::TxKind;
use alloy::signers::local::PrivateKeySigner;
use rig::alloy::consensus::TxEip7702;
use rig::alloy::primitives::{address, b256};
use rig::alloy::rpc::types::{AccessList, AccessListItem, TransactionRequest};
use rig::basic_system::system_implementation::system::pubdata::PUBDATA_ENCODING_VERSION;
use rig::ruint::aliases::{B160, U256};
use rig::system_hooks::addresses_constants::{L1_MESSENGER_ADDRESS, L2_BASE_TOKEN_ADDRESS, L2_INTEROP_ROOT_STORAGE_ADDRESS};
use rig::zksync_os_interface::error::InvalidTransaction;
use rig::{alloy, zksync_web3_rs, Chain};
use rig::{utils::*, BlockContext};
use std::str::FromStr;
use zksync_web3_rs::eip712::Eip712Meta;
use zksync_web3_rs::signers::{LocalWallet, Signer};

mod native_charging;

fn run_config() -> Option<rig::chain::RunConfig> {
    Some(rig::chain::RunConfig {
        app: Some("for_tests".to_string()),
        only_forward: false,
        check_storage_diff_hashes: true,
        ..Default::default()
    })
}

#[test]
fn run_base_system() {
    let mut chain = Chain::empty(None);
    // FIXME: this address looks very similar to bridgehub/shared bridge on gateway.
    // Which seems to suggest that it is special.
    // Consider changing this one to be more "random".

    let wallet = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();
    let wallet_ethers = LocalWallet::from_bytes(wallet.to_bytes().as_slice()).unwrap();

    // We used for test where from cannot have deployed code
    let eoa_wallet = PrivateKeySigner::from_str(
        "a226d3a5c8c408741c3446c762aee8dff742f21e381a0e5ab85a96c5c00100be",
    )
    .unwrap();
    let eoa_wallet_ethers = LocalWallet::from_bytes(eoa_wallet.to_bytes().as_slice()).unwrap();

    let from = wallet_ethers.address();
    let to = address!("0000000000000000000000000000000000010002");

    let encoded_mint_tx = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 1000,
            gas_limit: 80_000,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    let encoded_transfer_tx = {
        let transfer_tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 1,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 1000,
            gas_limit: 60_000,
            to: TxKind::Call(to),
            value: Default::default(),
            access_list: Default::default(),
            input: hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap().into(),
        };
        rig::utils::sign_and_encode_alloy_tx(transfer_tx, &wallet)
    };

    // `to` == null
    let encoded_deployment_tx = {
        let deployment_tx = TxEip2930 {
            chain_id: 37u64,
            nonce: 2,
            gas_price: 1000,
            gas_limit: 900_000,
            to: TxKind::Create,
            value: Default::default(),
            access_list: Default::default(),
            input: hex::decode(ERC_20_DEPLOYMENT_BYTECODE).unwrap().into(),
        };
        rig::utils::sign_and_encode_alloy_tx(deployment_tx, &wallet)
    };
    let encoded_transfer_to_eoa_tx = {
        let eoa_to = address!("4242000000000000000000000000000000000000");
        let transfer_to_eoa = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 1000,
            gas_limit: 21_000,
            to: TxKind::Call(eoa_to),
            value: alloy::primitives::U256::from(100),
            access_list: Default::default(),
            input: Default::default(),
        };
        rig::utils::sign_and_encode_alloy_tx(transfer_to_eoa, &eoa_wallet)
    };

    let encoded_mint2_tx = {
        let mint_tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 3,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 1000,
            gas_limit: 60_000,
            to: TxKind::Call(address!("14c252e395055507b10f199dd569f2379465d874")),
            value: Default::default(),
            access_list: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    let encoded_l1_l2_transfer = {
        let transfer = TransactionRequest {
            chain_id: Some(37),
            from: Some(address!("1234000000000000000000000000000000000000")),
            to: Some(TxKind::Call(address!(
                "4242000000000000000000000000000000000000"
            ))),
            gas: Some(21_000),
            max_fee_per_gas: Some(1000),
            max_priority_fee_per_gas: Some(1000),
            value: Some(alloy::primitives::U256::from(100)),
            nonce: Some(0),
            ..TransactionRequest::default()
        };
        rig::utils::encode_l1_tx(transfer)
    };

    let encoded_l1_l2_erc_transfer = {
        let tx = TransactionRequest {
            chain_id: Some(37),
            from: Some(alloy::signers::Signer::address(&wallet)),
            to: Some(TxKind::Call(to)),
            gas: Some(40_000),
            max_fee_per_gas: Some(1000),
            max_priority_fee_per_gas: Some(1000),
            nonce: Some(3),
            input: hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap().into(),
            ..TransactionRequest::default()
        };
        rig::utils::encode_l1_tx(tx)
    };

    let transactions = vec![
        encoded_mint_tx,
        encoded_transfer_tx,
        encoded_deployment_tx,
        encoded_transfer_to_eoa_tx,
        encoded_mint2_tx,
        encoded_l1_l2_transfer,
        encoded_l1_l2_erc_transfer,
    ];

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();
    chain.set_evm_bytecode(B160::from_be_bytes(to.into_array()), &bytecode);

    chain
        .set_balance(
            B160::from_be_bytes(from.0),
            U256::from(1_000_000_000_000_000_u64),
        )
        .set_balance(
            B160::from_be_bytes(eoa_wallet.address().0 .0),
            U256::from(1_000_000_000_000_000_u64),
        );

    let output = chain.run_block(transactions, None, None, run_config());

    // Assert all txs succeeded
    assert!(output.tx_results.iter().cloned().enumerate().all(|(i, r)| {
        let success = r.clone().is_ok_and(|o| o.is_success());
        if !success {
            println!("Transaction {i} failed with: {r:?}",)
        }
        success
    }));
}

#[test]
fn test_block_of_erc20() {
    let mut chain = Chain::empty_randomized(None);
    run_block_of_erc20(&mut chain, 10, None);
}

#[test]
fn test_gas_price_zero() {
    let mut chain = Chain::empty_randomized(None);
    let block_context = BlockContext {
        eip1559_basefee: U256::ZERO,
        ..BlockContext::default()
    };
    run_block_of_erc20(&mut chain, 10, Some(block_context));
}

#[test]
fn test_withdrawal() {
    let mut chain = Chain::empty(None);
    let bytecode_vec: Vec<u8> = hex::decode("60806040526004361015610013575b610129565b61001e60003561002d565b6362f84b240361000e576100f3565b60e01c90565b60405190565b600080fd5b600080fd5b600080fd5b600080fd5b600080fd5b600080fd5b909182601f830112156100915781359167ffffffffffffffff831161008c57602001926001830284011161008757565b610052565b61004d565b610048565b906020828203126100c857600082013567ffffffffffffffff81116100c3576100bf9201610057565b9091565b610043565b61003e565b90565b6100d9906100cd565b9052565b91906100f1906000602085019401906100d0565b565b346101245761012061010f610109366004610096565b90610404565b610117610033565b918291826100dd565b0390f35b610039565b600080fd5b600090565b90565b60018060a01b031690565b90565b61015861015361015d92610133565b610141565b610136565b90565b61016b617000610144565b90565b90565b61018561018061018a9261016e565b610141565b610136565b90565b634e487b7160e01b600052601160045260246000fd5b6101af6101b591610136565b91610136565b019060018060a01b0382116101c657565b61018d565b6101df6101da6101e492610136565b610141565b610136565b90565b6101f0906101cb565b90565b610216610211610201610160565b61020b6001610171565b906101a3565b6101e7565b90565b905090565b90826000939282370152565b90918261023a8161024193610219565b809361021e565b0190565b90916102509261022a565b90565b601f801991011690565b634e487b7160e01b600052604160045260246000fd5b9061027d90610253565b810190811067ffffffffffffffff82111761029757604052565b61025d565b906102af6102a8610033565b9283610273565b565b67ffffffffffffffff81116102cf576102cb602091610253565b0190565b61025d565b906102e66102e1836102b1565b61029c565b918252565b606090565b3d60001461030d576103013d6102d4565b903d6000602084013e5b565b6103156102eb565b9061030b565b151590565b5190565b90565b90565b61033e61033961034392610327565b610141565b610324565b90565b60000190565b60200190565b61035c90516100cd565b90565b1b90565b61037d61037861037283610320565b9261034c565b610352565b906020811061038b575b5090565b61039e906000199060200360080261035f565b1638610387565b6103ae906101e7565b90565b6103ba906100cd565b90565b60209181520190565b91906103e0816103d9816103e5956103bd565b809561021e565b610253565b0190565b909161040192602083019260008185039101526103c6565b90565b919061040e61012e565b5060008061041a6101f3565b85828591610432610429610033565b93849283610245565b03925af16104486104416102f0565b911561031b565b80156104cb575b6104ae5761045c90610363565b923384919261049461048e7f3a36e47291f4201faf137fab081d92295bce2d53be2c6ca68ba82c7faa9ce241936103a5565b936103b1565b936104a96104a0610033565b928392836103e9565b0390a3565b600063f801b06960e01b8152806104c760048201610346565b0390fd5b506104d581610320565b6104e86104e2602061032a565b91610324565b141561044f56fea2646970667358221220eaa72a072e95715690c88c25da2fd94b2b0a7a610c93721e3eb39b3c9804086464736f6c634300081c0033").expect("valid hex");
    let l1_messenger_hook_bytecode: &[u8] = &bytecode_vec; // borrow as slice
    chain.set_evm_bytecode(L1_MESSENGER_ADDRESS, l1_messenger_hook_bytecode);

    let wallet = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();
    let wallet_ethers = LocalWallet::from_bytes(wallet.to_bytes().as_slice()).unwrap();

    let from = wallet_ethers.address();

    // L2 base token address
    let to = address!("000000000000000000000000000000000000800a");

    let withdrawal_calldata =
        hex::decode("51cff8d9000000000000000000000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .unwrap();

    let withdrawal_tx = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 1000,
            gas_limit: 500_000,
            to: TxKind::Call(to),
            value: U256::from(10),
            input: withdrawal_calldata.into(),
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    let mut withdrawal_with_message_calldata =
        hex::decode("84bc3eb0000000000000000000000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .unwrap();
    // Offset (64)
    withdrawal_with_message_calldata.extend_from_slice(&U256::from(64).to_be_bytes::<32>());
    // length, 2 bytes
    withdrawal_with_message_calldata.extend_from_slice(&U256::from(2).to_be_bytes::<32>());
    // Extra data
    withdrawal_with_message_calldata.extend_from_slice(&[1u8, 2u8]);

    let withdrawal_with_message_tx = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 1,
            gas_price: 1000,
            gas_limit: 500_000,
            to: TxKind::Call(to),
            value: U256::from(5),
            input: withdrawal_with_message_calldata.into(),
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    let transactions = vec![withdrawal_tx, withdrawal_with_message_tx];

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();
    chain.set_evm_bytecode(B160::from_be_bytes(to.into_array()), &bytecode);

    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );

    let bytecode_vec_l2_base_token: Vec<u8> = hex::decode("60806040526004361015610013575b61019b565b61001e60003561003d565b806351cff8d914610038576384bc3eb00361000e57610171565b6100b3565b60e01c90565b60405190565b600080fd5b600080fd5b60018060a01b031690565b61006790610053565b90565b6100738161005e565b0361007a57565b600080fd5b9050359061008c8261006a565b565b906020828203126100a8576100a59160000161007f565b90565b610049565b60000190565b6100c66100c136600461008e565b6103fb565b6100ce610043565b806100d8816100ad565b0390f35b600080fd5b600080fd5b600080fd5b909182601f830112156101255781359167ffffffffffffffff831161012057602001926001830284011161011b57565b6100e6565b6100e1565b6100dc565b91909160408184031261016c57610144836000830161007f565b92602082013567ffffffffffffffff81116101675761016392016100eb565b9091565b61004e565b610049565b61018561017f36600461012a565b916105b7565b61018d610043565b80610197816100ad565b0390f35b600080fd5b90565b90565b6101ba6101b56101bf926101a0565b6101a3565b610053565b90565b6101cd6180006101a6565b90565b90565b6101e76101e26101ec926101d0565b6101a3565b610053565b90565b634e487b7160e01b600052601160045260246000fd5b61021161021791610053565b91610053565b019060018060a01b03821161022857565b6101ef565b61024161023c61024692610053565b6101a3565b610053565b90565b6102529061022d565b90565b61025e9061022d565b90565b61026a90610255565b90565b61029861029361028e61027e6101c2565b61028860086101d3565b90610205565b610249565b610261565b90565b6102a490610249565b90565b601f801991011690565b634e487b7160e01b600052604160045260246000fd5b906102d1906102a7565b810190811067ffffffffffffffff8211176102eb57604052565b6102b1565b60e01b90565b90565b610302816102f6565b0361030957565b600080fd5b9050519061031b826102f9565b565b90602082820312610337576103349160000161030e565b90565b610049565b5190565b60209181520190565b60005b83811061035d575050906000910152565b80602091830151818501520161034c565b61038d61039660209361039b936103848161033c565b93848093610340565b95869101610349565b6102a7565b0190565b6103b5916020820191600081840391015261036e565b90565b6103c0610043565b3d6000823e3d90fd5b6103d290610249565b90565b90565b6103e1906103d5565b9052565b91906103f9906000602085019401906103d8565b565b6104036107a7565b61044c60206104138484906108e4565b61042361041e61026d565b61029b565b61044160006362f84b24610435610043565b968795869485936102f0565b83526004830161039f565b03925af180156104d9576104ad575b50339190916104a86104966104907f2717ead6b9200dd235aad468c9809ea400fe33ac69b5bfaa6d3e90fc922b6398936103c9565b936103c9565b9361049f610043565b918291826103e5565b0390a3565b6104cd9060203d81116104d2575b6104c581836102c7565b81019061031d565b61045b565b503d6104bb565b6103b8565b600080fd5b906104f66104ef610043565b92836102c7565b565b67ffffffffffffffff8111610516576105126020916102a7565b0190565b6102b1565b90826000939282370152565b9092919261053c610537826104f8565b6104e3565b93818552602085019082840111610558576105569261051b565b565b6104de565b610568913691610527565b90565b91906105858161057e8161058a95610340565b809561051b565b6102a7565b0190565b916105b49391926105a7604082019460008301906103d8565b602081850391015261056b565b90565b9190916105c26107a7565b9161061960206105e0848633906105da8a889061055d565b92610991565b6105f06105eb61026d565b61029b565b61060e60006362f84b24610602610043565b968795869485936102f0565b83526004830161039f565b03925af180156106a85761067c575b5033919293909361067761066561065f7fc405fe8958410bbaf0c73b7a0c3e20859e86ca168a4c9b0def9c54d2555a306b956103c9565b956103c9565b9561066e610043565b9384938461058e565b0390a3565b61069c9060203d81116106a1575b61069481836102c7565b81019061031d565b610628565b503d61068a565b6103b8565b600090565b90565b6106c96106c46106ce926106b2565b6101a3565b610053565b90565b6106f46106ef6106df6101c2565b6106e960026106b5565b90610205565b610249565b90565b6107009061022d565b90565b61070c906106f7565b90565b61071890610249565b90565b905090565b61072c6000809261071b565b0190565b61073990610720565b90565b9061074e610749836104f8565b6104e3565b918252565b606090565b3d600014610775576107693d61073c565b903d6000602084013e5b565b61077d610753565b90610773565b151590565b90565b61079f61079a6107a492610788565b6101a3565b6103d5565b90565b6107af6106ad565b5034906000806107cd6107c86107c36106d1565b610703565b61070f565b846107d6610043565b90816107e181610730565b03925af16107f76107f0610758565b9115610783565b908115610823575b5061080657565b6000631bc5aabf60e21b81528061081f600482016100ad565b0390fd5b61082d915061033c565b61084061083a600061078b565b916103d5565b1415386107ff565b63ffffffff60e01b1690565b90565b61086361086891610848565b610854565b9052565b60601b90565b61087b9061086c565b90565b61088790610872565b90565b61089661089b9161005e565b61087e565b9052565b90565b6108ae6108b3916103d5565b61089f565b9052565b602093926108d86004836108d06014956108e097610857565b01809261088a565b0180926108a2565b0190565b610921906108f0610753565b50610912636c0960f960e01b9193610906610043565b948593602085016108b7565b602082018103825203826102c7565b90565b610949610940926020926109378161033c565b9485809361071b565b93849101610349565b0190565b60149361097f8561098e98979561097760048661096f6020986109879a610857565b01809261088a565b0180926108a2565b01809261088a565b0190610924565b90565b90926109d4926109c5916109a3610753565b50636c0960f960e01b93959190916109b9610043565b9687956020870161094d565b602082018103825203826102c7565b9056fea26469706673582212201e2659874946c1e4eedce28cd876068bf3e5496c34be61dc6ed85365e70db29664736f6c634300081c0033").expect("valid hex");
    let l2_base_token_contract_bytecode: &[u8] = &bytecode_vec_l2_base_token; // borrow as slice
    chain.set_evm_bytecode(L2_BASE_TOKEN_ADDRESS, l2_base_token_contract_bytecode);

    let output = chain.run_block(transactions, None, None, run_config());

    // Assert all txs succeeded
    assert!(output.tx_results.iter().cloned().enumerate().all(|(i, r)| {
        let success = r.clone().is_ok_and(|o| o.is_success());
        if !success {
            println!("Transaction {i} failed with: {r:?}")
        }
        success
    }));

    // Check preimage of withdrawal
    let mut expected_preimage =
        hex::decode("6c0960f9aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
    expected_preimage.extend_from_slice(&U256::from(10).to_be_bytes::<32>());

    let logs = output
        .tx_results
        .first()
        .unwrap()
        .clone()
        .unwrap()
        .l2_to_l1_logs;

    let first_log = logs.first().unwrap().clone();
    let returned_preimage = first_log.preimage.unwrap();
    assert_eq!(expected_preimage, returned_preimage);
}

#[test]
fn test_tx_with_access_list() {
    let mut chain = Chain::empty(None);

    let wallet = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();
    let wallet_ethers = LocalWallet::from_bytes(wallet.to_bytes().as_slice()).unwrap();

    let from = wallet_ethers.address();

    let to = address!("0000000000000000000000000000000000010002");

    // We do an initial mint to populate storage slots, otherwise SSTORE
    // costs are hard to reason about.
    let encoded_mint_tx = {
        let access_list = AccessList::from(vec![AccessListItem {
            address: to,
            storage_keys: vec![b256!(
                "0x0000000000000000000000000000000000000000000000000000000000000000"
            )],
        }]);
        let mint_tx = TxEip2930 {
            chain_id: 37u64,
            nonce: 0,
            gas_price: 1000,
            gas_limit: 75_000,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
            access_list,
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    let transactions = vec![encoded_mint_tx];

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();
    chain.set_evm_bytecode(B160::from_be_bytes(to.into_array()), &bytecode);

    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );

    let output = chain.run_block(transactions, None, None, run_config());

    // Assert all txs succeeded
    let result0 = output.tx_results.first().unwrap().clone();
    assert!(result0.is_ok_and(|o| o.is_success()));
}

#[test]
fn test_tx_with_authorization_list() {
    use rig::alloy::eips::eip7702::*;
    use rig::alloy::signers::SignerSync;
    let mut chain = Chain::empty(None);

    let wallet = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();
    let wallet_ethers = LocalWallet::from_bytes(wallet.to_bytes().as_slice()).unwrap();

    let delegate = PrivateKeySigner::from_str(
        "a226d3a5c8c408741c3446c762aee8dff742f21e381a0e5ab85a96c5c00100be",
    )
    .unwrap();

    let from = wallet_ethers.address();
    let to = delegate.address();

    let erc_20_contract = address!("0000000000000000000000000000000000010002");

    let encoded_mint_tx = {
        let authorization = Authorization {
            chain_id: U256::from(37u64),
            address: erc_20_contract,
            nonce: 0,
        };
        let signed_hash = authorization.signature_hash();
        let sig = delegate.sign_hash_sync(&signed_hash).expect("must sign");
        let signed = authorization.into_signed(sig);
        let authorization_list = vec![signed];
        let mint_tx = TxEip7702 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 1000,
            gas_limit: 100_000,
            to,
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
            access_list: Default::default(),
            authorization_list,
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    let transactions = vec![encoded_mint_tx];

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();
    chain.set_evm_bytecode(B160::from_be_bytes(erc_20_contract.into_array()), &bytecode);

    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );

    let run_config = rig::chain::RunConfig {
        app: Some("for_tests".to_string()),
        only_forward: false,
        check_storage_diff_hashes: true,
        ..Default::default()
    };
    let output = chain.run_block(transactions, None, None, Some(run_config));

    // Assert all txs succeeded
    let result0 = output.tx_results.first().unwrap().clone();
    assert!(result0.is_ok_and(|o| o.is_success()));
}

// Test that slots made warm in a tx are cold in the next tx
#[test]
fn test_cold_in_new_tx() {
    let mut chain = Chain::empty(None);

    let wallet = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();
    let wallet_ethers = LocalWallet::from_bytes(wallet.to_bytes().as_slice()).unwrap();

    let from = wallet_ethers.address();

    let to = address!("0000000000000000000000000000000000010002");

    // We do an initial mint to populate storage slots, otherwise SSTORE
    // costs are hard to reason about.
    let encoded_mint_tx = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 1000,
            gas_limit: 68_358,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    // Gas is just enough to succeed.
    let encoded_mint1_tx = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 1,
            gas_price: 1000,
            gas_limit: 34158,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    // Any lower gas amount should fail
    let encoded_mint_tx2 = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 2,
            gas_price: 1000,
            gas_limit: 34158 - 1,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    let transactions = vec![encoded_mint_tx, encoded_mint1_tx, encoded_mint_tx2];

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();
    chain.set_evm_bytecode(B160::from_be_bytes(to.into_array()), &bytecode);

    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );

    let output = chain.run_block(transactions, None, None, run_config());

    // Assert all txs succeeded
    let result0 = output.tx_results.first().unwrap().clone();
    let result1 = output.tx_results.get(1).unwrap().clone();
    let result2 = output.tx_results.get(2).unwrap().clone();
    assert!(result0.is_ok_and(|o| o.is_success()));
    assert!(result1.is_ok_and(|o| o.is_success()));
    assert!(result2.is_ok_and(|o| !o.is_success()));
}

#[test]
// Test that if we send 2 simple transfers from and to different addresses,
// the length of the pubdata from both is the same.
fn test_independent_txs_have_same_pubdata() {
    let mut chain = Chain::empty(None);

    let wallet1 = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();

    let wallet2 = PrivateKeySigner::from_str(
        "abcdebdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();
    let to1 = address!("0000000000000000000000000000000000010002");
    let to2 = address!("0000000000000000000000000000000000010003");

    let encoded_tx_1 = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 1500,
            max_priority_fee_per_gas: 1500,
            gas_limit: 21_000,
            to: TxKind::Call(to1),
            value: U256::from(10),
            input: Default::default(),
            ..Default::default()
        };
        rig::utils::sign_and_encode_alloy_tx(tx, &wallet1)
    };

    let encoded_tx_2 = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 1500,
            max_priority_fee_per_gas: 1500,
            gas_limit: 21_000,
            to: TxKind::Call(to2),
            value: U256::from(10),
            input: Default::default(),
            ..Default::default()
        };
        rig::utils::sign_and_encode_alloy_tx(tx, &wallet2)
    };

    let transactions = vec![encoded_tx_1, encoded_tx_2];

    chain
        .set_balance(
            B160::from_be_bytes(wallet1.address().0 .0),
            U256::from(1_000_000_000_000_000_u64),
        )
        .set_balance(
            B160::from_be_bytes(wallet2.address().0 .0),
            U256::from(1_000_000_000_000_000_u64),
        );

    let output = chain.run_block(transactions, None, None, run_config());

    // Assert all txs succeeded and compare pubdata len
    assert!(output.tx_results.iter().cloned().enumerate().all(|(i, r)| {
        let success = r.clone().is_ok_and(|o| o.is_success());
        if !success {
            println!("Transaction {i} failed with: {r:?}",)
        }
        success
    }));
    let result1 = output.tx_results.first().unwrap().clone();
    let result2 = output.tx_results.get(1).unwrap().clone();
    let pubdata_used_1 = result1.unwrap().pubdata_used;
    let pubdata_used_2 = result2.unwrap().pubdata_used;
    assert_eq!(pubdata_used_1, pubdata_used_2, "Pubdata used not equal")
}

#[test]
fn test_invalid_tx_does_not_bump_tx_counter() {
    let wallet = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();
    let wallet_ethers = LocalWallet::from_bytes(wallet.to_bytes().as_slice()).unwrap();
    let from = wallet_ethers.address();
    let to = address!("0000000000000000000000000000000000010002");
    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();

    // Invalid tx first
    let encoded_mint1_tx = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 1000,
            gas_limit: 34_158_000_000_000,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };
    let withdrawal_tx = {
        let to = address!("000000000000000000000000000000000000800a");

        let withdrawal_calldata =
            hex::decode("51cff8d9000000000000000000000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .unwrap();

        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 1000,
            gas_limit: 500_000,
            to: TxKind::Call(to),
            value: U256::from(10),
            input: withdrawal_calldata.into(),
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    let mut chain = Chain::empty(None);
    let transactions = vec![encoded_mint1_tx, withdrawal_tx];
    chain.set_evm_bytecode(B160::from_be_bytes(to.into_array()), &bytecode);
    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );
    let output = chain.run_block(transactions, None, None, None);

    // Assert tx succeeded/failed
    let result0 = output.tx_results.first().unwrap().clone();
    let result1 = output.tx_results.get(1).unwrap().clone();

    assert!(result0.as_ref().is_err());
    assert!(result1.as_ref().is_ok_and(|o| o.is_success()));
    assert!(
        result1
            .unwrap()
            .l2_to_l1_logs
            .first()
            .unwrap()
            .log
            .tx_number_in_block
            == 0
    );
}

#[test]
fn test_invalid_tx_does_not_affect_native() {
    let wallet = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();
    let wallet_ethers = LocalWallet::from_bytes(wallet.to_bytes().as_slice()).unwrap();
    let from = wallet_ethers.address();
    let to = address!("0000000000000000000000000000000000010002");
    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();

    // First we run a single tx to get the "normal" amount of native used
    let encoded_mint_tx = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 1000,
            gas_limit: 500_000,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    let mut chain = Chain::empty(None);
    let transactions = vec![encoded_mint_tx.clone()];
    chain.set_evm_bytecode(B160::from_be_bytes(to.into_array()), &bytecode);
    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );
    let output = chain.run_block(transactions, None, None, None);

    // Assert tx succeeded
    let result = output.tx_results.first().unwrap().clone();
    assert!(result.as_ref().is_ok_and(|o| o.is_success()));

    let native_used_reference = result.unwrap().native_used;

    // Same tx but with a huge gas limit, which makes it invalid
    // We run this one first and then the valid one, and check that
    // the valid one uses the same amount of native as in the reference case.
    let encoded_mint1_tx = {
        let mint_tx = TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 1000,
            gas_limit: 34_158_000_000_000,
            to: TxKind::Call(to),
            value: Default::default(),
            input: hex::decode(ERC_20_MINT_CALLDATA).unwrap().into(),
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    let mut chain = Chain::empty(None);
    let transactions = vec![encoded_mint1_tx, encoded_mint_tx];
    chain.set_evm_bytecode(B160::from_be_bytes(to.into_array()), &bytecode);
    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );
    let output = chain.run_block(transactions, None, None, None);

    // Assert tx succeeded
    let result0 = output.tx_results.first().unwrap().clone();
    let result1 = output.tx_results.get(1).unwrap().clone();
    assert!(result0.as_ref().is_err());
    assert!(result1.as_ref().is_ok_and(|o| o.is_success()));
    assert_eq!(
        result1.unwrap().native_used,
        native_used_reference,
        "Native used doesn't match"
    );
}

// TODO: find better place for regression tests
#[test]
fn test_regression_returndata_empty_3541() {
    let mut chain = Chain::empty(None);

    let wallet = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();
    let wallet_ethers = LocalWallet::from_bytes(wallet.to_bytes().as_slice()).unwrap();
    // Code for:
    // PUSH13 0x63EF0000006000526004601CF3
    // PUSH1  0x00
    // MSTORE
    // PUSH1  0x0D
    // PUSH1  0x13
    // PUSH1  0x00
    // CREATE
    // RETURNDATASIZE
    // ISZERO
    // PUSH1  0x08
    // PC
    // ADD
    // JUMPI
    // PUSH1  0x00
    // PUSH1  0x00
    // REVERT
    // JUMPDEST
    // PUSH1  0x00
    // PUSH1  0x00
    // RETURN
    // This code tries to deploy a contract with code starting with EF and
    // expects returndata to be empty, otherwise it reverts.
    const BYTECODE: &str =
        "6c63ef0000006000526004601cf3600052600d60136000f03d15600858015760006000fd5b60006000f3";

    let from = wallet_ethers.address();

    let to = address!("0000000000000000000000000000000000010002");

    // We do an initial mint to populate storage slots, otherwise SSTORE
    // costs are hard to reason about.
    let encoded_tx = {
        let mint_tx = TxEip2930 {
            chain_id: 37u64,
            nonce: 0,
            gas_price: 1000,
            gas_limit: 1_000_000,
            to: TxKind::Call(to),
            value: Default::default(),
            ..Default::default()
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    let transactions = vec![encoded_tx];

    let bytecode = hex::decode(BYTECODE).unwrap();
    chain.set_evm_bytecode(B160::from_be_bytes(to.into_array()), &bytecode);

    chain.set_balance(
        B160::from_be_bytes(from.0),
        U256::from(1_000_000_000_000_000_u64),
    );

    let output = chain.run_block(transactions, None, None, run_config());

    // Assert all txs succeeded
    let result0 = output.tx_results.first().unwrap().clone();
    assert!(result0.is_ok_and(|o| o.is_success()));
}

/// Test that transactions with balance calculation overflow are properly rejected
#[test]
fn test_balance_overflow_protection() {
    let mut chain = Chain::empty(None);

    let wallet = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();

    let from = alloy::primitives::Address::from_slice(&wallet.address().as_slice());
    let to = address!("0000000000000000000000000000000000010002");

    // Set a reasonable balance that would be sufficient for normal transactions
    chain.set_balance(
        B160::from_be_bytes(from.into_array()),
        U256::from(1_000_000_000_000_000_u64),
    );

    // Test 1: Transaction with max_fee_per_gas * gas_limit overflow
    let overflow_fee_tx = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            gas_limit: u64::MAX, // Will cause overflow when multiplied with max_fee_per_gas
            max_fee_per_gas: u128::MAX,
            max_priority_fee_per_gas: 0,
            to: TxKind::Call(to),
            value: U256::from(100u64), // Small value
            ..Default::default()
        };
        rig::utils::sign_and_encode_alloy_tx(tx, &wallet)
    };

    // Test 2: Transaction with value + fee_amount overflow
    let overflow_total_tx = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 1,
            gas_limit: 100_000,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 0,
            to: TxKind::Call(to),
            value: U256::MAX, // Maximum value will cause overflow when adding fees
            ..Default::default()
        };
        rig::utils::sign_and_encode_alloy_tx(tx, &wallet)
    };

    let output = chain.run_block(
        vec![overflow_fee_tx, overflow_total_tx],
        None,
        None,
        run_config(),
    );

    assert!(
        output.tx_results.get(0).unwrap().is_err(),
        "Transaction with fee overflow should fail"
    );
    assert!(
        output.tx_results.get(1).unwrap().is_err(),
        "Transaction with total balance overflow should fail"
    );
}

/// Test that upgrade transactions (L1 -> L2) that revert raise an internal error
/// instead of a validation error.
#[test]
fn test_upgrade_tx_revert_internal_error() {
    let mut chain = Chain::empty(None);

    // Create a contract that always reverts
    let revert_contract_address = address!("0000000000000000000000000000000000010003");
    // Simple contract bytecode that just does REVERT(0, 0)
    let revert_bytecode = hex::decode("60006000fd").unwrap(); // PUSH1 0, PUSH1 0, REVERT
    chain.set_evm_bytecode(
        B160::from_be_bytes(revert_contract_address.into_array()),
        &revert_bytecode,
    );

    // Create a proper upgrade transaction that calls the reverting contract
    let upgrade_tx = encode_upgrade_tx(TransactionRequest {
        chain_id: Some(37),
        from: Some(address!("1234000000000000000000000000000000000000")),
        to: Some(TxKind::Call(revert_contract_address)),
        gas: Some(100_000u64),
        max_fee_per_gas: Some(0),
        max_priority_fee_per_gas: Some(0),
        value: Some(alloy::primitives::U256::from(0)),
        nonce: Some(0),
        ..TransactionRequest::default()
    });

    let transactions = vec![upgrade_tx];

    // Use run_block_no_panic to catch the error instead of panicking
    let result = chain.run_block_no_panic(transactions, None, None, None);

    // The upgrade transaction should fail with an internal error (not validation error)
    assert!(result.is_err());

    // The error should be an internal error containing "Upgrade transaction must succeed"
    let error = result.unwrap_err();
    let error_debug = format!("{:?}", error);
    assert!(
        error_debug.contains("Upgrade transaction must succeed"),
        "Expected error to contain 'Upgrade transaction must succeed', got: {}",
        error_debug
    );
}

#[test]
fn test_upgrade_tx_succeeds() {
    let mut chain = Chain::empty(None);

    // Create a contract that always succeeds
    let revert_contract_address = address!("0000000000000000000000000000000000010003");
    // Simple contract bytecode that just does RETURN(0, 0)
    let revert_bytecode = hex::decode("60006000f3").unwrap(); // PUSH1 0, PUSH1 0, RETURN
    chain.set_evm_bytecode(
        B160::from_be_bytes(revert_contract_address.into_array()),
        &revert_bytecode,
    );

    // Create a proper upgrade transaction that calls the contract
    let upgrade_tx = encode_upgrade_tx(TransactionRequest {
        chain_id: Some(37),
        from: Some(address!("1234000000000000000000000000000000000000")),
        to: Some(TxKind::Call(revert_contract_address)),
        gas: Some(100_000u64),
        max_fee_per_gas: Some(0),
        max_priority_fee_per_gas: Some(0),
        value: Some(alloy::primitives::U256::from(0)),
        nonce: Some(0),
        ..TransactionRequest::default()
    });

    let transactions = vec![upgrade_tx];

    // Use run_block_no_panic to catch the error instead of panicking
    let result = chain.run_block_no_panic(transactions, None, None, None);
    assert!(result.is_ok());

    assert!(result.unwrap().tx_results[0].as_ref().unwrap().is_success());
}

#[test]
fn test_invalid_transaction_type_failure() {
    let mut chain = Chain::empty(None);

    // Create a simple success contract for the call
    let contract_address = address!("0000000000000000000000000000000000010003");
    let success_bytecode = hex::decode("60006000f3").unwrap(); // PUSH1 0, PUSH1 0, RETURN
    chain.set_evm_bytecode(
        B160::from_be_bytes(contract_address.into_array()),
        &success_bytecode,
    );

    let transaction_types = vec![0x7d, 0x80, 0xFF]; // Some invalid types;

    for tx_type in transaction_types {
        let invalid_tx = encode_special_tx_type(
            TransactionRequest {
                chain_id: Some(37),
                from: Some(address!("1234000000000000000000000000000000000000")),
                to: Some(TxKind::Call(contract_address)),
                gas: Some(100_000u64),
                max_fee_per_gas: Some(0),
                max_priority_fee_per_gas: Some(0),
                value: Some(alloy::primitives::U256::from(0)),
                nonce: Some(0),
                ..TransactionRequest::default()
            },
            tx_type,
        );

        let transactions = vec![invalid_tx];
        let result = chain.run_block(transactions, None, None, run_config());
        assert!(
            result.tx_results[0].is_err(),
            "Transaction with invalid type should fail"
        );
    }
}

#[test]
fn test_modexp_intermediate_zero_block() {
    let mut chain = Chain::empty(None);
    let wallet = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();

    // Modexp precompile address
    let modexp_address = address!("0000000000000000000000000000000000000005");

    let input_data = hex::decode(concat!(
        // Base length (96 bytes)
        "0000000000000000000000000000000000000000000000000000000000000060",
        // Exponent length (1 byte)
        "0000000000000000000000000000000000000000000000000000000000000001",
        // Modulus length (96 bytes)
        "0000000000000000000000000000000000000000000000000000000000000060",
        // Base (96 bytes):
        "1000000000000000000000000000000000000000000000000000000000000001",
        "0000000000000000000000000000000000000000000000000000000000000000", // zeroed 32-bytes block
        "1000000000000000000000000000000000000000000000000000000000000001",
        // Exponent (1 byte)
        "01",
        // Modulus (96 bytes): nop mask
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"
    ))
    .unwrap();

    // Unchanged base
    let expected_output = hex::decode(concat!(
        "1000000000000000000000000000000000000000000000000000000000000001",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "1000000000000000000000000000000000000000000000000000000000000001",
    ))
    .unwrap();

    let encoded_tx = {
        let mint_tx = TxEip2930 {
            chain_id: 37u64,
            nonce: 0,
            gas_price: 1000,
            gas_limit: 1_000_000,
            to: TxKind::Call(modexp_address),
            value: Default::default(),
            input: input_data.into(),
            ..Default::default()
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    let transactions = vec![encoded_tx];

    chain.set_balance(
        B160::from_be_bytes(wallet.address().into_array()),
        U256::from(10u64.pow(18)),
    );

    let result = chain.run_block(transactions, None, None, None);

    // The transaction should succeed
    assert!(
        result.tx_results[0].is_ok(),
        "Modexp transaction should succeed"
    );

    // Extract the result and check it
    let tx_result = result.tx_results[0].as_ref().unwrap();
    assert!(tx_result.is_success(), "Transaction should be successful");

    match &tx_result.execution_result {
        rig::zksync_os_interface::types::ExecutionResult::Success(execution_output) => {
            match execution_output {
                rig::zksync_os_interface::types::ExecutionOutput::Call(result) => {
                    assert_eq!(*result, expected_output)
                }
                rig::zksync_os_interface::types::ExecutionOutput::Create(_, _) => panic!(),
            }
        }
        rig::zksync_os_interface::types::ExecutionResult::Revert(_) => unreachable!(),
    }
}

#[test]
fn test_point_eval_call() {
    let mut chain = Chain::empty(None);
    let wallet = PrivateKeySigner::from_str(
        "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7",
    )
    .unwrap();

    let point_eval_address = address!("000000000000000000000000000000000000000a");

    let input_data = vec![
        1, 102, 133, 225, 114, 167, 73, 247, 66, 106, 69, 37, 154, 47, 12, 166, 56, 32, 114, 250,
        248, 157, 192, 88, 251, 163, 154, 210, 121, 34, 66, 235, 85, 219, 9, 223, 116, 132, 184,
        93, 126, 40, 112, 220, 62, 82, 110, 135, 177, 46, 241, 113, 107, 197, 47, 252, 248, 42,
        160, 119, 67, 165, 212, 245, 18, 209, 170, 150, 140, 245, 200, 141, 68, 162, 165, 129, 82,
        66, 8, 42, 39, 249, 157, 47, 168, 22, 131, 131, 56, 185, 83, 43, 243, 206, 226, 45, 145,
        193, 172, 89, 253, 243, 68, 226, 169, 9, 142, 178, 195, 105, 155, 150, 82, 169, 168, 239,
        192, 6, 196, 189, 168, 161, 215, 100, 180, 160, 250, 218, 60, 52, 231, 42, 12, 196, 209,
        81, 166, 221, 19, 125, 222, 83, 74, 242, 149, 23, 202, 113, 140, 69, 14, 237, 147, 86, 3,
        205, 89, 133, 238, 107, 188, 251, 226, 218, 135, 226, 78, 100, 190, 143, 162, 216, 23, 51,
        224, 222, 155, 138, 17, 239, 215, 199, 63, 57, 137, 141, 21, 143, 208, 196, 134, 126,
    ];

    let expected_output = vec![
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        16, 0, 115, 237, 167, 83, 41, 157, 125, 72, 51, 57, 216, 8, 9, 161, 216, 5, 83, 189, 164,
        2, 255, 254, 91, 254, 255, 255, 255, 255, 0, 0, 0, 1,
    ];

    let encoded_tx = {
        let mint_tx = TxEip2930 {
            chain_id: 37u64,
            nonce: 0,
            gas_price: 1000,
            gas_limit: 1_000_000,
            to: TxKind::Call(point_eval_address),
            value: Default::default(),
            input: input_data.into(),
            ..Default::default()
        };
        rig::utils::sign_and_encode_alloy_tx(mint_tx, &wallet)
    };

    let transactions = vec![encoded_tx];

    chain.set_balance(
        B160::from_be_bytes(wallet.address().into_array()),
        U256::from(10u64.pow(18)),
    );

    let result = chain.run_block(transactions, None, None, None);

    // The transaction should succeed
    assert!(result.tx_results[0].is_ok(), "Transaction should succeed");

    // Extract the result and check it
    let tx_result = result.tx_results[0].as_ref().unwrap();
    assert!(tx_result.is_success(), "Transaction should be successful");

    match &tx_result.execution_result {
        rig::zksync_os_interface::types::ExecutionResult::Success(execution_output) => {
            match execution_output {
                rig::zksync_os_interface::types::ExecutionOutput::Call(result) => {
                    assert_eq!(*result, expected_output)
                }
                rig::zksync_os_interface::types::ExecutionOutput::Create(_, _) => panic!(),
            }
        }
        rig::zksync_os_interface::types::ExecutionResult::Revert(_) => unreachable!(),
    }
}

#[test]
fn test_selfdestruct_to_precompile_gas() {
    // Test that a selfdestruct with a precompile as target doesn't charge for
    // extra warm gas (regression)

    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();

    let contract_address = address!("1000000000000000000000000000000000000001");

    // PUSH20 0x01
    // SELFDESTRUCT
    let bytecode = hex::decode("730000000000000000000000000000000000000001ff").unwrap();

    chain.set_balance(
        B160::from_be_bytes(wallet.address().into_array()),
        U256::from(1_000_000_000_000_000_u64),
    );
    chain.set_evm_bytecode(
        B160::from_be_bytes(contract_address.into_array()),
        &bytecode,
    );

    let encoded_tx = {
        let tx = TxEip2930 {
            chain_id: 37u64,
            nonce: 0,
            gas_price: 1000,
            gas_limit: 75_000,
            to: TxKind::Call(contract_address),
            value: Default::default(),
            input: Default::default(),
            access_list: Default::default(),
        };
        rig::utils::sign_and_encode_alloy_tx(tx, &wallet)
    };

    let result = chain.run_block(vec![encoded_tx], None, None, None);
    let res0 = result.tx_results.first().expect("Must have a tx result");
    assert!(res0.as_ref().is_ok(), "Tx should succeed");
    let gas_used = res0.clone().unwrap().gas_used;
    assert_eq!(gas_used, 26003);
}

#[test]
fn test_reject_caller_with_code_behavior() {
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();

    // Create a contract address with bytecode deployed
    let contract_address = wallet.address();
    let target_address = address!("4242000000000000000000000000000000000000");

    // Deploy bytecode to the contract address to make it a "contract with code"
    chain.set_evm_bytecode(
        B160::from_be_bytes(contract_address.into_array()),
        &hex::decode("60006000f3").unwrap(), // Simple contract: PUSH1 0, PUSH1 0, RETURN
    );

    // Set balance for the contract address
    chain.set_balance(
        B160::from_be_bytes(contract_address.into_array()),
        U256::from(1_000_000_000_000_000_u64),
    );

    let from_contract_tx = {
        let tx = TxEip2930 {
            chain_id: 37u64,
            nonce: 0,
            gas_price: 1000,
            gas_limit: 75_000,
            to: TxKind::Call(target_address),
            value: Default::default(),
            input: Default::default(),
            access_list: Default::default(),
        };
        rig::utils::sign_and_encode_alloy_tx(tx, &wallet)
    };

    let result_simulation = chain.simulate_block(vec![from_contract_tx.clone()], None);

    // In simulation mode, the transaction should succeed
    assert!(result_simulation.tx_results[0].is_ok(),);

    let tx_result = result_simulation.tx_results[0].as_ref().unwrap();
    assert!(
        tx_result.is_success(),
        "Transaction should be successful in simulation mode"
    );

    // But in normal mode it should fail
    let result_normal = chain.run_block(vec![from_contract_tx], None, None, run_config());
    assert!(matches!(
        result_normal.tx_results[0],
        Err(InvalidTransaction::RejectCallerWithCode)
    ));
}

#[test]
fn test_expensive_pubdata() {
    // Test if a transaction can be executed even if the pubdata price is such that
    // validation pubdata requires to use withheld resources.
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();
    let from = wallet.address();
    let target_address = address!("4242000000000000000000000000000000000000");

    // Set balance for the contract address
    chain.set_balance(B160::from_be_bytes(from.into_array()), U256::from(u64::MAX));

    let tx = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 134217728,
            max_priority_fee_per_gas: 134217728,
            gas_limit: 75_000,
            to: TxKind::Call(target_address),
            value: Default::default(),
            input: Default::default(),
            access_list: Default::default(),
        };
        rig::utils::sign_and_encode_alloy_tx(tx, &wallet)
    };

    // Validation uses 40 bytes of pubdata, we want the validation
    // pubdata charge to be > MAX_NATIVE_COMPUTATIONAL (2^35), to
    // ensure we use withheld resources for it.
    let native_price = U256::from(100);
    // Value s.t. (pubdata_price / native_price) * 40 > MAX_NATIVE_COMPUTATIONAL
    let pubdata_price = U256::from(85899346000u64);

    let block_context = BlockContext {
        native_price,
        pubdata_price,
        eip1559_basefee: U256::from(1),
        ..Default::default()
    };
    // Check tx succeeds
    let result = chain.run_block(vec![tx], Some(block_context), None, run_config());
    let res0 = result.tx_results.first().expect("Must have a tx result");
    assert!(res0.as_ref().is_ok(), "Tx should succeed");
}

#[test]
fn test_check_pubdata_encoding_version() {
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();
    let from = wallet.address();
    let target_address = address!("4242000000000000000000000000000000000000");

    // Set balance for the contract address
    chain.set_balance(B160::from_be_bytes(from.into_array()), U256::from(u64::MAX));

    let tx = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 134217728,
            max_priority_fee_per_gas: 134217728,
            gas_limit: 75_000,
            to: TxKind::Call(target_address),
            value: Default::default(),
            input: Default::default(),
            access_list: Default::default(),
        };
        rig::utils::sign_and_encode_alloy_tx(tx, &wallet)
    };

    let native_price = U256::from(100);
    let pubdata_price = U256::from(2);

    let block_context = BlockContext {
        native_price,
        pubdata_price,
        eip1559_basefee: U256::from(1),
        ..Default::default()
    };
    // Check tx succeeds
    let result = chain.run_block(vec![tx], Some(block_context), None, run_config());
    let res0 = result.tx_results.first().expect("Must have a tx result");
    assert!(res0.as_ref().is_ok(), "Tx should succeed");

    assert_eq!(result.pubdata[0], PUBDATA_ENCODING_VERSION);
}

#[test]
fn test_check_pubdata_has_timestamp() {
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();
    let from = wallet.address();
    let target_address = address!("4242000000000000000000000000000000000000");

    // Set balance for the contract address
    chain.set_balance(B160::from_be_bytes(from.into_array()), U256::from(u64::MAX));

    let tx = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 134217728,
            max_priority_fee_per_gas: 134217728,
            gas_limit: 75_000,
            to: TxKind::Call(target_address),
            value: Default::default(),
            input: Default::default(),
            access_list: Default::default(),
        };
        rig::utils::sign_and_encode_alloy_tx(tx, &wallet)
    };

    let native_price = U256::from(100);
    let pubdata_price = U256::from(2);
    let timestamp: u64 = 42;

    let block_context = BlockContext {
        native_price,
        pubdata_price,
        eip1559_basefee: U256::from(1),
        timestamp,
        ..Default::default()
    };
    // Check tx succeeds
    let result = chain.run_block(vec![tx], Some(block_context), None, run_config());
    let res0 = result.tx_results.first().expect("Must have a tx result");
    assert!(res0.as_ref().is_ok(), "Tx should succeed");

    // Pubdata format is [VERSION(1)][BLOCK_HASH(32)][TIMESTAMP(8)][DIFFS...]
    let pubdata_timestamp_bytes = &result.pubdata.as_slice()[33..41];
    let pubdata_timestamp = u64::from_be_bytes(
        pubdata_timestamp_bytes
            .try_into()
            .expect("Slice with incorrect length"),
    );
    assert_eq!(timestamp, pubdata_timestamp, "Timestamps do not match");
}

#[test]
fn test_simple_service_transaction() {
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();
    let from = wallet.address();
    let target_address = L2_INTEROP_ROOT_STORAGE_ADDRESS.to_be_bytes::<20>();

    // Set balance for the contract address
    chain.set_balance(B160::from_be_bytes(from.into_array()), U256::from(u64::MAX));

    let tx = encode_service_tx(500_000, &target_address, &[]);

    let block_context = BlockContext {
        eip1559_basefee: U256::ZERO,
        ..Default::default()
    };
    // Check tx succeeds
    let result = chain.run_block(vec![tx], Some(block_context), None, run_config());
    let res0 = result.tx_results.first().expect("Must have a tx result");
    assert!(res0.as_ref().is_ok(), "Tx should succeed");
}

#[test]
fn test_simple_service_transaction_whitelist() {
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();
    let from = wallet.address();
    // Invalid target
    let target_address = [0u8; 20];

    // Set balance for the contract address
    chain.set_balance(B160::from_be_bytes(from.into_array()), U256::from(u64::MAX));

    let tx = encode_service_tx(500_000, &target_address, &[]);

    let block_context = BlockContext {
        eip1559_basefee: U256::ZERO,
        ..Default::default()
    };
    // Check tx succeeds
    let result = chain.run_block(vec![tx], Some(block_context), None, run_config());
    let res0 = result.tx_results.first().expect("Must have a tx result");
    assert!(res0.as_ref().is_err(), "Tx should fail");
}

#[test]
fn test_service_block_invariants() {
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();
    let from = wallet.address();
    let target_address = L2_INTEROP_ROOT_STORAGE_ADDRESS.to_be_bytes::<20>();

    // Set balance for the contract address
    chain.set_balance(B160::from_be_bytes(from.into_array()), U256::from(u64::MAX));

    // Check that a service block with several service txs works
    let tx1 = encode_service_tx(500_000, &target_address, &[]);
    let tx2 = encode_service_tx(500_000, &target_address, &[]);
    let tx3 = encode_service_tx(500_000, &target_address, &[]);

    let block_context = BlockContext {
        eip1559_basefee: U256::ZERO,
        ..Default::default()
    };
    // Check txs succeed
    let result = chain.run_block(vec![tx1, tx2, tx3], Some(block_context), None, run_config());
    assert!(
        result.tx_results.iter().all(|res| res.is_ok()),
        "All txs should succeed"
    );

    // Check that a service block with a non-service tx fails
    let tx4 = encode_service_tx(500_000, &target_address, &[]);
    let tx_non_service = {
        let tx = TxEip1559 {
            chain_id: 37u64,
            nonce: 0,
            max_fee_per_gas: 134217728,
            max_priority_fee_per_gas: 134217728,
            gas_limit: 75_000,
            to: TxKind::Call(address!("4242000000000000000000000000000000000000")),
            value: Default::default(),
            input: Default::default(),
            access_list: Default::default(),
        };
        rig::utils::sign_and_encode_alloy_tx(tx, &wallet)
    };
    let block_context = BlockContext {
        eip1559_basefee: U256::ZERO,
        ..Default::default()
    };
    chain
        .run_block_no_panic(
            vec![tx4.clone(), tx_non_service.clone()],
            Some(block_context),
            None,
            run_config(),
        )
        .expect_err("Service block with non service tx should fail");

    // Check that a non-service block with a service tx fails
    let block_context = BlockContext {
        eip1559_basefee: U256::ZERO,
        ..Default::default()
    };
    chain
        .run_block_no_panic(
            vec![tx_non_service, tx4],
            Some(block_context),
            None,
            run_config(),
        )
        .expect_err("Service block with non service tx should fail");
}
