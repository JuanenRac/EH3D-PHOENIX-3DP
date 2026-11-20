//! A writer of PNG files for the pictures of the window (`--shot`), with no outside library: 8 bits per channel, RGBA, no filter, and the data of the image in the "stored" blocks
//! of the zlib format (no compression, so the file is larger than it could be and the code is short enough to be checked by reading it). The CRC of each chunk and the Adler-32 of
//! the data are worked out here and the tests read the file back with the inflate reader of the program.

use crate::crc32::crc32;
use crate::error::FormatError;

/// The most pixels of a picture (a window of 8192 by 8192 is 67 million: 16384 by 16384 is the limit).
pub const MAX_SIDE: u32 = 16_384;

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5552) {
        for byte in chunk {
            a += u32::from(*byte);
            b += a;
        }
        a %= 65_521;
        b %= 65_521;
    }
    (b << 16) | a
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// The PNG file of an image of `width` by `height` pixels, given as RGBA bytes row after row. The size must agree with the bytes.
pub fn encode_rgba(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, FormatError> {
    let bad = |why: String| FormatError::malformed("png", why);
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
        return Err(bad(format!(
            "a picture is 1 to {MAX_SIDE} pixels on each side, not {width} by {height}"
        )));
    }
    let want = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4));
    if want != Some(rgba.len()) {
        return Err(bad(format!(
            "{} bytes are not the {width} by {height} pixels of an RGBA picture",
            rgba.len()
        )));
    }
    // every row starts with the filter byte 0 (none)
    let row = width as usize * 4;
    let mut raw = Vec::with_capacity((row + 1) * height as usize);
    for line in rgba.chunks_exact(row) {
        raw.push(0);
        raw.extend_from_slice(line);
    }
    // zlib: the header, blocks of at most 65535 bytes that are stored as they are, and the Adler-32
    let mut z = vec![0x78, 0x01];
    let blocks = raw.chunks(65_535).count();
    for (i, block) in raw.chunks(65_535).enumerate() {
        z.push(u8::from(i + 1 == blocks));
        let len = block.len() as u16;
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(block);
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut head = Vec::with_capacity(13);
    head.extend_from_slice(&width.to_be_bytes());
    head.extend_from_slice(&height.to_be_bytes());
    head.extend_from_slice(&[8, 6, 0, 0, 0]); // 8 bits, colour type 6 (RGBA), the usual methods, no interlace
    chunk(&mut out, b"IHDR", &head);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inflate::inflate;
    use p3dp_core::rng::Rng;

    /// Reads the chunks of a file and checks the signature and every CRC: the kind and the data of each.
    fn chunks(png: &[u8]) -> Vec<([u8; 4], Vec<u8>)> {
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        let mut at = 8;
        let mut out = Vec::new();
        while at < png.len() {
            let len = u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
            let kind: [u8; 4] = png[at + 4..at + 8].try_into().unwrap();
            let data = png[at + 8..at + 8 + len].to_vec();
            let crc = u32::from_be_bytes(png[at + 8 + len..at + 12 + len].try_into().unwrap());
            assert_eq!(
                crc,
                crc32(&png[at + 4..at + 8 + len]),
                "the CRC of {kind:?}"
            );
            out.push((kind, data));
            at += 12 + len;
        }
        out
    }

    #[test]
    fn adler32_gives_the_published_value() {
        // the value of the example of the zlib specification
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
        assert_eq!(adler32(b""), 1);
        // a long run does not overflow
        assert_eq!(adler32(&vec![255u8; 100_000]), {
            let (mut a, mut b) = (1u64, 0u64);
            for _ in 0..100_000 {
                a = (a + 255) % 65_521;
                b = (b + a) % 65_521;
            }
            ((b << 16) | a) as u32
        });
    }

    #[test]
    fn a_picture_is_written_with_the_three_chunks_and_read_back_whole() {
        let mut rng = Rng::new(0x0050_4E47);
        for (w, h) in [(1u32, 1u32), (3, 2), (17, 5), (300, 90), (1, 400)] {
            let pixels = rng.bytes((w * h * 4) as usize);
            let png = encode_rgba(w, h, &pixels).unwrap();
            let c = chunks(&png);
            let kinds: Vec<&[u8; 4]> = c.iter().map(|(k, _)| k).collect();
            assert_eq!(kinds, [b"IHDR", b"IDAT", b"IEND"]);
            assert_eq!(
                &c[0].1[..8],
                &[w.to_be_bytes(), h.to_be_bytes()].concat()[..]
            );
            assert_eq!(&c[0].1[8..], &[8, 6, 0, 0, 0]);
            // the data inside the zlib wrapper
            let z = &c[1].1;
            assert_eq!(&z[..2], &[0x78, 0x01]);
            let raw = inflate(&z[2..z.len() - 4], 1 << 28).unwrap();
            assert_eq!(z[z.len() - 4..], adler32(&raw).to_be_bytes());
            assert_eq!(raw.len(), (w as usize * 4 + 1) * h as usize);
            for (y, line) in raw.chunks(w as usize * 4 + 1).enumerate() {
                assert_eq!(line[0], 0);
                assert_eq!(
                    &line[1..],
                    &pixels[y * w as usize * 4..(y + 1) * w as usize * 4]
                );
            }
        }
    }

    #[test]
    fn a_picture_longer_than_one_stored_block_is_cut_in_blocks_and_the_last_one_is_marked() {
        // 200 x 100 x 4 = 80000 bytes and the filter bytes: two blocks
        let png = encode_rgba(200, 100, &vec![7u8; 200 * 100 * 4]).unwrap();
        let z = &chunks(&png)[1].1;
        assert_eq!(z[2], 0, "the first block is not the last");
        let second = 2 + 5 + 65_535;
        assert_eq!(z[second], 1, "the second block is the last");
        assert_eq!(inflate(&z[2..z.len() - 4], 1 << 28).unwrap().len(), 80_100);
    }

    #[test]
    fn a_size_that_does_not_agree_with_the_bytes_or_is_out_of_range_is_refused() {
        for (w, h, n) in [
            (0u32, 1u32, 0usize),
            (1, 0, 0),
            (2, 2, 15),
            (2, 2, 17),
            (16_385, 1, 65_540),
            (1, 16_385, 65_540),
        ] {
            assert!(
                encode_rgba(w, h, &vec![0; n]).is_err(),
                "{w}x{h} with {n} bytes"
            );
        }
        assert!(encode_rgba(u32::MAX, u32::MAX, &[]).is_err());
    }
}
