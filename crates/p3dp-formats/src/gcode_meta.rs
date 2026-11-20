//! What a plain G-code file says about itself in its comment lines: the thumbnails that slicers put at the top as base64 text (`; thumbnail begin 220x124 5120` ... `; thumbnail end`)
//! and the figures they write as `; key = value` (the filament used, the time, the layer height). It is read only: no line of the program is run or changed, and only the start and the end of the
//! file are looked at, because that is where the comments are, so a very large file costs the same as a small one.
//!
//! Everything is bounded by [`ScanLimits`]: how much of the start and of the end is read, how many thumbnails are taken and how large each may be once decoded. A thumbnail whose declared size is
//! not the size of its text, that is not closed, that is not valid base64 or whose bytes are not the picture it says is not handed out; it is listed in `problems` with the line.

use crate::picture::PictureFormat;
use crate::summary::PrintSummary;

/// How far a scan goes. The defaults are conservative; the settings file can change them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanLimits {
    /// The bytes read from the start of the file (where the thumbnails are).
    pub head_bytes: usize,
    /// The bytes read from the end of the file (where the figures of the print usually are).
    pub tail_bytes: usize,
    /// The most thumbnails taken from one file.
    pub max_thumbnails: usize,
    /// The most bytes of one thumbnail once decoded.
    pub max_thumbnail_bytes: usize,
    /// The most `key = value` lines kept.
    pub max_notes: usize,
}

impl Default for ScanLimits {
    fn default() -> ScanLimits {
        ScanLimits {
            head_bytes: 1 << 20,
            tail_bytes: 64 << 10,
            max_thumbnails: 8,
            max_thumbnail_bytes: 1 << 20,
            max_notes: 500,
        }
    }
}

/// The longest text kept of a key or a value.
const MAX_TEXT: usize = 300;

/// A thumbnail found in the comments of a G-code file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GcodeThumbnail {
    pub format: PictureFormat,
    pub width: u32,
    pub height: u32,
    /// The bytes of the picture, decoded.
    pub data: Vec<u8>,
}

/// What a scan of the thumbnails found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThumbnailScan {
    pub thumbnails: Vec<GcodeThumbnail>,
    /// What was wrong with a block that was not handed out, each with its line (from 1).
    pub problems: Vec<String>,
}

/// The decoded bytes of standard base64 (with or without the `=` padding), or `None` when a character does not belong or the length is impossible.
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let body = text.trim_end_matches('=');
    if text.len() - body.len() > 2 {
        return None;
    }
    let mut out = Vec::with_capacity(body.len() / 4 * 3 + 3);
    let mut acc = 0u32;
    let mut bits = 0u32;
    for b in body.bytes() {
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    // one character alone cannot carry a byte, and the bits left over must be zero
    if body.len() % 4 == 1 || acc != 0 {
        return None;
    }
    Some(out)
}

/// The text cut at a character boundary not past `max` bytes from the start.
fn head_of(text: &str, max: usize) -> &str {
    let mut n = max.min(text.len());
    while !text.is_char_boundary(n) {
        n -= 1;
    }
    &text[..n]
}

/// The text from a character boundary not before `max` bytes from the end.
fn tail_of(text: &str, max: usize) -> &str {
    let mut n = text.len().saturating_sub(max);
    while !text.is_char_boundary(n) {
        n += 1;
    }
    &text[n..]
}

/// The words of a comment line (`; thumbnail begin 16x16 40`), or `None` when the line is not a comment.
fn comment(line: &str) -> Option<&str> {
    line.trim().strip_prefix(';').map(str::trim)
}

fn format_of_tag(tag: &str) -> Option<PictureFormat> {
    match tag {
        "thumbnail" | "thumbnail_PNG" => Some(PictureFormat::Png),
        "thumbnail_JPG" => Some(PictureFormat::Jpg),
        "thumbnail_QOI" => Some(PictureFormat::Qoi),
        _ => None,
    }
}

/// `220x124` as `(220, 124)`.
fn size_of(word: &str) -> Option<(u32, u32)> {
    let (w, h) = word.split_once(['x', 'X'])?;
    let (w, h) = (w.parse::<u32>().ok()?, h.parse::<u32>().ok()?);
    (w > 0 && h > 0 && w <= 65_535 && h <= 65_535).then_some((w, h))
}

