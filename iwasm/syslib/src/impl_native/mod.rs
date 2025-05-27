pub(crate) mod host_ops;
pub(crate) mod intx;
pub mod system;

use super::*;
use types::uintx::*;

impl<const N: usize> IntX<N, LE>
where
    Assert<{ size_bound(N) }>: IsTrue,
{
    pub fn from_hex(src: &str) -> Self {
        let mut new = IntX::<N, LE>::new();
        let mut src = src.strip_prefix("0x").unwrap_or(src);

        for dst in new.repr.as_u64_le_lsb_limbs_mut() {
            if src.is_empty() {
                break;
            }

            let len = core::cmp::min(src.len(), 16);
            let split_point = src.len() - len;

            let (left, right) = (&src[..split_point], &src[split_point..]);

            let parsed = match u64::from_str_radix(right, 16) {
                Ok(x) => x,
                Err(e) => panic!("Invalid hex literal: {}", e),
            };

            *dst = parsed;
            src = left;
        }

        new
    }
}

impl<const N: usize> IntX<N, BE>
where
    Assert<{ size_bound(N) }>: IsTrue,
{
    pub fn from_hex(src: &str) -> Self {
        let mut new = IntX::<N, BE>::new();
        let mut src = src.strip_prefix("0x").unwrap_or(src);

        for dst in new.repr.as_u64_be_lsb_limbs_mut() {
            if src.is_empty() {
                break;
            }

            let len = core::cmp::min(src.len(), 16);
            let split_point = src.len() - len;

            let (left, right) = (&src[..split_point], &src[split_point..]);

            let parsed = match u64::from_str_radix(right, 16) {
                Ok(x) => x.to_be(),
                Err(e) => panic!("Invalid hex literal: {}", e),
            };

            *dst = parsed;
            src = left;
        }

        new
    }
}

#[cfg(test)]
mod tests {
    use crate::types::ints::{U256, U256BE};

    #[test]
    fn from_hex_be() {
        for i in 0..32 {
            let mut bytes = [0u8; 32];
            let mut chars = ['0'; 64];

            bytes[i] = 0xff;
            chars[i * 2] = 'f';
            chars[i * 2 + 1] = 'f';

            let str = chars.iter().collect::<String>();

            println!("iter: {:>2} - '{str}'", i);

            let int = U256BE::from_hex(str.as_str());

            assert_eq!(bytes, int.as_bytes(), "Unequal bytes.");
            assert_eq!(
                format!("IntX<32, BE>(0x{str})"),
                format!("{:?}", int),
                "Unequal formatting."
            );
        }
    }

    #[test]
    fn from_hex_le() {
        for i in 0..32 {
            let mut chars = ['0'; 64];

            chars[i * 2] = 'f';
            chars[i * 2 + 1] = 'f';

            let str = chars.iter().collect::<String>();

            println!("iter: {:>2} - '{str}'", i);

            let int = U256::from_hex(str.as_str());

            assert_eq!(format!("IntX<32, LE>(0x{str})"), format!("{:?}", int));
        }
    }
}
