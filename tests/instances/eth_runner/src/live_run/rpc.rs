use crate::{
    block::Block,
    calltrace::CallTrace,
    prestate::{DiffTrace, PrestateTrace},
    receipts::BlockReceipts,
};
use alloy::primitives::B256;
use anyhow::{anyhow, Context};
use anyhow::Result;
use rig::log::{debug, warn};
use std::{io::Read, str::FromStr, time::Duration};
use serde_json::json;
use serde::Deserialize;
use serde_json::Deserializer;

// RPC Configuration Constants

/// Number of block hashes to fetch in one batched RPC call in `get_block_hashes_batch`.
/// Each batch is a single HTTP request containing multiple block hash requests.
const BATCH_SIZE: usize = 40;

/// Delay between batch requests in `get_block_hashes_batch` to respect rate limits.
/// Most RPC providers limit to ~50 requests per second. Using 1.1 seconds provides a safety buffer.
const RATE_LIMIT_DELAY_MS: u64 = 1100;

/// Maximum number of retry attempts for failed RPC requests.
const MAX_RETRIES: u32 = 5;

/// Initial delay in milliseconds before retrying a failed RPC request.
/// Subsequent retries use exponential backoff: delay = INITIAL_RETRY_DELAY_MS * (2^attempt).
const INITIAL_RETRY_DELAY_MS: u64 = 100;

/// Converts u64 to hex string with "0x" prefix.
fn to_hex(n: u64) -> String {
    format!("0x{n:x}")
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

/// Fetches multiple block hashes in batched RPC calls.
/// Returns a HashMap mapping block_number -> B256 hash.
pub fn get_block_hashes_batch(endpoint: &str, block_numbers: &[u64]) -> Result<std::collections::HashMap<u64, B256>> {
    if block_numbers.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    let mut all_hashes = std::collections::HashMap::new();
    
    debug!("RPC: get_block_hashes_batch({} blocks) - will be chunked into batches of {}", block_numbers.len(), BATCH_SIZE);
    
    // Process in chunks of BATCH_SIZE to respect rate limits
    let chunks: Vec<_> = block_numbers.chunks(BATCH_SIZE).collect();
    for (chunk_idx, chunk) in chunks.iter().enumerate() {
        debug!("RPC: fetching batch {} of {} ({} block hashes)", chunk_idx + 1, chunks.len(), chunk.len());
        
        // Create a batched JSON-RPC request for this chunk
        let batch: Vec<serde_json::Value> = chunk
            .iter()
            .enumerate()
            .map(|(i, &block_num)| {
                json!({
                    "method": "eth_getBlockByNumber",
                    "params": [to_hex(block_num), false], // false = don't need full block, just header
                    "id": i,
                    "jsonrpc": "2.0"
                })
            })
            .collect();
        
        let response = send(endpoint, json!(batch))?;
        
        // Parse the batched response (array of responses)
        // Use Deserializer with recursion limit disabled to handle large responses
        let mut de = Deserializer::from_str(&response);
        de.disable_recursion_limit();
        let response_value: serde_json::Value = Deserialize::deserialize(&mut de)
            .context(format!("Failed to parse batched RPC response for block hashes. Response length: {} bytes", response.len()))?;
        
        // Check if it's an array (batched response) or a single object (error)
        let responses = if response_value.is_array() {
            response_value.as_array()
                .ok_or_else(|| anyhow!("Failed to parse response as array"))?
                .clone()
        } else {
            return Err(anyhow!("Expected batched response (array), got single response: {}", response_value));
        };
        
        if responses.len() != chunk.len() {
            return Err(anyhow!("Expected {} responses in batch, got {}. Response: {}", chunk.len(), responses.len(), response));
        }
        
        // Build a HashMap by response ID to handle out-of-order responses
        let mut response_map: std::collections::HashMap<usize, serde_json::Value> = std::collections::HashMap::new();
        for resp in responses.into_iter() {
            // Check if it's a valid response object
            if !resp.is_object() {
                return Err(anyhow!("Expected response object, got: {}", resp));
            }
            
            let id = resp.get("id")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| anyhow!("Missing or invalid id in batch response: {}", resp))?;
            
            let id_usize = id as usize;
            if id_usize >= chunk.len() {
                return Err(anyhow!("Response id {} is out of range for chunk size {}", id_usize, chunk.len()));
            }
            
            // Check for errors
            if let Some(error) = resp.get("error") {
                return Err(anyhow!("RPC error in batch response (id={}): {}", id, error));
            }
            
            response_map.insert(id_usize, resp);
        }
        
        // Extract results by index, looking up by ID
        for (i, &block_num) in chunk.iter().enumerate() {
            let resp = response_map.get(&i)
                .ok_or_else(|| anyhow!("Missing response for id {} in batch", i))?;
            
            let result = resp.get("result")
                .ok_or_else(|| anyhow!("Missing result in batch response (id={}). Response object: {}", i, resp))?;
            
            // Extract hash from result
            let hash_hex = result.get("hash")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing hash in result for block number {}", block_num))?;
            
            let hash = B256::from_str(hash_hex)?;
            all_hashes.insert(block_num, hash);
        }
        
        // Add a delay between batches to respect rate limits
        // Only sleep if there are more chunks to process
        if chunk_idx < chunks.len() - 1 {
            std::thread::sleep(Duration::from_millis(RATE_LIMIT_DELAY_MS));
        }
    }
    
    Ok(all_hashes)
}

