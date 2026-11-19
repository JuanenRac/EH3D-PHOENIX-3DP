//! Inflate (RFC 1951) written from the specification: stored, fixed and dynamic blocks. The size of the output is limited, so a bomb is refused.

const MAX_BITS: usize = 15;
const LEN_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEN_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
const CL_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    buf: u32,
    cnt: u32,
}

impl Bits<'_> {
    fn bits(&mut self, need: u32) -> Result<u32, String> {
        let mut val = self.buf;
        while self.cnt < need {
            let Some(&b) = self.data.get(self.pos) else {
                return Err("the data ends before the end of the stream".to_string());
            };
            self.pos += 1;
            val |= u32::from(b) << self.cnt;
            self.cnt += 8;
        }
        self.buf = val >> need;
        self.cnt -= need;
        Ok(val & ((1u32 << need) - 1))
    }
}

struct Huffman {
    count: [u16; MAX_BITS + 1],
    symbol: Vec<u16>,
}

impl Huffman {
    fn new(lengths: &[u8]) -> Result<Huffman, String> {
        let mut count = [0u16; MAX_BITS + 1];
        for &l in lengths {
            count[l as usize] += 1;
        }
        let mut left: i32 = 1;
        for &c in &count[1..] {
            left <<= 1;
            left -= i32::from(c);
            if left < 0 {
                return Err("over-subscribed code lengths".to_string());
            }
        }
        let mut offs = [0u16; MAX_BITS + 1];
        for len in 1..MAX_BITS {
            offs[len + 1] = offs[len] + count[len];
        }
        let mut symbol = vec![0u16; lengths.len()];
        for (sym, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbol[offs[l as usize] as usize] = sym as u16;
                offs[l as usize] += 1;
            }
        }
        Ok(Huffman { count, symbol })
    }

    fn decode(&self, b: &mut Bits) -> Result<u16, String> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..=MAX_BITS {
            code |= b.bits(1)? as i32;
            let count = i32::from(self.count[len]);
            if code - count < first {
                return self
                    .symbol
                    .get((index + (code - first)) as usize)
                    .copied()
                    .ok_or_else(|| "a code that is not in the table".to_string());
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        Err("a code that is not in the table".to_string())
    }
}

/// Inflates a raw deflate stream. The output is never larger than `max_out`; a stream that would be is an error.
pub fn inflate(data: &[u8], max_out: usize) -> Result<Vec<u8>, String> {
    let mut b = Bits {
        data,
        pos: 0,
        buf: 0,
        cnt: 0,
    };
    let mut out: Vec<u8> = Vec::new();
    loop {
        let last = b.bits(1)?;
        match b.bits(2)? {
            0 => stored(&mut b, &mut out, max_out)?,
            1 => {
                let mut l = [0u8; 288];
                for (i, v) in l.iter_mut().enumerate() {
                    *v = match i {
                        0..=143 => 8,
                        144..=255 => 9,
                        256..=279 => 7,
                        _ => 8,
                    };
                }
                let lit = Huffman::new(&l)?;
                let dist = Huffman::new(&[5u8; 30])?;
                codes(&mut b, &mut out, &lit, &dist, max_out)?;
            }
            2 => {
                let nlen = b.bits(5)? as usize + 257;
                let ndist = b.bits(5)? as usize + 1;
                let ncode = b.bits(4)? as usize + 4;
                if nlen > 286 || ndist > 30 {
                    return Err("too many codes".to_string());
                }
                let mut cl = [0u8; 19];
                for &slot in CL_ORDER.iter().take(ncode) {
                    cl[slot] = b.bits(3)? as u8;
                }
                let clh = Huffman::new(&cl)?;
                let mut lengths = vec![0u8; nlen + ndist];
                let mut i = 0;
                while i < nlen + ndist {
                    let sym = clh.decode(&mut b)?;
                    if sym < 16 {
                        lengths[i] = sym as u8;
                        i += 1;
                    } else {
                        let (value, rep) = match sym {
                            16 => {
                                if i == 0 {
                                    return Err("a repeat with nothing before it".to_string());
                                }
                                (lengths[i - 1], 3 + b.bits(2)? as usize)
                            }
                            17 => (0, 3 + b.bits(3)? as usize),
                            _ => (0, 11 + b.bits(7)? as usize),
                        };
                        if i + rep > nlen + ndist {
                            return Err("a repeat goes past the lengths".to_string());
                        }
                        for slot in &mut lengths[i..i + rep] {
                            *slot = value;
                        }
                        i += rep;
                    }
                }
                if lengths[256] == 0 {
                    return Err("no end-of-block code".to_string());
                }
                let lit = Huffman::new(&lengths[..nlen])?;
                let dist = Huffman::new(&lengths[nlen..])?;
                codes(&mut b, &mut out, &lit, &dist, max_out)?;
            }
            _ => return Err("a block of an unknown type".to_string()),
        }
        if last == 1 {
            return Ok(out);
        }
    }
}

