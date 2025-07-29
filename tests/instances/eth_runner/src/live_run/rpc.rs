use crate::{
    block::Block,
    calltrace::CallTrace,
    prestate::{DiffTrace, PrestateTrace},
    receipts::BlockReceipts,
};
use alloy::primitives::B256;
use anyhow::anyhow;
use anyhow::Result;
use rig::log::debug;
use std::{io::Read, str::FromStr};
use ureq::json;

/// Converts u64 to hex string with "0x" prefix.
fn to_hex(n: u64) -> String {
    format!("0x{n:x}")
}

/// Fetches the full block data with transactions.
pub fn get_block(endpoint: &str, block_number: u64) -> Result<Block> {
    debug!("RPC: get_block({block_number})");
    let body = json!({
        "method": "eth_getBlockByNumber",
        "params": [to_hex(block_number), true],
        "id": 1,
        "jsonrpc": "2.0"
    });
    let res = send(endpoint, body)?;
    let block = serde_json::from_str(&res)?;
    Ok(block)
}

/// Fetches the block hash.
pub fn get_block_hash(endpoint: &str, block_number: u64) -> Result<B256> {
    debug!("RPC: get_block_hash({block_number})");

    let body = json!({
        "method": "eth_getBlockByNumber",
        "params": [to_hex(block_number), true],
        "id": 1,
        "jsonrpc": "2.0"
    });
    let res = send(endpoint, body)?;
    let res: serde_json::Value = serde_json::from_str(&res)?;
    let hash_hex = res["result"]["hash"]
        .as_str()
        .ok_or_else(|| anyhow!("No block hash found in response"))?;
    let hash = B256::from_str(hash_hex)?;
    Ok(hash)
}

/// Fetches the block receipts.
pub fn get_receipts(endpoint: &str, block_number: u64) -> Result<BlockReceipts> {
    debug!("RPC: get_receipts({block_number})");
    let body = json!({
        "method": "eth_getBlockReceipts",
        "params": [to_hex(block_number)],
        "id": 1,
        "jsonrpc": "2.0"
    });
    let res = send(endpoint, body)?;
    let v = serde_json::from_str(&res)?;
    Ok(v)
}

/// Fetches the prestate trace.
pub fn get_prestate(endpoint: &str, block_number: u64) -> Result<PrestateTrace> {
    debug!("RPC: get_prestate({block_number})");
    let body = json!({
        "method": "debug_traceBlockByNumber",
        "params": [to_hex(block_number), { "tracer": "prestateTracer" }],
        "id": 1,
        "jsonrpc": "2.0"
    });
    let res = send(endpoint, body)?;
    let v = serde_json::from_str(&res)?;
    Ok(v)
}

/// Fetches the diff trace.
pub fn get_difftrace(endpoint: &str, block_number: u64) -> Result<DiffTrace> {
    debug!("RPC: get_difftrace({block_number})");
    let body = json!({
        "method": "debug_traceBlockByNumber",
        "params": [to_hex(block_number), {
            "tracer": "prestateTracer",
            "tracerConfig": { "diffMode": true }
        }],
        "id": 1,
        "jsonrpc": "2.0"
    });
    let res = send(endpoint, body)?;
    let v = serde_json::from_str(&res)?;
    Ok(v)
}

pub fn get_calltrace(endpoint: &str, block_number: u64) -> Result<CallTrace> {
    debug!("RPC: get_calltrace({block_number})");
    use serde::Deserialize;
    use serde_json::Deserializer;

    let body = json!({
        "method": "debug_traceBlockByNumber",
        "params": [to_hex(block_number), {
            "tracer": "callTracer",
        }],
        "id": 1,
        "jsonrpc": "2.0"
    });
    let res = send(endpoint, body)?;

    let mut de = Deserializer::from_str(&res);
    de.disable_recursion_limit();

    let calltrace = CallTrace::deserialize(&mut de)?;
    Ok(calltrace)
}

fn send(endpoint: &str, body: serde_json::Value) -> Result<String> {
    let response = ureq::post(endpoint)
        .set("Content-Type", "application/json")
        .send_json(body)?;

    let mut out = String::new();
    response.into_reader().read_to_string(&mut out)?;
    Ok(out)
}