/// Fetches the chain ID from the RPC endpoint.
pub fn get_chain_id(endpoint: &str) -> Result<u64> {
    debug!("RPC: eth_chainId()");

    let body = json!({
        "method": "eth_chainId",
        "params": [],
        "id": 1,
        "jsonrpc": "2.0"
    });
    let res = send(endpoint, body)?;
    let res: serde_json::Value = serde_json::from_str(&res)?;
    let s = res["result"].as_str().unwrap();
    let hex = s.trim_start_matches("0x");
    let hex = if hex.is_empty() { "0" } else { hex };
    let id = u64::from_str_radix(hex, 16)?;
    Ok(id)
}

/// Decompresses response body based on Content-Encoding header.
/// Handles zstd (manual), gzip (auto-decompressed by ureq), and uncompressed.
fn decompress_response(
    raw_bytes: Vec<u8>,
    content_encoding: &str,
    read_time: std::time::Duration,
) -> Result<Vec<u8>> {
    use std::time::Instant;
    
    if content_encoding.contains("zstd") {
        let compressed_size = raw_bytes.len();
        let decompress_start = Instant::now();
        
        use zstd::stream::Decoder;
        let mut decoder = Decoder::new(&raw_bytes[..])
            .context("Failed to create zstd decoder")?;
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)
            .context("Failed to decompress zstd response")?;
        let decompress_time = decompress_start.elapsed();
        
        let space_saved = if decompressed.len() > 0 {
            (1.0 - compressed_size as f64 / decompressed.len() as f64) * 100.0
        } else {
            0.0
        };
        
        debug!("RPC zstd: read={:.2}ms ({} bytes compressed), decompress={:.2}ms ({} bytes decompressed, {:.1}% saved), total={:.2}ms", 
            read_time.as_secs_f64() * 1000.0,
            compressed_size,
            decompress_time.as_secs_f64() * 1000.0,
            decompressed.len(),
            space_saved,
            (read_time + decompress_time).as_secs_f64() * 1000.0
        );
        
        Ok(decompressed)
    } else {
        // No compression or gzip (ureq auto-decompresses gzip)
        // raw_bytes is already decompressed for gzip, or uncompressed for no compression
        if content_encoding.contains("gzip") {
            let decompressed_size = raw_bytes.len();
            debug!("RPC gzip: read={:.2}ms ({} bytes decompressed, compressed size unknown (chunked), auto-decompressed by ureq)",
                read_time.as_secs_f64() * 1000.0,
                decompressed_size
            );
        } else {
            debug!("RPC read: {:.2}ms ({} bytes, uncompressed)",
                read_time.as_secs_f64() * 1000.0,
                raw_bytes.len()
            );
        }
        Ok(raw_bytes)
    }
}



