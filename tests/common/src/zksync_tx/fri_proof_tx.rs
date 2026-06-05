use alloy::{
    eips::{eip2930::AccessList, Encodable2718, Typed2718},
    primitives::{keccak256, Address, Bytes, Signature, B256, U256},
    rlp::{BufMut, Encodable, Header},
    signers::{Signer, SignerSync},
};

/// Signed Gateway FRI proof tx helper for tests.
#[derive(Debug, Clone)]
pub struct ZKsyncFriProofTx {
    pub chain_id: u64,
    pub nonce: u64,
    pub max_priority_fee_per_gas: u128,
    pub max_fee_per_gas: u128,
    pub gas_limit: u64,
    pub to: Address,
    pub value: U256,
    pub input: Bytes,
    pub access_list: AccessList,
    pub statement_versioned_hashes: Vec<B256>,
    pub signature: Signature,
    pub signer: Address,
}

#[derive(Debug, Clone)]
pub struct UnsignedZKsyncFriProofTx {
    pub chain_id: u64,
    pub nonce: u64,
    pub max_priority_fee_per_gas: u128,
    pub max_fee_per_gas: u128,
    pub gas_limit: u64,
    pub to: Address,
    pub value: U256,
    pub input: Bytes,
    pub access_list: AccessList,
    pub statement_versioned_hashes: Vec<B256>,
}

impl UnsignedZKsyncFriProofTx {
    pub const TX_TYPE: u8 = 0x7c;

    pub fn sign<S: Signer + SignerSync<Signature>>(self, signer: S) -> ZKsyncFriProofTx {
        let signing_hash = self.signature_hash();
        let signer_address = signer.address();
        let signature = signer
            .sign_hash_sync(&signing_hash)
            .expect("FRI proof tx signing failed");
        ZKsyncFriProofTx {
            chain_id: self.chain_id,
            nonce: self.nonce,
            max_priority_fee_per_gas: self.max_priority_fee_per_gas,
            max_fee_per_gas: self.max_fee_per_gas,
            gas_limit: self.gas_limit,
            to: self.to,
            value: self.value,
            input: self.input,
            access_list: self.access_list,
            statement_versioned_hashes: self.statement_versioned_hashes,
            signature,
            signer: signer_address,
        }
    }

    pub fn signature_hash(&self) -> B256 {
        let mut encoded = Vec::with_capacity(1 + self.rlp_unsigned_length());
        encoded.put_u8(Self::TX_TYPE);
        self.rlp_encode_unsigned(&mut encoded);
        keccak256(encoded)
    }

    fn rlp_unsigned_length(&self) -> usize {
        Header {
            list: true,
            payload_length: self.rlp_unsigned_payload_length(),
        }
        .length_with_payload()
    }

    fn rlp_unsigned_payload_length(&self) -> usize {
        self.chain_id.length()
            + self.nonce.length()
            + self.max_priority_fee_per_gas.length()
            + self.max_fee_per_gas.length()
            + self.gas_limit.length()
            + self.to.length()
            + self.value.length()
            + self.input.length()
            + self.access_list.length()
            + self.statement_versioned_hashes.length()
    }

    fn rlp_encode_unsigned(&self, out: &mut dyn BufMut) {
        Header {
            list: true,
            payload_length: self.rlp_unsigned_payload_length(),
        }
        .encode(out);
        self.rlp_encode_unsigned_fields(out);
    }

    fn rlp_encode_unsigned_fields(&self, out: &mut dyn BufMut) {
        self.chain_id.encode(out);
        self.nonce.encode(out);
        self.max_priority_fee_per_gas.encode(out);
        self.max_fee_per_gas.encode(out);
        self.gas_limit.encode(out);
        self.to.encode(out);
        self.value.encode(out);
        self.input.encode(out);
        self.access_list.encode(out);
        self.statement_versioned_hashes.encode(out);
    }
}

impl ZKsyncFriProofTx {
    pub const TX_TYPE: u8 = UnsignedZKsyncFriProofTx::TX_TYPE;

    fn unsigned_view(&self) -> UnsignedZKsyncFriProofTx {
        UnsignedZKsyncFriProofTx {
            chain_id: self.chain_id,
            nonce: self.nonce,
            max_priority_fee_per_gas: self.max_priority_fee_per_gas,
            max_fee_per_gas: self.max_fee_per_gas,
            gas_limit: self.gas_limit,
            to: self.to,
            value: self.value,
            input: self.input.clone(),
            access_list: self.access_list.clone(),
            statement_versioned_hashes: self.statement_versioned_hashes.clone(),
        }
    }

    fn rlp_signed_payload_length(&self) -> usize {
        self.unsigned_view().rlp_unsigned_payload_length()
            + self.signature.v().length()
            + self.signature.r().length()
            + self.signature.s().length()
    }

    fn rlp_signed_length(&self) -> usize {
        Header {
            list: true,
            payload_length: self.rlp_signed_payload_length(),
        }
        .length_with_payload()
    }
}

impl Typed2718 for ZKsyncFriProofTx {
    fn ty(&self) -> u8 {
        Self::TX_TYPE
    }
}

impl Encodable2718 for ZKsyncFriProofTx {
    fn encode_2718_len(&self) -> usize {
        1 + self.rlp_signed_length()
    }

    fn encode_2718(&self, out: &mut dyn BufMut) {
        out.put_u8(Self::TX_TYPE);
        Header {
            list: true,
            payload_length: self.rlp_signed_payload_length(),
        }
        .encode(out);
        self.unsigned_view().rlp_encode_unsigned_fields(out);
        self.signature.v().encode(out);
        self.signature.r().encode(out);
        self.signature.s().encode(out);
    }
}