/// Finds the thumbnails in the first bytes of a G-code file.
pub fn thumbnails(text: &str, limits: &ScanLimits) -> ThumbnailScan {
    let mut scan = ThumbnailScan::default();
    let mut lines = head_of(text, limits.head_bytes).lines().enumerate();
    while let Some((n, line)) = lines.next() {
        let at = n + 1;
        let Some(words) = comment(line) else { continue };
        let w: Vec<&str> = words.split_whitespace().collect();
        let [tag, "begin", size, declared] = w.as_slice() else {
            continue;
        };
        let Some(format) = format_of_tag(tag) else {
            continue;
        };
        if scan.thumbnails.len() >= limits.max_thumbnails {
            scan.problems.push(format!(
                "line {at}: more than {} thumbnails; the rest are not read",
                limits.max_thumbnails
            ));
            break;
        }
        let (Some((width, height)), Ok(declared)) = (size_of(size), declared.parse::<usize>())
        else {
            scan.problems.push(format!(
                "line {at}: the size or the length of the thumbnail cannot be read"
            ));
            continue;
        };
        // a text of base64 is a third larger than the picture it carries
        if declared / 4 * 3 > limits.max_thumbnail_bytes {
            scan.problems.push(format!(
                "line {at}: a thumbnail of about {} bytes, more than the {} allowed",
                declared / 4 * 3,
                limits.max_thumbnail_bytes
            ));
            continue;
        }
        let mut payload = String::new();
        let mut closed = false;
        for (_, inner) in lines.by_ref() {
            let Some(c) = comment(inner) else { break };
            if c.split_whitespace().eq([*tag, "end"]) || c == "thumbnail end" {
                closed = true;
                break;
            }
            if payload.len() > declared.saturating_add(8) {
                break;
            }
            payload.extend(c.split_whitespace());
        }
        if !closed {
            scan.problems
                .push(format!("line {at}: the thumbnail is not closed"));
            continue;
        }
        if payload.len() != declared {
            scan.problems.push(format!(
                "line {at}: the thumbnail says {declared} characters and has {}",
                payload.len()
            ));
            continue;
        }
        let Some(data) = base64_decode(&payload) else {
            scan.problems
                .push(format!("line {at}: the thumbnail is not valid base64"));
            continue;
        };
        if !format.matches(&data) {
            scan.problems.push(format!(
                "line {at}: the bytes of the thumbnail are not a {} picture",
                format.extension()
            ));
            continue;
        }
        scan.thumbnails.push(GcodeThumbnail {
            format,
            width,
            height,
            data,
        });
    }
    scan
}

fn clip(s: &str) -> String {
    s.chars().take(MAX_TEXT).collect()
}

/// The `; key = value` lines of the start and of the end of a G-code file, in the order of the file (the start first), at most `max_notes` from each end. Thumbnail text is not a note.
pub fn notes(text: &str, limits: &ScanLimits) -> Vec<(String, String)> {
    let head = head_of(text, limits.head_bytes.min(64 << 10));
    let tail = if text.len() > head.len() {
        tail_of(text, limits.tail_bytes)
    } else {
        ""
    };
    let mut out: Vec<(String, String)> = Vec::new();
    for segment in [head, tail] {
        let first = out.len();
        for line in segment.lines() {
            if out.len() - first >= limits.max_notes {
                break;
            }
            let Some(words) = comment(line) else { continue };
            if let Some((k, v)) = words.split_once('=') {
                let k = k.trim();
                if !k.is_empty() && k.len() <= MAX_TEXT && !k.contains("thumbnail") {
                    out.push((clip(k), clip(v.trim())));
                }
            }
        }
    }
    out
}

