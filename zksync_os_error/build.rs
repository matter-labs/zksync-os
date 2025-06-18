use std::process::ExitCode;

use zksync_error_codegen::arguments::Backend;
use zksync_error_codegen::arguments::GenerationArguments;

const ROOT_ERROR_DEFINITIONS_FROM_ZKSYNC_ERROR: &str = "zksync-error://zksync-root.json";

fn main() -> ExitCode {
    let local_evm_path = "errors/evm.json";
    let local_os_path = "errors/zksync-os.json";
    let local_error_types_path = "errors/types/zksync-os-specific.json";
    let local_common_types_path = "errors/types/zksync-os-common.json";
    let null_path = "errors/null.json";

    // If one of error description files is changed, rebuild zksync-error
    println!("cargo::rerun-if-changed={local_evm_path}");
    println!("cargo::rerun-if-changed={local_os_path}");
    println!("cargo::rerun-if-changed={local_error_types_path}");

    let arguments = GenerationArguments {
        verbose: true,
        input_links: vec![
            ROOT_ERROR_DEFINITIONS_FROM_ZKSYNC_ERROR.into(),
            local_os_path.into(),
            local_evm_path.into(),
        ],

        mode: zksync_error_codegen::arguments::ResolutionMode::Normal {
            override_links: vec![
                (
                    "zksync-error://types/zksync-specific.json".to_owned(),
                    local_error_types_path.to_owned(),
                ),
                (
                    "zksync-error://types/zksync-os-common.json".to_owned(),
                    local_common_types_path.to_owned(),
                ),
                (
                    r#"{
                    "repo": "matter-labs/anvil-zksync",
                    "branch" : "main",
                    "path" : "etc/errors/anvil.json"
                    }"#
                    .to_owned(),
                    null_path.to_owned(),
                ),
            ],
            lock_file: "zksync-errors.lock".to_owned(),
        },
        outputs: vec![zksync_error_codegen::arguments::BackendOutput {
            output_path: ".".into(),
            backend: Backend::Rust,
            arguments: vec![("generate_cargo_toml".to_owned(), "false".to_owned())],
        }],
    };
    if let Err(e) = zksync_error_codegen::load_and_generate(arguments) {
        println!("{e}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
