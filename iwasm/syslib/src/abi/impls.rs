use super::*;

impl<T: ABICodableCommon> ABICodableCommon for Vec<T> {
    fn is_dynamic() -> bool {
        true
    }

    fn head_encoding_size() -> u32 {
        32
    }
}

impl<T: ABIDecodable> ABIDecodable for Vec<T> {
    fn read<I: DecoderInterface>(interface: &mut I) -> Result<Self, ()> {
        let len = <u32 as ABIDecodableBase>::read(interface)?;
        let mut result = Vec::with_capacity(len as usize);
        let mut decoder = interface.create_sequence_decoder::<T>(len)?;
        for _ in 0..len {
            let element = decoder.decode_field::<T>()?;
            result.push(element);
        }

        Ok(result)
    }
}

impl<T: ABIEncodable> ABIEncodable for Vec<T> {
    fn full_encoding_size(&self) -> u32 {
        let mut total = 32 + 32;
        if T::is_dynamic() {
            total += self.len() as u32 * 32;
        }
        for el in self.iter() {
            total += el.full_encoding_size();
        }

        total
    }
    fn write<I: EncoderInterface>(&self, interface: &mut I) -> Result<u32, ()> {
        // NOTE: we only encode "body" and require that encoder knows how to encode the head
        // of dynamic structures
        let mut total_written = 0u32;
        let len = self.len() as u32;
        // println!("Encoding len of {} for {:?}", len, std::any::type_name::<Self>());
        total_written += interface.encode_field(&len)?;
        // now we formally have a different structure that is like a tuple, but we only now realized
        // what is its length
        let mut subencoder = interface.create_sequence_encoder::<T>(len)?;
        for el in self.iter() {
            total_written += subencoder.encode_field(el)?;
        }

        Ok(total_written)
    }
}

impl<T: ABICodableCommon, const N: usize> ABICodableCommon for [T; N] {
    fn is_dynamic() -> bool {
        T::is_dynamic()
    }

    fn head_encoding_size() -> u32 {
        if Self::is_dynamic() {
            32 * (N as u32)
        } else {
            T::head_encoding_size() * (N as u32)
        }
    }
}

