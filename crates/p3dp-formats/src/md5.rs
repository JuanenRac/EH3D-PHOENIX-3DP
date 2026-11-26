//! The MD5 digest of RFC 1321, written from the text of the RFC. It is used for one thing: the checksum file that sits beside the G-code inside a printer package, so a damaged
//! copy is told. It is not a protection against anybody: MD5 gives none.

const S: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9,
    14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15,
    21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

/// The table of the RFC: the integer part of 2^32 times the absolute sine of each step.
fn table() -> [u32; 64] {
    let mut k = [0u32; 64];
    for (i, v) in k.iter_mut().enumerate() {
        *v = (((i as f64 + 1.0).sin().abs()) * 4_294_967_296.0).floor() as u32;
    }
    k
}

fn block(state: &mut [u32; 4], chunk: &[u8], k: &[u32; 64]) {
    let mut m = [0u32; 16];
    for (i, w) in m.iter_mut().enumerate() {
        *w = u32::from_le_bytes([
            chunk[4 * i],
            chunk[4 * i + 1],
            chunk[4 * i + 2],
            chunk[4 * i + 3],
        ]);
    }
    let [mut a, mut b, mut c, mut d] = *state;
    for (i, (&shift, &constant)) in S.iter().zip(k.iter()).enumerate() {
        let (f, g) = match i / 16 {
            0 => ((b & c) | (!b & d), i),
            1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
            2 => (b ^ c ^ d, (3 * i + 5) % 16),
            _ => (c ^ (b | !d), (7 * i) % 16),
        };
        let sum = a
            .wrapping_add(f)
            .wrapping_add(constant)
            .wrapping_add(m[g])
            .rotate_left(shift);
        a = d;
        d = c;
        c = b;
        b = b.wrapping_add(sum);
    }
    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
}

/// The 16 bytes of the digest of the data.
pub fn md5(data: &[u8]) -> [u8; 16] {
    let k = table();
    let mut state = [0x6745_2301u32, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476];
    let mut tail = Vec::with_capacity(128);
    for chunk in data.chunks(64) {
        if chunk.len() == 64 {
            block(&mut state, chunk, &k);
        } else {
            tail.extend_from_slice(chunk);
        }
    }
    tail.push(0x80);
    while tail.len() % 64 != 56 {
        tail.push(0);
    }
    tail.extend_from_slice(&(data.len() as u64).wrapping_mul(8).to_le_bytes());
    for chunk in tail.chunks(64) {
        block(&mut state, chunk, &k);
    }
    let mut out = [0u8; 16];
    for (i, v) in state.iter().enumerate() {
        out[4 * i..4 * i + 4].copy_from_slice(&v.to_le_bytes());
    }
    out
}

/// The digest as 32 hexadecimal digits in capitals.
pub fn md5_hex(data: &[u8]) -> String {
    md5(data).iter().map(|b| format!("{b:02X}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_test_suite_of_the_rfc() {
        for (text, want) in [
            ("", "d41d8cd98f00b204e9800998ecf8427e"),
            ("a", "0cc175b9c0f1b6a831c399e269772661"),
            ("abc", "900150983cd24fb0d6963f7d28e17f72"),
            ("message digest", "f96b697d7cb7938d525a2f31aaf161d0"),
            (
                "abcdefghijklmnopqrstuvwxyz",
                "c3fcd3d76192e4007dfb496cca67e13b",
            ),
            (
                "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
                "d174ab98d277d9f5a5611c2c9f419d9f",
            ),
            (
                "12345678901234567890123456789012345678901234567890123456789012345678901234567890",
                "57edf4a22be3c955ac49da2e2107b67a",
            ),
        ] {
            assert_eq!(md5_hex(text.as_bytes()), want.to_uppercase(), "{text:?}");
        }
    }

    #[test]
    fn the_lengths_round_the_edge_of_a_block_all_differ() {
        let mut seen: Vec<String> = (50..70).map(|n| md5_hex(&vec![7u8; n])).collect();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), 20);
    }
}
