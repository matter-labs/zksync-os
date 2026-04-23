#[cfg(test)]
mod tests {
    use std::{io::Read, path::PathBuf, str::FromStr};

    use riscv_transpiler::ir::{
        preprocess_bytecode, DecodingOptions, InstructionName,
    };

    /// Decoder config used to preprocess ZKsync OS binaries when
    /// checking them for unsupported opcodes.
    ///
    /// Mirrors the decoder config the runtime uses in `zksync_os_runner`
    /// so that "supported by the binary checker" matches "decodable by
    /// the simulator." MOP is enabled because `full_statement_verifier`
    /// (linked for Gateway FRI verification) emits `mop.rr.*` via the
    /// Zimop extension, and the airbender `dev` prover supports MOP.
    struct BinaryCheckerDecoderConfig;

    impl DecodingOptions for BinaryCheckerDecoderConfig {
        const SUPPORT_MOP: bool = true;
        const SUPPORT_MUL_DIV: bool = true;
        const SUPPORT_SIGNED_MUL_DIV: bool = false;
        const SUPPORT_SUBWORD_MEM_ACCESS: bool = true;
    }

    fn read_text_section(app_dist_path: &str) -> Vec<u32> {
        let mut binary = vec![];

        let zksync_os_path =
            std::env::var("ZKSYNC_OS_DIR").unwrap_or_else(|_| String::from("../../zksync_os"));
        let file_path = PathBuf::from_str(&zksync_os_path)
            .unwrap()
            .join(app_dist_path);
        let mut file = std::fs::File::open(file_path).unwrap();
        file.read_to_end(&mut binary).unwrap();
        assert!(binary.len() % 4 == 0);

        binary
            .as_chunks()
            .0
            .iter()
            .map(|el| u32::from_le_bytes(*el))
            .collect()
    }

    fn verify_binary(app: &str) {
        let text_section = read_text_section(app);

        // Decode the text section with our MOP-aware decoder options;
        // any slot that ends up as `InstructionName::Illegal` is an
        // opcode the simulator would not recognize.
        //
        // Two classes of `Illegal` decodes are expected and benign:
        //
        //   1. Delegation fillers. The decoder compresses Blake2s /
        //      BigInt / KeccakSpecial5 CSR dispatches (a repeated
        //      opcode) into a single `ZicsrDelegation` instruction and
        //      leaves the N-1 filler slots as `Illegal`. We skip any
        //      `Illegal` whose raw opcode matches the preceding slot.
        //
        //   2. Canonical UNIMP (`csrrw x0, cycle, x0` → `0xc0001073`).
        //      The Rust compiler emits this on `-C panic=abort` to
        //      mark unreachable code paths. It's never executed in a
        //      well-formed run and airbender explicitly treats this
        //      encoding as the canonical UNIMP marker.
        const CANONICAL_UNIMP: u32 = 0xc0001073;

        let instructions = preprocess_bytecode::<BinaryCheckerDecoderConfig>(&text_section);
        let illegal: Vec<(usize, u32)> = instructions
            .iter()
            .enumerate()
            .filter_map(|(pc, instr)| {
                if instr.name != InstructionName::Illegal {
                    return None;
                }
                let opcode = text_section[pc];
                if opcode == CANONICAL_UNIMP {
                    return None;
                }
                if pc > 0 && text_section[pc - 1] == opcode {
                    return None;
                }
                Some((pc, opcode))
            })
            .collect();

        if !illegal.is_empty() {
            for (pc, opcode) in &illegal {
                println!(
                    "Unsupported opcode 0x{:08x} at PC = 0x{:08x}",
                    opcode, pc
                );
            }
            panic!("Unsupported opcodes in binary");
        }
    }

    #[test]
    #[ignore = "runs only on CI / explicit opt-in"]
    fn verify_default_binaries() {
        verify_binary("dist/singleblock_batch/app.text");
        verify_binary("dist/multiblock_batch/app.text")
    }
}
