//! Small helpers for assembling simple EVM runtime bytecode in tests.
//!
//! This is intentionally not a full assembler. It only covers the tiny subset
//! of patterns that repeatedly show up in the test suite.

use alloy::primitives::Address;

#[derive(Clone, Debug, Default)]
pub struct BytecodeBuilder {
    bytes: Vec<u8>,
}

impl BytecodeBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push0_n(mut self, count: usize) -> Self {
        self.bytes.extend(std::iter::repeat_n(0x5f, count));
        self
    }

    pub fn push0(self) -> Self {
        self.push0_n(1)
    }

    pub fn push_u8(mut self, value: u8) -> Self {
        self.bytes.extend_from_slice(&[0x60, value]);
        self
    }

    pub fn push_u16(mut self, value: u16) -> Self {
        self.bytes.push(0x61);
        self.bytes.extend_from_slice(&value.to_be_bytes());
        self
    }

    pub fn push_bytes(mut self, bytes: &[u8]) -> Self {
        assert!(
            !bytes.is_empty() && bytes.len() <= 32,
            "push_bytes supports 1..=32 bytes"
        );
        self.bytes.push(0x5f + bytes.len() as u8);
        self.bytes.extend_from_slice(bytes);
        self
    }

    pub fn push_address(mut self, address: Address) -> Self {
        self.bytes.push(0x73);
        self.bytes.extend_from_slice(&address.into_array());
        self
    }

    pub fn jumpdest(mut self) -> Self {
        self.bytes.push(0x5b);
        self
    }

    pub fn jump(mut self) -> Self {
        self.bytes.push(0x56);
        self
    }

    pub fn sstore(mut self) -> Self {
        self.bytes.push(0x55);
        self
    }

    pub fn tload(mut self) -> Self {
        self.bytes.push(0x5c);
        self
    }

    pub fn tstore(mut self) -> Self {
        self.bytes.push(0x5d);
        self
    }

    pub fn gas(mut self) -> Self {
        self.bytes.push(0x5a);
        self
    }

    fn push_small(self, value: u8) -> Self {
        if value == 0 {
            self.push0()
        } else {
            self.push_u8(value)
        }
    }

    /// Appends a zero-arg, zero-value `CALL` that forwards the current gas.
    ///
    /// Stack setup emitted:
    /// `out_size=0, out_offset=0, in_size=0, in_offset=0, value=0, callee, gas, CALL`
    pub fn call_simple(self, callee: Address) -> Self {
        self.call_with_gas(callee, 0, 0, 0, 0)
    }

    /// Appends a zero-value `CALL` that forwards the current gas.
    ///
    /// The offsets and sizes are assumed to fit in a single byte because this
    /// helper is only meant for tiny hand-authored test snippets.
    pub fn call_with_gas(
        self,
        callee: Address,
        input_offset: u8,
        input_size: u8,
        output_offset: u8,
        output_size: u8,
    ) -> Self {
        self.push_small(output_size)
            .push_small(output_offset)
            .push_small(input_size)
            .push_small(input_offset)
            .push0()
            .push_address(callee)
            .gas()
            .call()
    }

    pub fn mstore(mut self) -> Self {
        self.bytes.push(0x52);
        self
    }

    pub fn mstore8(mut self) -> Self {
        self.bytes.push(0x53);
        self
    }

    pub fn pop(mut self) -> Self {
        self.bytes.push(0x50);
        self
    }

    pub fn call(mut self) -> Self {
        self.bytes.push(0xf1);
        self
    }

    pub fn return_(mut self) -> Self {
        self.bytes.push(0xf3);
        self
    }

    /// Appends `PUSH0 PUSH0 RETURN`.
    pub fn return_empty(self) -> Self {
        self.push0().push0().return_()
    }

    pub fn revert(mut self) -> Self {
        self.bytes.push(0xfd);
        self
    }

    pub fn invalid(mut self) -> Self {
        self.bytes.push(0xfe);
        self
    }

    pub fn calldatasize(mut self) -> Self {
        self.bytes.push(0x36);
        self
    }

    pub fn eq(mut self) -> Self {
        self.bytes.push(0x14);
        self
    }

    pub fn selfdestruct(mut self) -> Self {
        self.bytes.push(0xff);
        self
    }

    pub fn jumpi(mut self) -> Self {
        self.bytes.push(0x57);
        self
    }

    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

