//! The header of binary G-code files: the file starts with the four letters `GCDE`, a version, the kind of checksum, and then a run of blocks, each with a small header (its kind, how
//! it is compressed and its sizes), its parameters, its data and, when the file has checksums, a CRC-32. This reader is read-only and goes no further than the headers: it lists the
//! blocks, and it reads the metadata blocks that are stored as plain text (`key=value` lines) so the person can see which printer, which filament and what time the file says. It never
//! inflates or decodes the blocks of G-code, and it does not write the format.
//!
//! The layout was taken from the public description of the format; it was checked against files made by the tests of this repository following that description, and not against files
//! written by another program, so a file that does not follow it is refused with the reason and a file that does is read only as far as its headers.
//!
//! Limits: at most [`MAX_BLOCKS`] blocks, at most [`MAX_METADATA_ENTRIES`] entries and [`MAX_TEXT`] bytes in each text; a block that runs past the end of the file is a damaged file.

use crate::crc32::crc32;
use crate::error::FormatError;
use p3dp_core::Limits;

/// The first four bytes of the file.
pub const MAGIC: &[u8; 4] = b"GCDE";
/// The most blocks read from one file.
pub const MAX_BLOCKS: usize = 2_000_000;
/// The most entries kept from the metadata of a file.
pub const MAX_METADATA_ENTRIES: usize = 500;
/// The longest text kept of a key or a value.
pub const MAX_TEXT: usize = 300;

fn bad(reason: impl Into<String>) -> FormatError {
    FormatError::malformed("bgcode", reason)
}

/// What a block holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    FileMetadata,
    GCode,
    SlicerMetadata,
    PrinterMetadata,
    PrintMetadata,
    Thumbnail,
    Unknown(u16),
}

impl BlockKind {
    fn from_code(c: u16) -> BlockKind {
        match c {
            0 => BlockKind::FileMetadata,
            1 => BlockKind::GCode,
            2 => BlockKind::SlicerMetadata,
            3 => BlockKind::PrinterMetadata,
            4 => BlockKind::PrintMetadata,
            5 => BlockKind::Thumbnail,
            other => BlockKind::Unknown(other),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            BlockKind::FileMetadata => "file metadata",
            BlockKind::GCode => "g-code",
            BlockKind::SlicerMetadata => "slicer metadata",
            BlockKind::PrinterMetadata => "printer metadata",
            BlockKind::PrintMetadata => "print metadata",
            BlockKind::Thumbnail => "thumbnail",
            BlockKind::Unknown(_) => "unknown",
        }
    }

    fn is_metadata(self) -> bool {
        matches!(
            self,
            BlockKind::FileMetadata
                | BlockKind::SlicerMetadata
                | BlockKind::PrinterMetadata
                | BlockKind::PrintMetadata
        )
    }
}

/// How the data of a block is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    None,
    Deflate,
    Heatshrink11,
    Heatshrink12,
    Unknown(u16),
}

impl Compression {
    fn from_code(c: u16) -> Compression {
        match c {
            0 => Compression::None,
            1 => Compression::Deflate,
            2 => Compression::Heatshrink11,
            3 => Compression::Heatshrink12,
            other => Compression::Unknown(other),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Compression::None => "none",
            Compression::Deflate => "deflate",
            Compression::Heatshrink11 => "heatshrink 11,4",
            Compression::Heatshrink12 => "heatshrink 12,4",
            Compression::Unknown(_) => "unknown",
        }
    }
}

/// The header of one block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockInfo {
    pub kind: BlockKind,
    pub compression: Compression,
    /// The size of the data once it is unpacked.
    pub unpacked_bytes: u32,
    /// The size as it is stored (the same as the unpacked size when the block is not compressed).
    pub stored_bytes: u32,
    /// Where the block starts in the file.
    pub offset: usize,
    /// Whether the CRC-32 of the block matches (`None` when the file has no checksums).
    pub checksum_ok: Option<bool>,
}

/// The kind of checksum of the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Checksum {
    None,
    Crc32,
}

/// What the header of a file says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub version: u32,
    pub checksum: Checksum,
    pub blocks: Vec<BlockInfo>,
    /// The `key = value` pairs of the metadata blocks that are stored as plain text, in the order of the file, each with the kind of block it came from.
    pub metadata: Vec<(BlockKind, String, String)>,
}

