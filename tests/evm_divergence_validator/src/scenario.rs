use std::collections::HashMap;

use serde::Deserialize;

/// Top-level scenario file.
#[derive(Debug, Deserialize)]
pub struct Scenario {
    /// Solidity contracts: name -> source or path.
    #[serde(default)]
    pub contracts: HashMap<String, ContractDef>,

    /// Named accounts with pre-funded balances.
    #[serde(default)]
    pub accounts: HashMap<String, AccountDef>,

    /// Optional block context overrides.
    #[serde(default)]
    pub block: Option<BlockDef>,

    /// Transaction steps to execute in a single block.
    pub steps: Vec<Step>,
}

#[derive(Debug, Deserialize)]
pub struct ContractDef {
    /// Inline Solidity source code.
    #[serde(default)]
    pub source: Option<String>,

    /// Path to a .sol file (relative to scenario file).
    #[serde(default)]
    pub file: Option<String>,

    /// Raw hex runtime bytecode to predeploy at `address`.
    /// When set, the contract is injected into state before execution
    /// (no CREATE transaction needed). Requires `address`.
    #[serde(default)]
    pub bytecode: Option<String>,

    /// Address to predeploy the contract at. Required when `bytecode` is set.
    #[serde(default)]
    pub address: Option<String>,

    /// Solidity compiler version (e.g. "0.8.26"). Uses forge default if omitted.
    #[serde(default)]
    #[allow(dead_code)]
    pub solc: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AccountDef {
    /// Balance in wei (decimal or hex string).
    pub balance: String,
}

#[derive(Debug, Deserialize)]
pub struct BlockDef {
    #[serde(default)]
    pub basefee: Option<u128>,
    #[serde(default)]
    pub gas_limit: Option<u64>,
    #[serde(default)]
    pub timestamp: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum Step {
    #[serde(rename = "deploy")]
    Deploy {
        /// Contract name (must be a key in `contracts`).
        contract: String,
        /// Account name for the sender.
        from: String,
        /// Constructor arguments as JSON values.
        #[serde(default)]
        args: Vec<serde_json::Value>,
        /// Gas limit override.
        #[serde(default)]
        gas: Option<u64>,
        /// ETH value to send (decimal wei).
        #[serde(default)]
        value: Option<String>,
    },
    #[serde(rename = "call")]
    Call {
        /// Target: either a named account/contract address, or "$deployed:N"
        /// referencing the Nth deploy step's resulting address.
        to: String,
        /// Account name for the sender.
        from: String,
        /// Function signature, e.g. "transfer(address,uint256)".
        function: String,
        /// Function arguments as JSON values.
        #[serde(default)]
        args: Vec<serde_json::Value>,
        /// Gas limit override.
        #[serde(default)]
        gas: Option<u64>,
        /// ETH value to send (decimal wei).
        #[serde(default)]
        value: Option<String>,
    },
    /// Send raw bytecode/calldata — for cases unreachable via Solidity.
    #[serde(rename = "send_raw")]
    SendRaw {
        /// Target address (hex, named account, or "$deployed:N"). Omit for CREATE.
        #[serde(default)]
        to: Option<String>,
        /// Account name for the sender.
        from: String,
        /// Raw hex data (deployment bytecode or calldata).
        #[serde(default)]
        data: Option<String>,
        /// Gas limit override.
        #[serde(default)]
        gas: Option<u64>,
        /// ETH value to send (decimal wei).
        #[serde(default)]
        value: Option<String>,
    },
}
