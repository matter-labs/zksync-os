use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context};

use crate::scenario::ContractDef;

/// Compiled artifact: ABI + bytecode.
#[derive(Debug, Clone)]
pub struct CompiledContract {
    pub abi: alloy::json_abi::JsonAbi,
    pub bytecode: Vec<u8>,
}

/// Compiles all contracts defined in the scenario.
///
/// Uses `forge build` in a temporary foundry project.
pub fn compile_contracts(
    contracts: &HashMap<String, ContractDef>,
    scenario_dir: &Path,
) -> anyhow::Result<HashMap<String, CompiledContract>> {
    // Filter to only contracts that need compilation (have source or file, not raw bytecode).
    let to_compile: HashMap<&String, &ContractDef> = contracts
        .iter()
        .filter(|(_, def)| def.bytecode.is_none())
        .collect();

    if to_compile.is_empty() {
        return Ok(HashMap::new());
    }

    let tmp_dir = tempfile::tempdir().context("failed to create temp dir for compilation")?;
    let project_dir = tmp_dir.path();
    let src_dir = project_dir.join("src");
    std::fs::create_dir_all(&src_dir)?;

    // Write foundry.toml
    std::fs::write(
        project_dir.join("foundry.toml"),
        "[profile.default]\nsrc = \"src\"\nout = \"out\"\nlibs = []\nevm_version = \"cancun\"\n",
    )?;

    // Write each contract source
    for (name, def) in &to_compile {
        validate_contract_name(name)?;

        let source = if let Some(ref inline) = def.source {
            inline.clone()
        } else if let Some(ref file) = def.file {
            let path = scenario_dir.join(file);
            std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read contract file: {}", path.display()))?
        } else {
            bail!("contract '{name}' has neither 'source' nor 'file'");
        };

        let filename = format!("{name}.sol");
        std::fs::write(src_dir.join(&filename), &source)?;
    }

    // Run forge build
    let output = Command::new("forge")
        .arg("build")
        .arg("--force")
        .current_dir(project_dir)
        .output()
        .context("failed to run forge build")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        bail!("forge build failed:\n{stdout}\n{stderr}");
    }

    // Parse artifacts from out/
    let out_dir = project_dir.join("out");
    let mut result = HashMap::new();

    for name in to_compile.keys() {
        let artifact = find_artifact(&out_dir, name)
            .with_context(|| format!("failed to find artifact for contract '{name}'"))?;
        result.insert((*name).clone(), artifact);
    }

    Ok(result)
}

/// Validates that a contract name is safe to use as a filename.
/// Rejects names containing path separators or other unsafe characters
/// to prevent escape from the temp project directory.
fn validate_contract_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty() {
        bail!("contract name must not be empty");
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") || name.contains('\0') {
        bail!(
            "invalid contract name '{name}': must not contain path separators or '..' (Solidity contract names should be alphanumeric)"
        );
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        bail!(
            "invalid contract name '{name}': must only contain ASCII alphanumeric characters or underscore"
        );
    }
    Ok(())
}

/// Finds the forge artifact JSON for a contract name.
fn find_artifact(out_dir: &Path, contract_name: &str) -> anyhow::Result<CompiledContract> {
    // Forge outputs to out/<FileName>.sol/<ContractName>.json
    // We search all subdirs for <ContractName>.json
    let target = format!("{contract_name}.json");

    for entry in walkdir(out_dir)? {
        if entry
            .file_name()
            .is_some_and(|n| n == std::ffi::OsStr::new(&target))
        {
            let content = std::fs::read_to_string(&entry)?;
            return parse_forge_artifact(&content, contract_name);
        }
    }

    bail!(
        "no artifact found for '{contract_name}' in {}",
        out_dir.display()
    );
}

/// Simple recursive directory walk (avoids adding walkdir crate).
fn walkdir(dir: &Path) -> anyhow::Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    if !dir.is_dir() {
        return Ok(files);
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(walkdir(&path)?);
        } else {
            files.push(path);
        }
    }
    Ok(files)
}

/// Parses a forge artifact JSON to extract ABI and bytecode.
fn parse_forge_artifact(json_str: &str, name: &str) -> anyhow::Result<CompiledContract> {
    let artifact: serde_json::Value =
        serde_json::from_str(json_str).context("invalid artifact JSON")?;

    // ABI
    let abi_value = artifact
        .get("abi")
        .with_context(|| format!("no 'abi' in artifact for '{name}'"))?;
    let abi: alloy::json_abi::JsonAbi =
        serde_json::from_value(abi_value.clone()).context("failed to parse ABI")?;

    // Bytecode
    let bytecode_hex = artifact
        .pointer("/bytecode/object")
        .and_then(|v| v.as_str())
        .with_context(|| format!("no bytecode in artifact for '{name}'"))?;

    let bytecode_hex = bytecode_hex.strip_prefix("0x").unwrap_or(bytecode_hex);
    let bytecode = hex::decode(bytecode_hex).context("invalid bytecode hex")?;

    if bytecode.is_empty() {
        bail!("empty bytecode for '{name}' — is it an interface or abstract contract?");
    }

    Ok(CompiledContract { abi, bytecode })
}