impl Header {
    /// A value of the metadata by its key (the first block that has it wins).
    pub fn value(&self, key: &str) -> Option<&str> {
        self.metadata
            .iter()
            .find(|(_, k, _)| k == key)
            .map(|(_, _, v)| v.as_str())
    }

    /// How many blocks of a kind.
    pub fn count(&self, kind: BlockKind) -> usize {
        self.blocks.iter().filter(|b| b.kind == kind).count()
    }

    /// Whether every checksum that the file carries is right.
    pub fn all_checksums_ok(&self) -> bool {
        self.blocks.iter().all(|b| b.checksum_ok != Some(false))
    }

    /// The time the file says the print takes, in seconds, from a text like `1d 2h 3m 4s` in the key `estimated printing time (normal mode)`.
    pub fn estimated_seconds(&self) -> Option<u64> {
        parse_duration(self.value("estimated printing time (normal mode)")?)
    }
}

/// `1d 2h 3m 4s` (any of the parts, in that order) as seconds.
pub fn parse_duration(text: &str) -> Option<u64> {
    let mut total = 0u64;
    let mut seen = false;
    for part in text.split_whitespace() {
        let (num, unit) = part.split_at(part.len().checked_sub(1)?);
        let n: u64 = num.parse().ok()?;
        let mult = match unit {
            "d" => 86_400,
            "h" => 3_600,
            "m" => 60,
            "s" => 1,
            _ => return None,
        };
        total = total.checked_add(n.checked_mul(mult)?)?;
        seen = true;
    }
    seen.then_some(total)
}

fn u16_at(d: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(i..i + 2)?.try_into().ok()?))
}

fn u32_at(d: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(i..i + 4)?.try_into().ok()?))
}

fn clip(s: &str) -> String {
    s.chars().take(MAX_TEXT).collect()
}

/// Whether the bytes start like a binary G-code file.
pub fn is_bgcode(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}

