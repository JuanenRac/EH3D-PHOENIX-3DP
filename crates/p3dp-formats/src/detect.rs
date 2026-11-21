//! Which format a file is, and one function to read or write a mesh in any of them.

use crate::error::FormatError;
use crate::{obj, stl, threemf};
use p3dp_core::{Limits, Mesh};

/// The formats of a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Stl,
    Obj,
    ThreeMf,
}

impl Format {
    /// The format of a file extension (any case, with or without the dot).
    pub fn from_extension(ext: &str) -> Option<Format> {
        match ext.trim_start_matches('.').to_ascii_lowercase().as_str() {
            "stl" => Some(Format::Stl),
            "obj" => Some(Format::Obj),
            "3mf" => Some(Format::ThreeMf),
            _ => None,
        }
    }

    /// The format of a path, by its extension.
    pub fn from_path(path: &str) -> Option<Format> {
        let (_, ext) = path.rsplit_once('.')?;
        Format::from_extension(ext)
    }

    /// By what the file starts with: `PK` is a package, `solid` or a binary STL header is an STL, `v ` or `#` is an OBJ.
    pub fn sniff(bytes: &[u8]) -> Option<Format> {
        if bytes.starts_with(b"PK\x03\x04") {
            return Some(Format::ThreeMf);
        }
        let head = String::from_utf8_lossy(&bytes[..bytes.len().min(256)]).to_ascii_lowercase();
        let head = head.trim_start();
        if head.starts_with("solid") {
            return Some(Format::Stl);
        }
        // a binary STL is told by its size before the text of its 80-byte header is looked at: the header may start with anything
        if bytes.len() >= 84 {
            let n = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
            if 84usize.checked_add(n.saturating_mul(50)) == Some(bytes.len()) {
                return Some(Format::Stl);
            }
        }
        if head.starts_with("v ")
            || head.starts_with('#')
            || head.starts_with("o ")
            || head.starts_with("mtllib")
        {
            return Some(Format::Obj);
        }
        None
    }

    pub fn extension(self) -> &'static str {
        match self {
            Format::Stl => "stl",
            Format::Obj => "obj",
            Format::ThreeMf => "3mf",
        }
    }
}

/// Reads a mesh. The format comes from the hint (a path or an extension) when it is given and agrees with what the file starts with; a file whose content says another
/// format than its name is read as its content says. A file larger than the limit is refused before anything is read.
pub fn read_mesh(
    bytes: &[u8],
    name_hint: Option<&str>,
    limits: &Limits,
) -> Result<Mesh, FormatError> {
    if bytes.len() as u64 > limits.max_file_bytes {
        return Err(FormatError::TooLarge {
            bytes: bytes.len() as u64,
            max: limits.max_file_bytes,
        });
    }
    let by_name =
        name_hint.and_then(|n| Format::from_path(n).or_else(|| Format::from_extension(n)));
    let by_content = Format::sniff(bytes);
    let format = by_content.or(by_name).ok_or_else(|| {
        FormatError::malformed("model", "the file is not an STL, an OBJ or a 3MF")
    })?;
    match format {
        Format::Stl => stl::read(bytes, limits),
        Format::Obj => {
            // a comment or a name in another encoding (Latin-1) must not refuse the model
            let text = String::from_utf8_lossy(bytes);
            obj::read(&text, limits)
        }
        Format::ThreeMf => threemf::read(bytes, limits),
    }
}

/// Writes a mesh in a format (the 3MF carries the title; the others carry only the mesh).
pub fn write_mesh(mesh: &Mesh, format: Format, title: &str) -> Result<Vec<u8>, FormatError> {
    match format {
        Format::Stl => stl::write_binary(mesh),
        Format::Obj => obj::write(mesh).map(String::into_bytes),
        Format::ThreeMf => threemf::write(
            mesh,
            &threemf::Meta {
                title: title.to_string(),
                printer: String::new(),
            },
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_core::mesh::cube;

    #[test]
    fn extensions_and_paths() {
        assert_eq!(Format::from_extension(".STL"), Some(Format::Stl));
        assert_eq!(Format::from_path("C:/x/a.3MF"), Some(Format::ThreeMf));
        assert_eq!(Format::from_path("noext"), None);
        assert_eq!(Format::from_path("a.gcode"), None);
        assert_eq!(Format::Obj.extension(), "obj");
    }

    #[test]
    fn every_format_round_trips_through_read_and_write() {
        let c = cube(10.0);
        for f in [Format::Stl, Format::Obj, Format::ThreeMf] {
            let b = write_mesh(&c, f, "t").unwrap();
            assert_eq!(Format::sniff(&b), Some(f), "{f:?}");
            let back = read_mesh(&b, None, &Limits::default()).unwrap();
            assert_eq!(back.triangles.len(), 12);
            assert!((back.volume_mm3() - 1000.0).abs() < 1e-3);
        }
    }

    #[test]
    fn the_content_wins_over_a_wrong_name() {
        let b = write_mesh(&cube(5.0), Format::ThreeMf, "t").unwrap();
        assert!(read_mesh(&b, Some("model.stl"), &Limits::default()).is_ok());
    }

    #[test]
    fn a_big_or_unknown_file_is_refused() {
        let l = Limits {
            max_file_bytes: 10,
            ..Limits::default()
        };
        assert!(matches!(
            read_mesh(&[0u8; 11], None, &l),
            Err(FormatError::TooLarge { .. })
        ));
        assert!(read_mesh(b"hello", None, &Limits::default()).is_err());
        assert!(read_mesh(b"", Some("a.stl"), &Limits::default()).is_err());
    }
}
