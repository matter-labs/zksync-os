#![feature(array_chunks)]

#[cfg(test)]
mod tests {
    use std::{io::Read, path::PathBuf, str::FromStr};

    use prover::{cs::machine::Machine, field::Mersenne31Field};

    fn read_text_section() -> Vec<u32> {
        let mut binary = vec![];

        let zksync_os_path =
            std::env::var("ZKSYNC_OS_DIR").unwrap_or_else(|_| String::from("../../zksync_os"));
        let file_path = PathBuf::from_str(&zksync_os_path)
            .unwrap()
            .join("server_app.text");
        let mut file = std::fs::File::open(file_path).unwrap();
        file.read_to_end(&mut binary).unwrap();
        assert!(binary.len() % 4 == 0);

        binary
            .array_chunks()
            .map(|el| u32::from_le_bytes(*el))
            .collect()
    }

    #[test]
    fn verify_default_binary() {
        let text_section = read_text_section();
        type M = prover::cs::machine::machine_configurations::full_isa_with_delegation_no_exceptions_no_signed_mul_div::FullIsaMachineWithDelegationNoExceptionHandlingNoSignedMulDiv;
        let unsupported_opcodes =
            <M as Machine<Mersenne31Field>>::verify_bytecode_base(&text_section);
        if unsupported_opcodes.len() > 0 {
            for (pc, opcode) in unsupported_opcodes.into_iter() {
                println!(
                    "Potentially unsupported opcode 0x{:08x} at PC = 0x{:08x}",
                    opcode, pc
                );
            }
            panic!("Unsupported opcodes in binary");
        }
    }
}