pub fn infinite_loop() -> Vec<u8> {
    BytecodeBuilder::new().jumpdest().push0().jump().finish()
}

pub fn revert() -> Vec<u8> {
    BytecodeBuilder::new().push0().push0().revert().finish()
}

pub fn revert_with_data(data: &[u8]) -> Vec<u8> {
    assert!(
        !data.is_empty() && data.len() <= 32,
        "revert_with_data supports 1..=32 bytes"
    );

    let offset = 32 - data.len() as u8;
    BytecodeBuilder::new()
        .push_bytes(data)
        .push0()
        .mstore()
        .push_u8(data.len() as u8)
        .push_u8(offset)
        .revert()
        .finish()
}

pub fn invalid_opcode() -> Vec<u8> {
    BytecodeBuilder::new().invalid().finish()
}

pub fn return_empty() -> Vec<u8> {
    BytecodeBuilder::new().return_empty().finish()
}

pub fn sstore_u16_then_revert(slot: u8, value: u16) -> Vec<u8> {
    BytecodeBuilder::new()
        .push_u16(value)
        .push_u8(slot)
        .sstore()
        .push0()
        .push0()
        .revert()
        .finish()
}

pub fn tstore_u8_then_revert(slot: u8, value: u8) -> Vec<u8> {
    BytecodeBuilder::new()
        .push_u8(value)
        .push_u8(slot)
        .tstore()
        .push0()
        .push0()
        .revert()
        .finish()
}

pub fn selfdestruct(beneficiary: Address) -> Vec<u8> {
    BytecodeBuilder::new()
        .push_address(beneficiary)
        .selfdestruct()
        .finish()
}

#[cfg(test)]
mod tests {
    use crate::alloy::primitives::address;
    use crate::evm_bytecode;

    use super::BytecodeBuilder;

    #[test]
    fn convenience_helpers_match_expected_bytecode() {
        assert_eq!(evm_bytecode::infinite_loop(), vec![0x5b, 0x5f, 0x56]);
        assert_eq!(evm_bytecode::revert(), vec![0x5f, 0x5f, 0xfd]);
        assert_eq!(evm_bytecode::return_empty(), vec![0x5f, 0x5f, 0xf3]);
        assert_eq!(evm_bytecode::invalid_opcode(), vec![0xfe]);
        assert_eq!(
            evm_bytecode::revert_with_data(&[0xde, 0xad, 0xbe, 0xef]),
            vec![0x63, 0xde, 0xad, 0xbe, 0xef, 0x5f, 0x52, 0x60, 0x04, 0x60, 0x1c, 0xfd]
        );
    }

    #[test]
    fn address_helpers_emit_push20_sequences() {
        let beneficiary = address!("dead000000000000000000000000000000001234");
        assert_eq!(
            evm_bytecode::selfdestruct(beneficiary),
            vec![
                0x73, 0xde, 0xad, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x12, 0x34, 0xff
            ]
        );
    }

    #[test]
    fn builder_supports_small_composed_call_sequences() {
        let inner = address!("0000000000000000000000000000000000000205");
        let bytecode = BytecodeBuilder::new()
            .call_simple(inner)
            .pop()
            .return_empty()
            .finish();

        assert_eq!(
            bytecode,
            vec![
                0x5f, 0x5f, 0x5f, 0x5f, 0x5f, 0x73, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x05, 0x5a, 0xf1,
                0x50, 0x5f, 0x5f, 0xf3
            ]
        );
    }

    #[test]
    fn builder_supports_call_with_io_ranges() {
        let inner = address!("0000000000000000000000000000000000000d11");
        let bytecode = BytecodeBuilder::new()
            .call_with_gas(inner, 0, 1, 0, 0x20)
            .finish();

        assert_eq!(
            bytecode,
            vec![
                0x60, 0x20, 0x5f, 0x60, 0x01, 0x5f, 0x5f, 0x73, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0d, 0x11,
                0x5a, 0xf1
            ]
        );
    }
}