/// Sends JSON-RPC request to endpoint with retry logic and compression support.
fn send(endpoint: &str, body: serde_json::Value) -> Result<String> {
    use std::time::Instant;
    
    let request_size = serde_json::to_string(&body)?.len();
    let network_start = Instant::now();
    
    // We need to get the content encoding from the response, so we'll handle it differently
    // Make the request and process it in one go
    let mut last_error = None;
    for attempt in 0..=MAX_RETRIES {
        match ureq::post(endpoint)
            .header("Content-Type", "application/json")
            .header("Accept-Encoding", "zstd, gzip")
            .send_json(&body)
        {
            Ok(response) => {
                let network_time = network_start.elapsed();
                
                // Get Content-Encoding header from response
                let content_encoding = response.headers()
                    .get("Content-Encoding")
                    .and_then(|h| h.to_str().ok())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "none".to_string());
                
                // Read response body
                let read_start = Instant::now();
                let body = response.into_body();
                let mut raw_bytes = Vec::new();
                {
                    let mut reader = body.into_reader();
                    reader.read_to_end(&mut raw_bytes)?;
                }
                let read_time = read_start.elapsed();
                
                debug!("RPC raw response: {} bytes, Content-Encoding: '{}'", 
                    raw_bytes.len(), content_encoding
                );
                
                // Decompress if needed
                let decompressed_bytes = decompress_response(raw_bytes, &content_encoding, read_time)?;
                
                // Convert to string
                let out = String::from_utf8(decompressed_bytes)
                    .context("Response is not valid UTF-8 after decompression")?;
                
                let response_size = out.len();
                debug!("RPC network: {:.2}ms (request: {} bytes, response: {} bytes, encoding: {})",
                    network_time.as_secs_f64() * 1000.0,
                    request_size,
                    response_size,
                    content_encoding
                );
                
                return Ok(out);
            }
            Err(e) => {
                last_error = Some(e);
                if attempt < MAX_RETRIES {
                    let delay_ms = INITIAL_RETRY_DELAY_MS * (1 << attempt);
                    warn!("RPC request failed (attempt {}/{}): {}. Retrying in {}ms...", 
                        attempt + 1, 
                        MAX_RETRIES + 1,
                        last_error.as_ref().unwrap(),
                        delay_ms
                    );
                    std::thread::sleep(Duration::from_millis(delay_ms));
                } else {
                    break;
                }
            }
        }
    }
    
    // All retries exhausted
    Err(last_error.unwrap().into())
}

/// Fetches all block traces for a single block in a batched RPC call.
/// 
/// This is a convenience wrapper around `get_all_block_traces_batch()` for single blocks.
/// It's used in error recovery paths and backup endpoint scenarios where we need to fetch
/// a single block's traces. For prefetching multiple blocks, use `get_all_block_traces_batch()`.
pub fn get_all_block_traces(
    endpoint: &str,
    block_number: u64,
) -> Result<(Block, PrestateTrace, DiffTrace, BlockReceipts, CallTrace)> {
    debug!("RPC: get_all_block_traces({block_number}) - batched");
    
    // Use batch function internally for code reuse
    let mut results = get_all_block_traces_batch(endpoint, &[block_number])?;
    
    results.remove(&block_number)
        .ok_or_else(|| anyhow!("Batch function did not return result for block {}", block_number))
}