/// The figures the comments state about the print.
pub fn summary(text: &str, limits: &ScanLimits) -> PrintSummary {
    let n = notes(text, limits);
    PrintSummary::from_pairs(n.iter().map(|(k, v)| (k.as_str(), v.as_str())))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Base64 of the bytes (the tests write what they read back).
    fn encode(data: &[u8]) -> String {
        const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s = String::new();
        for c in data.chunks(3) {
            let n = (u32::from(c[0]) << 16)
                | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
                | u32::from(*c.get(2).unwrap_or(&0));
            s.push(A[(n >> 18) as usize & 63] as char);
            s.push(A[(n >> 12) as usize & 63] as char);
            s.push(if c.len() > 1 {
                A[(n >> 6) as usize & 63] as char
            } else {
                '='
            });
            s.push(if c.len() > 2 {
                A[n as usize & 63] as char
            } else {
                '='
            });
        }
        s
    }

    const PNG: [u8; 12] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3, 4];

    fn block(tag: &str, w: u32, h: u32, data: &[u8], width: usize) -> String {
        let text = encode(data);
        let mut s = format!("; {tag} begin {w}x{h} {}\n", text.len());
        let mut i = 0;
        while i < text.len() {
            let j = (i + width).min(text.len());
            s.push_str(&format!("; {}\n", &text[i..j]));
            i = j;
        }
        s.push_str(&format!("; {tag} end\n"));
        s
    }

    #[test]
    fn base64_round_trips_every_length_and_refuses_what_is_not_base64() {
        for n in 0..40usize {
            let data: Vec<u8> = (0..n).map(|i| (i * 37 + 11) as u8).collect();
            assert_eq!(base64_decode(&encode(&data)).unwrap(), data, "{n}");
        }
        assert_eq!(base64_decode("TWFu").unwrap(), b"Man");
        assert_eq!(
            base64_decode("TWE").unwrap(),
            b"Ma",
            "the padding may be missing"
        );
        assert!(base64_decode("TW$u").is_none());
        assert!(
            base64_decode("A").is_none(),
            "one character cannot carry a byte"
        );
        assert!(
            base64_decode("TWF===").is_none(),
            "more padding than there can be"
        );
        assert!(
            base64_decode("TWG=").is_none(),
            "the bits left over must be zero"
        );
    }

    #[test]
    fn thumbnails_in_three_formats_are_found_with_their_size_and_their_bytes() {
        let jpg = [0xFF, 0xD8, 0xFF, 0xE0, 5, 6, 7];
        let text = format!(
            "; generated by a test\n{}{}{}G28\n",
            block("thumbnail", 16, 16, &PNG, 8),
            block("thumbnail_JPG", 32, 24, &jpg, 76),
            block("thumbnail_QOI", 8, 8, b"qoif1234567", 4)
        );
        let s = thumbnails(&text, &ScanLimits::default());
        assert!(s.problems.is_empty(), "{:?}", s.problems);
        assert_eq!(s.thumbnails.len(), 3);
        assert_eq!(
            (
                s.thumbnails[0].format,
                s.thumbnails[0].width,
                s.thumbnails[0].height
            ),
            (PictureFormat::Png, 16, 16)
        );
        assert_eq!(s.thumbnails[0].data, PNG);
        assert_eq!(s.thumbnails[1].format, PictureFormat::Jpg);
        assert_eq!(s.thumbnails[1].data, jpg);
        assert_eq!(s.thumbnails[2].data, b"qoif1234567");
        // a file with none
        assert!(thumbnails("G28\nG1 X1\n", &ScanLimits::default())
            .thumbnails
            .is_empty());
    }

    #[test]
    fn a_thumbnail_that_is_wrong_is_named_with_its_line_and_not_handed_out() {
        let l = ScanLimits::default();
        let good = block("thumbnail", 16, 16, &PNG, 8);
        // the declared length is not the real one
        let wrong_len = good.replacen(" 16x16 16", " 16x16 99", 1);
        assert_ne!(wrong_len, good);
        let s = thumbnails(&wrong_len, &l);
        assert!(s.thumbnails.is_empty());
        assert!(
            s.problems[0].contains("line 1") && s.problems[0].contains("says 99"),
            "{:?}",
            s.problems
        );
        // not closed
        let open = good.replace("; thumbnail end\n", "G28\n");
        assert!(thumbnails(&open, &l).problems[0].contains("not closed"));
        // not base64
        let bad64 = good.replace("; iVBO", "; i!BO");
        let s = thumbnails(&bad64, &l);
        assert!(
            s.thumbnails.is_empty() && !s.problems.is_empty(),
            "{:?}",
            s.problems
        );
        // base64 of something that is not a PNG
        let not_png = block("thumbnail", 16, 16, b"GIF89a-----", 8);
        assert!(thumbnails(&not_png, &l).problems[0].contains("not a png"));
        // a size that cannot be read
        let e = thumbnails("; thumbnail begin 0x16 4\n; AAAA\n; thumbnail end\n", &l);
        assert!(e.problems[0].contains("cannot be read"), "{:?}", e.problems);
        // a good one after a bad one is still found
        let both = format!("{not_png}{good}");
        let s = thumbnails(&both, &l);
        assert_eq!((s.thumbnails.len(), s.problems.len()), (1, 1));
    }

    #[test]
    fn the_limits_cap_the_number_the_size_and_how_far_the_scan_goes() {
        let one = block("thumbnail", 16, 16, &PNG, 8);
        let five = one.repeat(5);
        let two = ScanLimits {
            max_thumbnails: 2,
            ..ScanLimits::default()
        };
        let s = thumbnails(&five, &two);
        assert_eq!(s.thumbnails.len(), 2);
        assert!(s.problems[0].contains("more than 2"));
        // a thumbnail declared larger than the limit is not even decoded
        let small = ScanLimits {
            max_thumbnail_bytes: 4,
            ..ScanLimits::default()
        };
        let s = thumbnails(&one, &small);
        assert!(
            s.thumbnails.is_empty() && s.problems[0].contains("more than the 4"),
            "{:?}",
            s.problems
        );
        // the scan does not go past the first bytes
        let far = format!("{}{one}", "; filler\n".repeat(200));
        let near = ScanLimits {
            head_bytes: 100,
            ..ScanLimits::default()
        };
        assert!(thumbnails(&far, &near).thumbnails.is_empty());
        assert_eq!(thumbnails(&far, &ScanLimits::default()).thumbnails.len(), 1);
    }

    #[test]
    fn the_figures_at_the_start_and_at_the_end_are_read_and_the_middle_is_not() {
        let half = "G1 X1 Y1 E0.1\n".repeat(50_000);
        let text = format!(
            "; generated by a test\n; layer_height = 0.2\n{half}; not a note = from the middle\n{half}; filament used [mm] = 1500.25\n; filament used [g] = 4.5\n; estimated printing time (normal mode) = 1h 30m 0s\n"
        );
        let l = ScanLimits::default();
        let n = notes(&text, &l);
        assert!(n.iter().any(|(k, _)| k == "layer_height"));
        assert!(n
            .iter()
            .any(|(k, v)| k == "filament used [mm]" && v == "1500.25"));
        assert!(
            !n.iter().any(|(k, _)| k.contains("not a note")),
            "the middle is not read"
        );
        let s = summary(&text, &l);
        assert_eq!(s.layer_height_mm, Some(0.2));
        assert_eq!(s.filament_mm, Some(1500.25));
        assert_eq!(s.filament_g, Some(4.5));
        assert_eq!(s.seconds, Some(5400));
        // a short file is read once, not twice
        let short = "; a = 1\nG28\n";
        assert_eq!(notes(short, &l).len(), 1);
        // the thumbnail text is not a note, and the count is capped
        let with_thumb = block("thumbnail", 16, 16, &PNG, 8);
        assert!(notes(&with_thumb, &l).is_empty());
        let capped = ScanLimits {
            max_notes: 3,
            ..ScanLimits::default()
        };
        assert_eq!(notes(&"; k = v\n".repeat(10), &capped).len(), 3);
    }

    #[test]
    fn text_with_multibyte_characters_is_cut_on_a_boundary_and_never_panics() {
        let text = format!(
            "; name = {}\n{}",
            "ñ日本".repeat(100),
            "; z = 1\n".repeat(50)
        );
        for head in [0, 1, 2, 3, 7, 11, 100] {
            for tail in [0, 1, 5, 13] {
                let l = ScanLimits {
                    head_bytes: head,
                    tail_bytes: tail,
                    ..ScanLimits::default()
                };
                let _ = notes(&text, &l);
                let _ = thumbnails(&text, &l);
            }
        }
        assert_eq!(head_of("añb", 2), "a");
        assert_eq!(tail_of("añb", 2), "b");
    }
}