fn stored(b: &mut Bits, out: &mut Vec<u8>, max_out: usize) -> Result<(), String> {
    b.buf = 0;
    b.cnt = 0;
    let Some(head) = b.data.get(b.pos..b.pos + 4) else {
        return Err("a stored block is cut short".to_string());
    };
    let len = usize::from(head[0]) | (usize::from(head[1]) << 8);
    let nlen = usize::from(head[2]) | (usize::from(head[3]) << 8);
    if len != (!nlen & 0xFFFF) {
        return Err("a stored block with a wrong length check".to_string());
    }
    b.pos += 4;
    let Some(body) = b.data.get(b.pos..b.pos + len) else {
        return Err("a stored block is cut short".to_string());
    };
    if out.len() + len > max_out {
        return Err("the data inflates to more than the limit".to_string());
    }
    out.extend_from_slice(body);
    b.pos += len;
    Ok(())
}

fn codes(
    b: &mut Bits,
    out: &mut Vec<u8>,
    lit: &Huffman,
    dist: &Huffman,
    max_out: usize,
) -> Result<(), String> {
    loop {
        let sym = lit.decode(b)? as usize;
        if sym < 256 {
            if out.len() >= max_out {
                return Err("the data inflates to more than the limit".to_string());
            }
            out.push(sym as u8);
        } else if sym == 256 {
            return Ok(());
        } else {
            let s = sym - 257;
            if s >= 29 {
                return Err("an invalid length code".to_string());
            }
            let len = LEN_BASE[s] as usize + b.bits(u32::from(LEN_EXTRA[s]))? as usize;
            let ds = dist.decode(b)? as usize;
            if ds >= 30 {
                return Err("an invalid distance code".to_string());
            }
            let d = DIST_BASE[ds] as usize + b.bits(u32::from(DIST_EXTRA[ds]))? as usize;
            if d > out.len() {
                return Err("a distance before the start of the data".to_string());
            }
            if out.len() + len > max_out {
                return Err("the data inflates to more than the limit".to_string());
            }
            let start = out.len() - d;
            for k in 0..len {
                let byte = out[start + k];
                out.push(byte);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_core::rng::Rng;

    #[test]
    fn a_stored_block() {
        let data = [1u8, 3, 0, 0xFC, 0xFF, b'a', b'b', b'c'];
        assert_eq!(inflate(&data, 100).unwrap(), b"abc");
        // a wrong check
        let bad = [1u8, 3, 0, 0, 0, b'a', b'b', b'c'];
        assert!(inflate(&bad, 100).is_err());
        // cut short
        assert!(inflate(&data[..6], 100).is_err());
    }

    #[test]
    fn a_fixed_block_with_a_match() {
        // "abcabcabc" from zlib raw deflate with fixed codes: literals a b c then a match of length 6 at distance 3
        let data = [75u8, 76, 74, 78, 4, 35, 0];
        assert_eq!(inflate(&data, 100).unwrap(), b"abcabcabc");
        assert!(inflate(&data, 5).is_err());
    }

    #[test]
    fn real_streams_from_zlib() {
        let fixed_in = include_bytes!("../tests/fixtures/sample.txt");
        let fixed = include_bytes!("../tests/fixtures/sample.deflate");
        assert_eq!(inflate(fixed, 1 << 20).unwrap(), fixed_in);
        let dyn_in = include_bytes!("../tests/fixtures/dynamic.txt");
        let dynamic = include_bytes!("../tests/fixtures/dynamic.deflate");
        assert_eq!(inflate(dynamic, 1 << 20).unwrap(), dyn_in);
    }

    #[test]
    fn the_output_limit_stops_a_bomb() {
        let dynamic = include_bytes!("../tests/fixtures/dynamic.deflate");
        assert!(inflate(dynamic, 100).is_err());
    }

    #[test]
    fn truncated_and_corrupted_streams_are_errors_never_panics() {
        let dynamic = include_bytes!("../tests/fixtures/dynamic.deflate");
        for cut in [0, 1, 2, 10, 100, 1000, dynamic.len() - 1] {
            assert!(inflate(&dynamic[..cut], 1 << 20).is_err(), "{cut}");
        }
        let mut r = Rng::new(77);
        for _ in 0..300 {
            let mut d = dynamic.to_vec();
            for _ in 0..1 + r.below(4) {
                let i = r.below(d.len() as u64) as usize;
                d[i] ^= 1 << r.below(8);
            }
            let _ = inflate(&d, 1 << 20);
        }
        for _ in 0..300 {
            let n = r.below(64) as usize;
            let _ = inflate(&r.bytes(n), 4096);
        }
    }
}