/// Reads the header of a binary G-code file and the headers of its blocks.
pub fn read_header(bytes: &[u8], limits: &Limits) -> Result<Header, FormatError> {
    if bytes.len() as u64 > limits.max_file_bytes {
        return Err(FormatError::TooLarge {
            bytes: bytes.len() as u64,
            max: limits.max_file_bytes,
        });
    }
    if !is_bgcode(bytes) {
        return Err(bad("the file does not start with GCDE"));
    }
    let version = u32_at(bytes, 4).ok_or_else(|| bad("the header is cut short"))?;
    if version != 1 {
        return Err(FormatError::unsupported(
            "bgcode",
            format!("version {version} (this reader knows version 1)"),
        ));
    }
    let checksum = match u16_at(bytes, 8).ok_or_else(|| bad("the header is cut short"))? {
        0 => Checksum::None,
        1 => Checksum::Crc32,
        other => {
            return Err(FormatError::unsupported(
                "bgcode",
                format!("checksum kind {other}"),
            ))
        }
    };
    let mut at = 10usize;
    let mut blocks: Vec<BlockInfo> = Vec::new();
    let mut metadata: Vec<(BlockKind, String, String)> = Vec::new();
    while at < bytes.len() {
        if blocks.len() >= MAX_BLOCKS {
            return Err(bad(format!("more than {MAX_BLOCKS} blocks")));
        }
        let start = at;
        let kind_code = u16_at(bytes, at).ok_or_else(|| bad("a block header is cut short"))?;
        let comp_code = u16_at(bytes, at + 2).ok_or_else(|| bad("a block header is cut short"))?;
        let unpacked = u32_at(bytes, at + 4).ok_or_else(|| bad("a block header is cut short"))?;
        let kind = BlockKind::from_code(kind_code);
        let compression = Compression::from_code(comp_code);
        if let BlockKind::Unknown(c) = kind {
            return Err(FormatError::unsupported(
                "bgcode",
                format!("block kind {c}"),
            ));
        }
        if let Compression::Unknown(c) = compression {
            return Err(FormatError::unsupported(
                "bgcode",
                format!("compression {c}"),
            ));
        }
        let (stored, header_len) = if compression == Compression::None {
            (unpacked, 8usize)
        } else {
            let packed = u32_at(bytes, at + 8).ok_or_else(|| bad("a block header is cut short"))?;
            (packed, 12usize)
        };
        let params = if kind == BlockKind::Thumbnail { 6 } else { 2 };
        let data_at = start + header_len + params;
        let end = data_at
            .checked_add(stored as usize)
            .ok_or_else(|| bad("a size that is too large"))?;
        let crc_end = if checksum == Checksum::Crc32 {
            end.checked_add(4)
                .ok_or_else(|| bad("a size that is too large"))?
        } else {
            end
        };
        if crc_end > bytes.len() {
            return Err(bad(format!(
                "the block at byte {start} runs past the end of the file"
            )));
        }
        let checksum_ok = (checksum == Checksum::Crc32)
            .then(|| u32_at(bytes, end) == Some(crc32(&bytes[start..end])));
        if kind.is_metadata() && compression == Compression::None {
            // the parameter of a metadata block is its encoding: 0 is plain `key=value` lines
            if u16_at(bytes, start + header_len) == Some(0) {
                let text = String::from_utf8_lossy(&bytes[data_at..end]);
                for line in text.lines() {
                    if metadata.len() >= MAX_METADATA_ENTRIES {
                        break;
                    }
                    if let Some((k, v)) = line.split_once('=') {
                        metadata.push((kind, clip(k.trim()), clip(v.trim())));
                    }
                }
            }
        }
        blocks.push(BlockInfo {
            kind,
            compression,
            unpacked_bytes: unpacked,
            stored_bytes: stored,
            offset: start,
            checksum_ok,
        });
        at = crc_end;
    }
    if blocks.is_empty() {
        return Err(bad("the file has a header and no block"));
    }
    Ok(Header {
        version,
        checksum,
        blocks,
        metadata,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Writes a block as the format describes it (the tests build their files with this and read them with the reader).
    pub(crate) fn block(
        kind: u16,
        compression: u16,
        params: &[u8],
        data: &[u8],
        unpacked: u32,
        crc: bool,
    ) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&kind.to_le_bytes());
        b.extend_from_slice(&compression.to_le_bytes());
        b.extend_from_slice(&unpacked.to_le_bytes());
        if compression != 0 {
            b.extend_from_slice(&(data.len() as u32).to_le_bytes());
        }
        b.extend_from_slice(params);
        b.extend_from_slice(data);
        if crc {
            let c = crc32(&b);
            b.extend_from_slice(&c.to_le_bytes());
        }
        b
    }

    pub(crate) fn file(crc: bool, blocks: &[Vec<u8>]) -> Vec<u8> {
        let mut f = Vec::new();
        f.extend_from_slice(MAGIC);
        f.extend_from_slice(&1u32.to_le_bytes());
        f.extend_from_slice(&u16::from(crc).to_le_bytes());
        for b in blocks {
            f.extend_from_slice(b);
        }
        f
    }

    fn sample(crc: bool) -> Vec<u8> {
        let meta = b"Producer=SomeSlicer 1.0\n";
        let printer = b"printer_model=GENERIC\nfilament_type=PLA\nestimated printing time (normal mode)=1h 2m 3s\n";
        let gcode = [0u8; 40];
        let thumb = [0u8; 10];
        file(
            crc,
            &[
                block(0, 0, &0u16.to_le_bytes(), meta, meta.len() as u32, crc),
                block(
                    3,
                    0,
                    &0u16.to_le_bytes(),
                    printer,
                    printer.len() as u32,
                    crc,
                ),
                block(5, 0, &[0, 0, 16, 0, 16, 0], &thumb, thumb.len() as u32, crc),
                block(1, 1, &0u16.to_le_bytes(), &gcode, 400, crc),
                block(1, 3, &0u16.to_le_bytes(), &gcode[..12], 300, crc),
            ],
        )
    }

    #[test]
    fn a_file_with_every_kind_of_block_is_listed_and_its_text_is_read() {
        let h = read_header(&sample(true), &Limits::default()).unwrap();
        assert_eq!((h.version, h.checksum), (1, Checksum::Crc32));
        assert_eq!(h.blocks.len(), 5);
        assert_eq!(h.count(BlockKind::GCode), 2);
        assert_eq!(h.blocks[3].compression, Compression::Deflate);
        assert_eq!(
            (h.blocks[3].unpacked_bytes, h.blocks[3].stored_bytes),
            (400, 40)
        );
        assert_eq!(h.blocks[4].compression, Compression::Heatshrink12);
        assert_eq!(h.value("Producer"), Some("SomeSlicer 1.0"));
        assert_eq!(h.value("printer_model"), Some("GENERIC"));
        assert_eq!(h.estimated_seconds(), Some(3723));
        assert!(h.all_checksums_ok());
        assert!(h.blocks.iter().all(|b| b.checksum_ok == Some(true)));
        // the same file with no checksums
        let h = read_header(&sample(false), &Limits::default()).unwrap();
        assert_eq!(h.checksum, Checksum::None);
        assert!(h.blocks.iter().all(|b| b.checksum_ok.is_none()));
        assert_eq!(h.value("filament_type"), Some("PLA"));
    }

    #[test]
    fn a_wrong_checksum_is_reported_and_does_not_stop_the_reading() {
        let mut f = sample(true);
        let n = f.len();
        f[n - 1] ^= 0xFF;
        let h = read_header(&f, &Limits::default()).unwrap();
        assert!(!h.all_checksums_ok());
        assert_eq!(h.blocks.last().unwrap().checksum_ok, Some(false));
        assert_eq!(h.blocks[0].checksum_ok, Some(true));
    }

    #[test]
    fn what_is_not_this_format_or_is_cut_or_made_to_hurt_is_refused_with_the_reason() {
        let l = Limits::default();
        assert!(!is_bgcode(b"; plain gcode\nG28\n"));
        assert!(read_header(b"; plain gcode\nG28\n", &l).is_err());
        assert!(read_header(b"GCDE", &l).is_err());
        let mut v2 = sample(false);
        v2[4] = 2;
        assert!(matches!(
            read_header(&v2, &l),
            Err(FormatError::Unsupported { .. })
        ));
        let mut bad_sum = sample(false);
        bad_sum[8] = 9;
        assert!(matches!(
            read_header(&bad_sum, &l),
            Err(FormatError::Unsupported { .. })
        ));
        // a header and no block
        assert!(read_header(&file(false, &[]), &l).is_err());
        // every cut of a good file is an error or a shorter file, and none panics
        let f = sample(true);
        for n in 0..f.len() {
            let _ = read_header(&f[..n], &l);
        }
        // a block that claims more than the file holds
        let mut huge = block(1, 1, &[0, 0], &[1, 2, 3], 10, false);
        huge[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(read_header(&file(false, &[huge]), &l).is_err());
        // a block kind and a compression that are not known
        let unknown_kind = block(77, 0, &[0, 0], &[1], 1, false);
        assert!(matches!(
            read_header(&file(false, &[unknown_kind]), &l),
            Err(FormatError::Unsupported { .. })
        ));
        let unknown_comp = block(1, 9, &[0, 0], &[1], 1, false);
        assert!(matches!(
            read_header(&file(false, &[unknown_comp]), &l),
            Err(FormatError::Unsupported { .. })
        ));
        // a file larger than the limit
        let small = Limits {
            max_file_bytes: 20,
            ..Limits::default()
        };
        assert!(matches!(
            read_header(&sample(false), &small),
            Err(FormatError::TooLarge { .. })
        ));
    }

    #[test]
    fn the_text_is_cut_and_the_number_of_entries_is_limited() {
        let mut text = String::new();
        for i in 0..(MAX_METADATA_ENTRIES + 50) {
            text.push_str(&format!("k{i}={}\n", "v".repeat(1000)));
        }
        let f = file(
            false,
            &[block(
                0,
                0,
                &[0, 0],
                text.as_bytes(),
                text.len() as u32,
                false,
            )],
        );
        let h = read_header(&f, &Limits::default()).unwrap();
        assert_eq!(h.metadata.len(), MAX_METADATA_ENTRIES);
        assert!(h.metadata.iter().all(|(_, _, v)| v.len() == MAX_TEXT));
        // an encoding other than plain text is not read as text
        let enc = file(false, &[block(0, 0, &[1, 0], b"a=b\n", 4, false)]);
        assert!(read_header(&enc, &Limits::default())
            .unwrap()
            .metadata
            .is_empty());
    }

    #[test]
    fn durations_are_read_in_days_hours_minutes_and_seconds() {
        assert_eq!(parse_duration("1d 2h 3m 4s"), Some(93_784));
        assert_eq!(parse_duration("45s"), Some(45));
        assert_eq!(parse_duration("2h"), Some(7_200));
        for bad in ["", "soon", "1x", "h", "1 h", "-5s"] {
            assert_eq!(parse_duration(bad), None, "{bad}");
        }
    }
}
