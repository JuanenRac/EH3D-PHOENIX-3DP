//! The formats of the small pictures that slicers put inside a G-code file (a thumbnail of the print): PNG, JPEG and QOI. The program does not decode them; it only says which one a
//! block of bytes is, from the first bytes, so a picture that is not what its header says is not handed on to a window that would draw it.

/// The format of a picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PictureFormat {
    Png,
    Jpg,
    Qoi,
}

impl PictureFormat {
    /// The code that binary G-code uses for the format of a thumbnail (0 PNG, 1 JPEG, 2 QOI).
    pub fn from_code(code: u16) -> Option<PictureFormat> {
        match code {
            0 => Some(PictureFormat::Png),
            1 => Some(PictureFormat::Jpg),
            2 => Some(PictureFormat::Qoi),
            _ => None,
        }
    }

    /// The name of the format, as a file extension.
    pub fn extension(self) -> &'static str {
        match self {
            PictureFormat::Png => "png",
            PictureFormat::Jpg => "jpg",
            PictureFormat::Qoi => "qoi",
        }
    }

    /// The format a name stands for (`png`, `jpg`, `jpeg` or `qoi`, any case).
    pub fn from_name(name: &str) -> Option<PictureFormat> {
        match name.trim().to_ascii_lowercase().as_str() {
            "png" => Some(PictureFormat::Png),
            "jpg" | "jpeg" => Some(PictureFormat::Jpg),
            "qoi" => Some(PictureFormat::Qoi),
            _ => None,
        }
    }

    /// Whether the bytes start the way a picture of this format starts.
    pub fn matches(self, data: &[u8]) -> bool {
        match self {
            PictureFormat::Png => {
                data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A])
            }
            PictureFormat::Jpg => data.starts_with(&[0xFF, 0xD8, 0xFF]),
            PictureFormat::Qoi => data.starts_with(b"qoif"),
        }
    }

    /// The format whose first bytes these are.
    pub fn sniff(data: &[u8]) -> Option<PictureFormat> {
        [PictureFormat::Png, PictureFormat::Jpg, PictureFormat::Qoi]
            .into_iter()
            .find(|f| f.matches(data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_format_is_known_by_its_code_its_name_and_its_first_bytes() {
        for (code, f, ext) in [
            (0, PictureFormat::Png, "png"),
            (1, PictureFormat::Jpg, "jpg"),
            (2, PictureFormat::Qoi, "qoi"),
        ] {
            assert_eq!(PictureFormat::from_code(code), Some(f));
            assert_eq!(f.extension(), ext);
            assert_eq!(PictureFormat::from_name(ext), Some(f));
        }
        assert_eq!(PictureFormat::from_code(3), None);
        assert_eq!(PictureFormat::from_name(" JPEG "), Some(PictureFormat::Jpg));
        assert_eq!(PictureFormat::from_name("gif"), None);
        let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0];
        assert_eq!(PictureFormat::sniff(&png), Some(PictureFormat::Png));
        assert_eq!(
            PictureFormat::sniff(&[0xFF, 0xD8, 0xFF, 0xE0]),
            Some(PictureFormat::Jpg)
        );
        assert_eq!(PictureFormat::sniff(b"qoif\0\0"), Some(PictureFormat::Qoi));
    }

    #[test]
    fn bytes_that_are_not_the_format_they_claim_do_not_match_and_short_input_is_safe() {
        assert!(!PictureFormat::Png.matches(b"GIF89a"));
        assert!(!PictureFormat::Png.matches(&[0x89, b'P']));
        assert!(!PictureFormat::Jpg.matches(&[]));
        assert_eq!(PictureFormat::sniff(b""), None);
        assert_eq!(PictureFormat::sniff(b"; thumbnail"), None);
    }
}
