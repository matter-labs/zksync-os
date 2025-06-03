use std::process::ExitCode;

use zksync_error_codegen::arguments::Backend;
use zksync_error_codegen::arguments::GenerationArguments;

const ROOT_ERROR_DEFINITIONS_FROM_ZKSYNC_ERROR: &str = "zksync-error://zksync-root.json";

fn main() -> ExitCode {
    let local_evm_path = "errors/evm.json";
    let local_os_path = "errors/zksync-os.json";

    println!("cargo::rerun-if-changed={local_evm_path}");
    println!("cargo::rerun-if-changed={local_os_path}");

    let root_link = ROOT_ERROR_DEFINITIONS_FROM_ZKSYNC_ERROR;

    let arguments = GenerationArguments {
        verbose: true,
        input_links: vec![
            root_link.into(),
            local_os_path.into(),
            local_evm_path.into(),
        ],
        override_links: vec![],
        outputs: vec![zksync_error_codegen::arguments::BackendOutput {
            output_path: ".".into(),
            backend: Backend::Rust,
            arguments: vec![
                ("use_anyhow".to_owned(), "false".to_owned()),
                ("generate_cargo_toml".to_owned(), "false".to_owned()),
            ],
        }],
    };
    if let Err(e) = zksync_error_codegen::load_and_generate(arguments) {
        println!("{e}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
