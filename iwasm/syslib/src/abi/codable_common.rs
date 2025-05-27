use super::*;

impl ABICodableCommon for () {
    fn is_dynamic() -> bool {
        false
    }
    fn head_encoding_size() -> u32 {
        32
    }
}

impl ABIDecodableBase for () {
    fn read<I: DecoderInterface>(_interface: &mut I) -> Result<Self, ()> {
        Ok(())
    }
}

impl ABIEncodableBase for () {
    fn write<B: EncoderInterface>(&self, _interface: &mut B) -> Result<u32, ()> {
        Ok(0)
    }
}

impl ABICodableCommon for bool {
    fn is_dynamic() -> bool {
        false
    }
    fn head_encoding_size() -> u32 {
        32
    }
}

impl ABIDecodableBase for bool {
    fn read<I: DecoderInterface>(interface: &mut I) -> Result<Self, ()> {
        for _ in 0..7 {
            let el = interface.read_u32()?;
            if el != 0 {
                return Err(());
            }
        }
        let value = interface.read_u32()?;
        match value {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(()),
        }
    }
}

impl ABIEncodableBase for bool {
    fn write<B: EncoderInterface>(&self, interface: &mut B) -> Result<u32, ()> {
        for _ in 0..7 {
            interface.write_u32(0)?;
        }
        interface.write_u32(*self as u32)?;
        Ok(32)
    }
}

impl ABICodableCommon for u8 {
    fn is_dynamic() -> bool {
        false
    }
    fn head_encoding_size() -> u32 {
        32
    }
}

impl ABIDecodableBase for u8 {
    fn read<I: DecoderInterface>(interface: &mut I) -> Result<Self, ()> {
        for _ in 0..7 {
            let el = interface.read_u32()?;
            if el != 0 {
                return Err(());
            }
        }
        let value = interface.read_u32()?;
        const BOUND: u32 = 1u32 << 8;
        match value {
            0..BOUND => Ok(value as u8),
            BOUND.. => Err(()),
        }
    }
}

impl ABIEncodableBase for u8 {
    fn write<B: EncoderInterface>(&self, interface: &mut B) -> Result<u32, ()> {
        for _ in 0..7 {
            interface.write_u32(0)?;
        }
        interface.write_u32(*self as u32)?;
        Ok(32)
    }
}

impl ABICodableCommon for u32 {
    fn is_dynamic() -> bool {
        false
    }
    fn head_encoding_size() -> u32 {
        32
    }
}

impl ABIDecodableBase for u32 {
    fn read<I: DecoderInterface>(interface: &mut I) -> Result<Self, ()> {
        for _ in 0..7 {
            let el = interface.read_u32()?;
            if el != 0 {
                return Err(());
            }
        }
        let value = interface.read_u32()?;
        Ok(value)
    }
}

impl ABIEncodableBase for u32 {
    fn write<B: EncoderInterface>(&self, interface: &mut B) -> Result<u32, ()> {
        for _ in 0..7 {
            interface.write_u32(0)?;
        }
        interface.write_u32(*self)?;
        Ok(32)
    }
}

impl ABICodableCommon for u64 {
    fn is_dynamic() -> bool {
        false
    }
    fn head_encoding_size() -> u32 {
        32
    }
}

impl ABIDecodableBase for u64 {
    fn read<I: DecoderInterface>(interface: &mut I) -> Result<Self, ()> {
        for _ in 0..6 {
            let el = interface.read_u32()?;
            if el != 0 {
                return Err(());
            }
        }
        let high = interface.read_u32()?;
        let low = interface.read_u32()?;
        Ok(((high as u64) << 32) | (low as u64))
    }
}

impl ABIEncodableBase for u64 {
    fn write<B: EncoderInterface>(&self, interface: &mut B) -> Result<u32, ()> {
        for _ in 0..6 {
            interface.write_u32(0)?;
        }
        interface.write_u32((*self >> 32) as u32)?;
        interface.write_u32(*self as u32)?;
        Ok(32)
    }
}
