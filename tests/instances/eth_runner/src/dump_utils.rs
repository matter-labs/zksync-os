use crate::live_run::rpc;
use alloy_primitives::Address;
use anyhow::Context;
use anyhow::Result;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct AccountStateDiffs {
    pub address: Address,
    pub pre: alloy_rpc_types_eth::Account,
    pub post: alloy_rpc_types_eth::Account,
    pub post_leaf_encoding: String,
}

pub fn dump_eth_block(
    block_number: u64,
    endpoint: &str,
    account_diffs_endpoint: Option<&str>,
    beacon_chain_endpoint: &str,
    block_dir: String,
) -> Result<()> {
    let dir = Path::new(&block_dir);

    let block = rpc::get_block(endpoint, block_number)
        .context(format!("Failed to fetch block for {block_number}"))?;

    if !beacon_chain_endpoint.is_empty() {
        let sidecars =
            rpc::get_blobs_from_beacon_chain(beacon_chain_endpoint, &block.result.header)?;
        serde_json::to_writer(fs::File::create(dir.join("blobs.json"))?, &sidecars)?;
    }

    let prestate = rpc::get_prestate(endpoint, block_number)
        .context(format!("Failed to fetch prestate trace for {block_number}"))?;
    let diff = rpc::get_difftrace(endpoint, block_number)
        .context(format!("Failed to fetch diff trace for {block_number}"))?;
    let receipts = rpc::get_receipts(endpoint, block_number)
        .context(format!("Failed to fetch block receipts for {block_number}"))?;
    let call = rpc::get_calltrace(endpoint, block_number)
        .context(format!("Failed to fetch call trace for {block_number}"))?;
    let witness = rpc::get_witness(endpoint, block_number)
        .context(format!("Failed to fetch witness for {block_number}"))?;
    let block_hashes = rpc::fetch_block_hashes_array(endpoint, block_number)
        .context(format!("Failed to fetch block hashes for {block_number}"))?
        .to_vec();

    use std::fs;
    use std::path::Path;

    // second

    serde_json::to_writer(fs::File::create(dir.join("block.json"))?, &block)?;
    serde_json::to_writer(fs::File::create(dir.join("calltrace.json"))?, &call)?;
    serde_json::to_writer(fs::File::create(dir.join("receipts.json"))?, &receipts)?;
    serde_json::to_writer(fs::File::create(dir.join("prestatetrace.json"))?, &prestate)?;
    serde_json::to_writer(fs::File::create(dir.join("difftrace.json"))?, &diff)?;
    serde_json::to_writer(fs::File::create(dir.join("witness.json"))?, &witness)?;
    serde_json::to_writer(
        fs::File::create(dir.join("block_hashes.json"))?,
        &block_hashes,
    )?;

    let mut account_diffs = vec![];
    if let Some(endpoint) = account_diffs_endpoint {
        for el in witness.result.keys.iter() {
            if el.len() == 20 {
                // potentially interesting account
                let address = Address::try_from(&*el.0).unwrap();
                let Ok((account_proof_pre, _)) =
                    rpc::get_account_proof(endpoint, address, block_number - 1)
                else {
                    continue;
                };
                let Ok((account_proof_post, leaf)) =
                    rpc::get_account_proof(endpoint, address, block_number)
                else {
                    continue;
                };
                let diff = AccountStateDiffs {
                    address,
                    pre: account_proof_pre,
                    post: account_proof_post,
                    post_leaf_encoding: format!("0x{}", hex::encode(&leaf)),
                };
                account_diffs.push(diff);
            }
        }
    }

    serde_json::to_writer(
        fs::File::create(dir.join("account_diffs.json"))?,
        &account_diffs,
    )?;

    Ok(())
}