impl<T: ABIEncodable, const N: usize> ABIEncodable for [T; N] {
    fn full_encoding_size(&self) -> u32 {
        if T::is_dynamic() {
            let mut total = 32 * (N as u32);
            for el in self.iter() {
                total += el.full_encoding_size();
            }

            total
        } else {
            T::head_encoding_size() * (N as u32)
        }
    }
    fn write<I: EncoderInterface>(&self, interface: &mut I) -> Result<u32, ()> {
        let mut total_written = 0u32;
        for el in self.iter() {
            total_written += interface.encode_field(el)?;
        }

        Ok(total_written)
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod test {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct TokenTransferData {
        address: u64,
        amount: u32,
    }

    impl ABICodableCommon for TokenTransferData {
        fn is_dynamic() -> bool {
            <u64 as ABICodableCommon>::is_dynamic() || <u32 as ABICodableCommon>::is_dynamic()
        }
        fn head_encoding_size() -> u32 {
            <u64 as ABICodableCommon>::head_encoding_size()
                + <u32 as ABICodableCommon>::head_encoding_size()
        }
    }

    impl ABIEncodable for TokenTransferData {
        fn full_encoding_size(&self) -> u32 {
            <u64 as ABIEncodable>::full_encoding_size(&self.address)
                + <u32 as ABIEncodable>::full_encoding_size(&self.amount)
        }
        fn write<I: EncoderInterface>(&self, interface: &mut I) -> Result<u32, ()> {
            let mut total_written = 0;
            total_written += interface.encode_field(&self.address)?;
            total_written += interface.encode_field(&self.amount)?;

            Ok(total_written)
        }
    }

    impl ABIDecodable for TokenTransferData {
        fn read<I: DecoderInterface>(interface: &mut I) -> Result<Self, ()> {
            let address = interface.decode_field()?;
            let amount = interface.decode_field()?;

            let new = Self { address, amount };

            Ok(new)
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct TokenTransferData2 {
        address: u64,
        amount: Vec<u32>,
    }

    impl ABICodableCommon for TokenTransferData2 {
        fn is_dynamic() -> bool {
            <u64 as ABICodableCommon>::is_dynamic() || <Vec<u32> as ABICodableCommon>::is_dynamic()
        }
        fn head_encoding_size() -> u32 {
            <u64 as ABICodableCommon>::head_encoding_size()
                + <Vec<u32> as ABICodableCommon>::head_encoding_size()
        }
    }

    impl ABIEncodable for TokenTransferData2 {
        fn full_encoding_size(&self) -> u32 {
            <u64 as ABIEncodable>::full_encoding_size(&self.address)
                + <Vec<u32> as ABIEncodable>::full_encoding_size(&self.amount)
        }
        fn write<I: EncoderInterface>(&self, interface: &mut I) -> Result<u32, ()> {
            let mut total_written = 0;
            total_written += interface.encode_field(&self.address)?;
            total_written += interface.encode_field(&self.amount)?;

            Ok(total_written)
        }
    }

    impl ABIDecodable for TokenTransferData2 {
        fn read<I: DecoderInterface>(interface: &mut I) -> Result<Self, ()> {
            let address = interface.decode_field()?;
            let amount = interface.decode_field()?;

            let new = Self { address, amount };

            Ok(new)
        }
    }

    // #[test]
    // fn test_trivial() {
    //     let transfer = TokenTransferData {
    //         amount: u32::MAX,
    //         address: 1234567,
    //     };
    //
    //     let result = abi_encode_to_vec(&transfer).unwrap();
    //     assert_eq!(result.len(), 64);
    //     dbg!(hex::encode(&result));
    //     let decoded = abi_encode_from_bytes::<TokenTransferData>(&result).unwrap();
    //     assert_eq!(decoded, transfer)
    // }
    //
    // #[test]
    // fn test_dynamic_trivial() {
    //     let transfer = TokenTransferData2 {
    //         amount: vec![u32::MAX, 1],
    //         address: 1234567,
    //     };
    //
    //     let result = abi_encode_to_vec(&transfer).unwrap();
    //     assert_eq!(result.len(), 160);
    //     dbg!(hex::encode(&result));
    //     let decoded = abi_encode_from_bytes::<TokenTransferData2>(&result).unwrap();
    //     assert_eq!(decoded, transfer)
    // }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct VecOfStatic {
        elements: Vec<TokenTransferData>,
    }

    impl ABICodableCommon for VecOfStatic {
        fn is_dynamic() -> bool {
            <Vec<TokenTransferData> as ABICodableCommon>::is_dynamic()
        }
        fn head_encoding_size() -> u32 {
            <Vec<TokenTransferData> as ABICodableCommon>::head_encoding_size()
        }
    }

    impl ABIEncodable for VecOfStatic {
        fn full_encoding_size(&self) -> u32 {
            <Vec<TokenTransferData> as ABIEncodable>::full_encoding_size(&self.elements)
        }
        fn write<I: EncoderInterface>(&self, interface: &mut I) -> Result<u32, ()> {
            let mut total_written = 0;
            total_written += interface.encode_field(&self.elements)?;

            Ok(total_written)
        }
    }

    impl ABIDecodable for VecOfStatic {
        fn read<I: DecoderInterface>(interface: &mut I) -> Result<Self, ()> {
            let elements = interface.decode_field()?;

            let new = Self { elements };

            Ok(new)
        }
    }

    // #[test]
    // fn test_vec_of_static() {
    //     let transfer = TokenTransferData {
    //         amount: u32::MAX,
    //         address: 1234567,
    //     };
    //
    //     let vec = VecOfStatic {
    //         elements: vec![transfer],
    //     };
    //
    //     let result = abi_encode_to_vec(&vec).unwrap();
    //     dbg!(hex::encode(&result));
    //     assert_eq!(result.len(), 128);
    //     let decoded = abi_encode_from_bytes::<VecOfStatic>(&result).unwrap();
    //     assert_eq!(decoded, vec);
    // }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct VecOfDynamic {
        elements: Vec<TokenTransferData2>,
    }

    impl ABICodableCommon for VecOfDynamic {
        fn is_dynamic() -> bool {
            <Vec<TokenTransferData2> as ABICodableCommon>::is_dynamic()
        }
        fn head_encoding_size() -> u32 {
            <Vec<TokenTransferData2> as ABICodableCommon>::head_encoding_size()
        }
    }

    impl ABIEncodable for VecOfDynamic {
        fn full_encoding_size(&self) -> u32 {
            <Vec<TokenTransferData2> as ABIEncodable>::full_encoding_size(&self.elements)
        }
        fn write<I: EncoderInterface>(&self, interface: &mut I) -> Result<u32, ()> {
            let mut total_written = 0;
            total_written += interface.encode_field(&self.elements)?;

            Ok(total_written)
        }
    }

    impl ABIDecodable for VecOfDynamic {
        fn read<I: DecoderInterface>(interface: &mut I) -> Result<Self, ()> {
            let elements = interface.decode_field()?;

            let new = Self { elements };

            Ok(new)
        }
    }

    // #[test]
    // fn test_vec_of_dynamic() {
    //     let transfer = TokenTransferData2 {
    //         address: 1234567,
    //         amount: vec![u32::MAX, 1],
    //     };
    //
    //     let vec = VecOfDynamic {
    //         elements: vec![transfer],
    //     };
    //
    //     let result = abi_encode_to_vec(&vec).unwrap();
    //     dbg!(hex::encode(&result));
    //     assert_eq!(result.len(), 256);
    //     let decoded = abi_encode_from_bytes::<VecOfDynamic>(&result).unwrap();
    //     assert_eq!(decoded, vec);
    // }
}