/// Fetches block traces for multiple blocks in a single batched HTTP request.
/// 
/// Returns a HashMap mapping block_number -> (Block, PrestateTrace, DiffTrace, BlockReceipts, CallTrace).
/// Only includes successfully fetched blocks in the result (failed blocks are skipped with a warning).
pub fn get_all_block_traces_batch(
    endpoint: &str,
    block_numbers: &[u64],
) -> Result<std::collections::HashMap<u64, (Block, PrestateTrace, DiffTrace, BlockReceipts, CallTrace)>> {
    if block_numbers.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    
    debug!("RPC: get_all_block_traces_batch({} blocks) - batched", block_numbers.len());
    
    // Create a batched JSON-RPC request with 5 calls per block
    // ID format: (block_index * 5) + call_type
    // call_type: 0=block, 1=prestate, 2=diff, 3=receipts, 4=call
    let mut batch = Vec::new();
    for (block_idx, &block_number) in block_numbers.iter().enumerate() {
        let block_hex = to_hex(block_number);
        let base_id = block_idx * 5;
        
        batch.push(json!({
            "method": "eth_getBlockByNumber",
            "params": [block_hex.clone(), true],
            "id": base_id + 0,
            "jsonrpc": "2.0"
        }));
        
        batch.push(json!({
            "method": "debug_traceBlockByNumber",
            "params": [block_hex.clone(), { "tracer": "prestateTracer" }],
            "id": base_id + 1,
            "jsonrpc": "2.0"
        }));
        
        batch.push(json!({
            "method": "debug_traceBlockByNumber",
            "params": [block_hex.clone(), {
                "tracer": "prestateTracer",
                "tracerConfig": { "diffMode": true }
            }],
            "id": base_id + 2,
            "jsonrpc": "2.0"
        }));
        
        batch.push(json!({
            "method": "eth_getBlockReceipts",
            "params": [block_hex.clone()],
            "id": base_id + 3,
            "jsonrpc": "2.0"
        }));
        
        batch.push(json!({
            "method": "debug_traceBlockByNumber",
            "params": [block_hex, {
                "tracer": "callTracer",
            }],
            "id": base_id + 4,
            "jsonrpc": "2.0"
        }));
    }
    
    let response = send(endpoint, json!(batch))?;
    
    // Parse the batched response
    // Use Deserializer with recursion limit disabled to handle large responses
    let parse_start = std::time::Instant::now();
    let mut de = Deserializer::from_str(&response);
    de.disable_recursion_limit();
    let response_value: serde_json::Value = Deserialize::deserialize(&mut de)
        .context(format!("Failed to parse batched RPC response. Response length: {} bytes", response.len()))?;
    let parse_time = parse_start.elapsed();
    
    debug!("RPC parse: {:.2}ms (response size: {} bytes)", 
        parse_time.as_secs_f64() * 1000.0,
        response.len()
    );
    
    let responses = if response_value.is_array() {
        response_value.as_array()
            .ok_or_else(|| anyhow!("Failed to parse response as array"))?
            .clone()
    } else {
        return Err(anyhow!("Expected batched response (array), got single response: {}", response_value));
    };
    
    let expected_responses = block_numbers.len() * 5;
    if responses.len() != expected_responses {
        return Err(anyhow!("Expected {} responses in batch, got {}. Response: {}", expected_responses, responses.len(), response));
    }
    
    // Group responses by block
    let group_start = std::time::Instant::now();
    let mut results = std::collections::HashMap::new();
    
    for block_idx in 0..block_numbers.len() {
        let block_number = block_numbers[block_idx];
        let base_id = block_idx * 5;
        
        // Extract results for this block
        let mut block_result = None;
        let mut prestate_result = None;
        let mut diff_result = None;
        let mut receipts_result = None;
        let mut call_result = None;
        
        for resp in &responses {
            if !resp.is_object() {
                continue;
            }
            
            let id = resp.get("id")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| anyhow!("Missing or invalid id in batch response"))?;
            
            // Check if this response belongs to this block
            if id < base_id as u64 || id >= (base_id + 5) as u64 {
                continue;
            }
            
            // Check for errors - skip this block if any call failed
            if let Some(error) = resp.get("error") {
                warn!("RPC error for block {} (id={}): {}", block_number, id, error);
                break;
            }
            
            let result = match resp.get("result") {
                Some(r) => r,
                None => {
                    warn!("Missing result for block {} (id={})", block_number, id);
                    break;
                }
            };
            
            match id as usize - base_id {
                0 => block_result = Some(result.clone()),
                1 => prestate_result = Some(result.clone()),
                2 => diff_result = Some(result.clone()),
                3 => receipts_result = Some(result.clone()),
                4 => call_result = Some(result.clone()),
                _ => {}
            }
        }
        
        // Only add to results if we got all 5 responses
        if let (Some(block_res), Some(prestate_res), Some(diff_res), Some(receipts_res), Some(call_res)) = 
            (block_result, prestate_result, diff_result, receipts_result, call_result) {
            
            // Deserialize each result
            let deserialize_start = std::time::Instant::now();
            let block_json = json!({
                "jsonrpc": "2.0",
                "result": block_res,
                "id": base_id
            });
            let block: Block = serde_json::from_value(block_json)
                .context(format!("Failed to deserialize block for block {}", block_number))?;
            
            let prestate_json = json!({
                "jsonrpc": "2.0",
                "result": prestate_res,
                "id": base_id + 1
            });
            let prestate: PrestateTrace = serde_json::from_value(prestate_json)
                .context(format!("Failed to deserialize prestate for block {}", block_number))?;
            
            let diff_json = json!({
                "jsonrpc": "2.0",
                "result": diff_res,
                "id": base_id + 2
            });
            let diff: DiffTrace = serde_json::from_value(diff_json)
                .context(format!("Failed to deserialize diff for block {}", block_number))?;
            
            let receipts_json = json!({
                "jsonrpc": "2.0",
                "result": receipts_res,
                "id": base_id + 3
            });
            let receipts: BlockReceipts = serde_json::from_value(receipts_json)
                .context(format!("Failed to deserialize receipts for block {}", block_number))?;
            
            // CallTrace needs special handling due to recursion limit
            let call_json = json!({
                "jsonrpc": "2.0",
                "result": call_res,
                "id": base_id + 4
            });
            let call_str = serde_json::to_string(&call_json)?;
            let mut de = Deserializer::from_str(&call_str);
            de.disable_recursion_limit();
            let call: CallTrace = CallTrace::deserialize(&mut de)
                .context(format!("Failed to deserialize call trace for block {}", block_number))?;
            
            let deserialize_time = deserialize_start.elapsed();
            if block_idx == 0 {
                debug!("Block {} deserialize: {:.2}ms", block_number, deserialize_time.as_secs_f64() * 1000.0);
            }
            
            results.insert(block_number, (block, prestate, diff, receipts, call));
        } else {
            warn!("Failed to fetch all traces for block {}, skipping", block_number);
        }
    }
    
    let group_time = group_start.elapsed();
    debug!("RPC group/deserialize: {:.2}ms ({} blocks)", 
        group_time.as_secs_f64() * 1000.0,
        block_numbers.len()
    );
    
    Ok(results)
}
