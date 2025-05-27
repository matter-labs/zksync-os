use std::ffi::OsStr;
use std::{io::Read, path::Path};

use serde::*;

use crate::parsers::runtime::main_parser::RuntimeParser;
use crate::parsers::verification_time::main_parser::VerificationTimeParser;
use crate::routines::memory::OutputBuffer;
use crate::routines::runtime::host::*;
use crate::routines::runtime::stack_value::*;
use crate::routines::runtime::*;
use crate::routines::verification_time::Validator;
use crate::types::ValueTypeVecRef;

pub mod host_fns;
use self::host_fns::*;

#[derive(Serialize, Deserialize, Clone)]
pub struct TestDescription {
    pub source_filename: String,
    pub commands: Vec<Command>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
#[derive(Clone)]
pub enum Command {
    #[serde(rename = "module")]
    Module { line: u64, filename: String },
    #[serde(rename = "action")]
    Action { line: u64, action: Action },
    #[serde(rename = "assert_return")]
    AssertReturn {
        line: u64,
        action: Action,
        expected: Vec<TypeAndValue>,
    },
    #[serde(rename = "assert_exhaustion")]
    AssertExhaustion { line: u64, action: Action },
    #[serde(rename = "assert_trap")]
    AssertTrap {
        line: u64,
        action: Action,
        text: String,
    },
    #[serde(rename = "assert_malformed")]
    AssertMalformed {
        line: u64,
        filename: String,
        text: String,
        module_type: String,
    },
    #[serde(rename = "assert_invalid")]
    AssertInvalid {
        line: u64,
        filename: String,
        text: String,
        module_type: String,
    },
    #[serde(rename = "assert_uninstantiable")]
    AssertUninstantiable {
        line: u64,
        filename: String,
        text: String,
        module_type: String,
    },
    #[serde(rename = "assert_unlinkable")]
    AssertUnlinkable {
        line: u64,
        filename: String,
        text: String,
        module_type: String,
    },
    #[serde(rename = "register")]
    Register {
        line: u64,
        name: Option<String>,
        r#as: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum Type {
    #[serde(rename = "i32")]
    I32,
    #[serde(rename = "i64")]
    I64,
    #[serde(rename = "f32")]
    F32,
    #[serde(rename = "f64")]
    F64,
    #[serde(rename = "externref")]
    ExternRef,
    #[serde(rename = "funcref")]
    FuncRef,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TypeAndValue {
    #[serde(rename = "type")]
    pub value_type: Type,
    #[serde(rename = "value")]
    pub value_string: String,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
#[derive(Clone)]
pub enum Action {
    #[serde(rename = "invoke")]
    Invoke(ActionInvoke),
    #[serde(rename = "get")]
    Get(ActionGet),
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ActionInvoke {
    #[serde(rename = "field")]
    pub field: String,
    #[serde(rename = "args")]
    pub args: Vec<TypeAndValue>,
    #[serde(rename = "module")]
    pub module: Option<String>,
    #[serde(rename = "text")]
    pub text: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ActionGet {
    #[serde(rename = "module")]
    pub module: Option<String>,
    #[serde(rename = "field")]
    pub field: String,
}

const REFERENCE_TESTS_PATH: &str = "../testsuite/dumps/";

pub fn dump_reference_tests() -> Vec<(String, TestDescription)> {
    dump_reference_tests_at_path(REFERENCE_TESTS_PATH)
}

pub fn dump_reference_tests_at_path(base: &str) -> Vec<(String, TestDescription)> {
    let paths = std::fs::read_dir(base).unwrap();

    let mut paths: Vec<_> = paths.map(|r| r.unwrap()).collect();
    paths.sort_by_key(|dir| dir.path());

    let mut results = vec![];

    for path in paths {
        if let Ok(file_type) = path.file_type() {
            if file_type.is_file() {
                let path = path.path();
                if let Some(ext) = path.extension() {
                    if let Some(ext) = OsStr::to_str(ext) {
                        if ext == "json" {
                            let tests = read_test_description(&path);
                            let filename = path.file_stem().unwrap().to_str().unwrap().to_string();
                            results.push((filename, tests));
                        } else {
                            continue;
                        }
                    }
                }
            }
        }
    }

    results
}

pub fn read_test_description(filename: &Path) -> TestDescription {
    println!("Processing {}", filename.display());
    let file = std::fs::File::open(filename).unwrap();
    serde_json::from_reader(file).unwrap()
}

fn transform_args(args: &[TypeAndValue]) -> Vec<StackValue> {
    let mut result = Vec::with_capacity(args.len());
    for arg in args.iter() {
        match arg.value_type {
            Type::I32 => {
                let value = arg.value_string.parse::<u32>().unwrap();
                let value = StackValue::new_i32(value as i32);
                result.push(value);
            }
            Type::I64 => {
                let value = arg.value_string.parse::<u64>().unwrap();
                let value = StackValue::new_i64(value as i64);
                result.push(value);
            }
            Type::FuncRef => {
                let value = arg.value_string.parse::<u16>().unwrap();
                let value = StackValue::new_funcref(value);
                result.push(value);
            }
            _ => {
                panic!("{:?} not supported", arg);
            }
        }
    }

    result
}

#[ignore = "depends on some testsuite"]
#[test]
fn dump_tests() {
    let _ = dump_reference_tests();
}

fn run_module_imports(all_tests: &[(String, TestDescription)]) {
    for (filename, _descr) in all_tests.iter() {
        if filename == "imports" {
            continue;
        }
        run_module_imports_for_suite(all_tests, filename);
    }
}

fn run_module_imports_for_suite(all_tests: &[(String, TestDescription)], suite_name: &str) {
    for (filename, descr) in all_tests.iter() {
        if filename != suite_name {
            continue;
        }
        for command_idx in 0..descr.commands.len() {
            let command = &descr.commands[command_idx];
            match command {
                Command::Module { line: _, filename } => {
                    if filename == "memory_grow.3.wasm" {
                        println!("Breakpoint");
                    }
                    let file = Path::new(REFERENCE_TESTS_PATH).join(filename);
                    let mut file = std::fs::File::open(file).unwrap();
                    let mut buffer = vec![];
                    file.read_to_end(&mut buffer).unwrap();
                    drop(file);

                    let source = VerificationTimeParser::new(&buffer);
                    let mut memory_manager = ();
                    let res = Validator::parse(source, &mut memory_manager, |x| println!("{}", x));
                    if res.is_ok() {
                        println!("File {} validated successfully", filename);
                    } else {
                        println!("Validation failed for {}", filename);
                        continue;
                    }

                    let (sidetable, func_to_sidetable_mapping) = res.unwrap();
                    let source = RuntimeParser::new(&buffer);
                    let memory_manager = ();
                    let mapping_fn = |idx: u16| func_to_sidetable_mapping[idx as usize];
                    let parsed_module =
                        Interpreter::<_, _, ValueTypeVecRef<'_>, _>::new_from_validated_code(
                            source,
                            &mapping_fn,
                            memory_manager,
                            |x| println!("{}", x),
                        )
                        .unwrap();

                    let mut host = TrivialHost::empty();
                    let mut memory_manager = ();
                    let full_bytecode = parsed_module.full_bytecode;
                    let inst = parsed_module.instantiate_module(&mut host, &mut memory_manager);
                    if inst.is_err() {
                        println!("Could not instantiate for {}", filename);
                        continue;
                    }
                    let mut instance = inst.unwrap();

                    // let mut instance = None;

                    for el in descr.commands[(command_idx + 1)..].iter() {
                        match el {
                            Command::Action { line, action } => {
                                match action {
                                    Action::Invoke(invoke) => {
                                        // let instance = instance.get_or_insert(parsed_module.instantiate_module().unwrap());
                                        let ActionInvoke {
                                            field,
                                            args,
                                            module: _,
                                            text: _,
                                        } = invoke;
                                        println!(
                                            "Running line {}, field {}, expecting to return",
                                            line, field
                                        );
                                        let function_idx =
                                            parsed_module.find_function_idx_by_name(field).unwrap();
                                        let inputs = transform_args(args);
                                        instance.reset();
                                        let _output = instance
                                            .run_function_by_index(
                                                sidetable.by_ref(),
                                                function_idx,
                                                &inputs,
                                                full_bytecode,
                                                &mut host,
                                            )
                                            .unwrap();
                                    }
                                    _ => {
                                        continue;
                                    }
                                }
                            }
                            Command::AssertReturn {
                                line,
                                action,
                                expected,
                            } => {
                                match action {
                                    Action::Invoke(invoke) => {
                                        // let instance = instance.get_or_insert(parsed_module.instantiate_module().unwrap());
                                        let ActionInvoke {
                                            field,
                                            args,
                                            module,
                                            text: _,
                                        } = invoke;
                                        println!(
                                            "Running line {}, field {}, expecting to return values",
                                            line, field
                                        );
                                        if *line == 87 {
                                            println!("Breakpoint");
                                        }
                                        let function_idx =
                                            parsed_module.find_function_idx_by_name(field);
                                        if function_idx.is_err() {
                                            println!("Failed to find a function to invoke for file {}, module {:?}, function {}", filename, module, field);
                                            continue;
                                        }
                                        let function_idx = function_idx.unwrap();
                                        let inputs = transform_args(args);
                                        instance.reset();
                                        let potentially_ignore_returns =
                                            instance.num_imported_functions != 0
                                                || instance.num_imported_globals != 0
                                                || instance.num_imported_tables != 0
                                                || instance.memory_is_imported;
                                        instance
                                            .run_function_by_index(
                                                sidetable.by_ref(),
                                                function_idx,
                                                &inputs,
                                                full_bytecode,
                                                &mut host,
                                            )
                                            .unwrap();
                                        let output = instance.dump_returnvalues();
                                        let expected = transform_args(expected);
                                        if !potentially_ignore_returns {
                                            assert_eq!(&expected, output);
                                        } else if &expected != output {
                                            println!("Output divergence for file {}, module {:?}, function {}", filename, module, field);
                                            println!("Expected: {:?}", &expected);
                                            println!("Received: {:?}", output);
                                        }
                                    }
                                    _ => {
                                        continue;
                                    }
                                }
                            }
                            Command::AssertTrap {
                                line,
                                action,
                                text: _,
                            } => {
                                match action {
                                    Action::Invoke(invoke) => {
                                        // let instance = instance.get_or_insert(parsed_module.instantiate_module().unwrap());
                                        let ActionInvoke {
                                            field,
                                            args,
                                            module: _,
                                            text: _,
                                        } = invoke;
                                        println!(
                                            "Running line {}, field {}, expecting to panic",
                                            line, field
                                        );
                                        let function_idx =
                                            parsed_module.find_function_idx_by_name(field).unwrap();
                                        let inputs = transform_args(args);
                                        instance.reset();
                                        let output = instance.run_function_by_index(
                                            sidetable.by_ref(),
                                            function_idx,
                                            &inputs,
                                            full_bytecode,
                                            &mut host,
                                        );
                                        assert!(output.is_err());
                                    }
                                    _ => {
                                        continue;
                                    }
                                }
                            }
                            _ => break,
                        }
                    }
                }
                Command::AssertInvalid {
                    line: _,
                    filename,
                    text: _,
                    module_type,
                } => {
                    if filename == "exports.81.wasm" {
                        println!("Breakpoint");
                    }

                    if module_type == "binary" {
                        let file = Path::new(REFERENCE_TESTS_PATH).join(filename);
                        let mut file = std::fs::File::open(file).unwrap();
                        let mut buffer = vec![];
                        file.read_to_end(&mut buffer).unwrap();
                        drop(file);

                        let source = VerificationTimeParser::new(&buffer);
                        let mut memory_manager = ();
                        let res =
                            Validator::parse(source, &mut memory_manager, |x| println!("{}", x));
                        if res.is_ok() {
                            panic!("File {} should me invalid, but was imported", filename);
                        } else {
                            println!("File {} successfully rejected", filename);
                        }
                    }
                }
                _ => {
                    continue;
                }
            }
        }
    }
}

// #[test]
// fn run_test() {
//     let all_tests = dump_reference_tests();
//     run_tests_in_module(&all_tests, "i32", "i32.0.wasm");
// }

#[ignore = "depends on some testsuite"]
#[test]
fn test_all_module_imports() {
    let all_tests = dump_reference_tests();
    run_module_imports(&all_tests)
}

#[ignore = "depends on some testsuite"]
#[test]
fn run_imports_in_suite() {
    let all_tests: Vec<(String, TestDescription)> = dump_reference_tests();
    run_module_imports_for_suite(&all_tests, "memory_grow");
}

#[ignore = "depends on some testsuite"]
#[test]
fn run_wasm_contract() {
    use crate::types::ValueType;
    let file = Path::new("./rust_for_contracts")
        .join("target")
        .join("wasm32-unknown-unknown")
        .join("release")
        .join("rust_for_contracts.wasm");
    let mut file = std::fs::File::open(file).unwrap();
    let mut buffer = vec![];
    file.read_to_end(&mut buffer).unwrap();
    drop(file);

    let source = VerificationTimeParser::new(&buffer);
    let mut memory_manager = ();
    let res = Validator::parse(source, &mut memory_manager, |x| println!("{}", x));
    if res.is_ok() {
        println!("Test contract validated successfully");
    } else {
        panic!("Test contract validation failed");
    }

    let (sidetable, func_to_sidetable_mapping) = res.unwrap();
    let source = RuntimeParser::new(&buffer);
    let memory_manager = ();
    let mapping_fn = |idx: u16| func_to_sidetable_mapping[idx as usize];
    let parsed_module = Interpreter::<_, _, ValueTypeVecRef<'_>, _>::new_from_validated_code(
        source,
        &mapping_fn,
        memory_manager,
        |x| println!("{}", x),
    )
    .unwrap();

    use crate::types::FunctionType;

    let mut host = TrivialHost::<Vec<u8>>::default();
    host.add_host_function(
        "env",
        "short_host_op",
        FunctionType {
            inputs: vec![
                ValueType::I32,
                ValueType::I32,
                ValueType::I32,
                ValueType::I32,
            ],
            outputs: vec![ValueType::I32, ValueType::I32],
        },
        short_host_op,
    )
    .unwrap();
    host.add_host_function(
        "env",
        "long_host_op",
        FunctionType {
            inputs: vec![
                ValueType::I32,
                ValueType::I32,
                ValueType::I32,
                ValueType::I32,
                ValueType::I32,
                ValueType::I32,
            ],
            outputs: vec![ValueType::I32, ValueType::I32],
        },
        long_host_op,
    )
    .unwrap();
    let mut calldata = vec![];
    calldata.extend(
        hex::decode("0000000000000000000000000000000000000000ffffffffffffffffffffffff").unwrap(),
    );
    calldata.extend(
        hex::decode("000000000000000000000000000000000000000011111111ffffffffffffffff").unwrap(),
    );
    calldata.extend(
        hex::decode("000000000000000000000000000000000000000011111111ffffffffffffffff").unwrap(),
    );
    calldata.extend(
        hex::decode("0000000000000000000000000000000000000000ffffffffffffffffffffffff").unwrap(),
    );
    assert_eq!(calldata.len(), 128);
    host.set_context(calldata);
    let now = std::time::Instant::now();
    let mut memory_manager = ();
    let inst = parsed_module.instantiate_module(&mut host, &mut memory_manager);
    if inst.is_err() {
        panic!("Could not instantiate for test contract");
    }
    let mut instance = inst.unwrap();
    let full_bytecode = parsed_module.full_bytecode;
    let function_idx = parsed_module
        .find_function_idx_by_name("contract_main_extern")
        .unwrap();
    // let now = std::time::Instant::now();
    let output = instance.run_function_by_index(
        sidetable.by_ref(),
        function_idx,
        &[],
        full_bytecode,
        &mut host,
    );
    dbg!(now.elapsed());
    dbg!(&output);
}
