// Helpers to manually encode RLP items and lists, for negative tests

pub(crate) fn rlp_bytes(b: &[u8]) -> Vec<u8> {
    match b.len() {
        0 => vec![0x80],
        1 if b[0] < 0x80 => vec![b[0]],
        n if n <= 55 => {
            let mut v = Vec::with_capacity(1 + n);
            v.push(0x80 + n as u8);
            v.extend_from_slice(b);
            v
        }
        n => {
            let len = (n as u64).to_be_bytes();
            let i = len.iter().position(|&x| x != 0).unwrap_or(len.len() - 1);
            let len_bytes = &len[i..];
            let mut v = Vec::with_capacity(1 + len_bytes.len() + n);
            v.push(0xb7 + len_bytes.len() as u8);
            v.extend_from_slice(len_bytes);
            v.extend_from_slice(b);
            v
        }
    }
}

pub(crate) fn rlp_list(items: &[Vec<u8>]) -> Vec<u8> {
    let payload_len: usize = items.iter().map(|x| x.len()).sum();
    if payload_len <= 55 {
        let mut v = Vec::with_capacity(1 + payload_len);
        v.push(0xc0 + payload_len as u8);
        for it in items {
            v.extend_from_slice(it);
        }
        v
    } else {
        let len = (payload_len as u64).to_be_bytes();
        let i = len.iter().position(|&x| x != 0).unwrap_or(len.len() - 1);
        let len_bytes = &len[i..];
        let mut v = Vec::with_capacity(1 + len_bytes.len() + payload_len);
        v.push(0xf7 + len_bytes.len() as u8);
        v.extend_from_slice(len_bytes);
        for it in items {
            v.extend_from_slice(it);
        }
        v
    }
}

pub(crate) fn rlp_uint(mut n: u128) -> Vec<u8> {
    if n == 0 {
        return vec![0x80]; // canonical zero
    }
    let mut buf = [0u8; 16];
    for i in (0..16).rev() {
        buf[i] = (n & 0xff) as u8;
        n >>= 8;
    }
    let first = buf.iter().position(|&b| b != 0).unwrap();
    rlp_bytes(&buf[first..])
}
