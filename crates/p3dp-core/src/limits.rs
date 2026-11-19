//! The limits that keep a hostile or damaged file from asking for gigabytes. Each has a conservative default and a key in the settings file.

use crate::error::CoreError;
use crate::kv;

/// The limits of the readers. A file that goes beyond one is refused with a message; nothing is cut short.
#[derive(Debug, Clone, PartialEq)]
pub struct Limits {
    /// The largest file that is read, in bytes.
    pub max_file_bytes: u64,
    /// The most triangles in one mesh.
    pub max_triangles: usize,
    /// The most vertices in one mesh.
    pub max_vertices: usize,
    /// The largest size of a mesh along any axis, in millimetres.
    pub max_extent_mm: f64,
    /// The most entries in a ZIP package (3MF).
    pub max_zip_entries: usize,
    /// The most bytes that one entry of a ZIP package may inflate to.
    pub max_inflated_bytes: u64,
    /// The largest ratio between the inflated and the packed size of an entry (a bomb is far above it).
    pub max_inflate_ratio: u64,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            max_file_bytes: 256 << 20,
            max_triangles: 5_000_000,
            max_vertices: 15_000_000,
            max_extent_mm: 2_000.0,
            max_zip_entries: 4_096,
            max_inflated_bytes: 512 << 20,
            max_inflate_ratio: 1_000,
        }
    }
}

impl Limits {
    /// The keys of the settings file, in the order they are written.
    pub const KEYS: [&'static str; 7] = [
        "limit_max_file_mib",
        "limit_max_triangles",
        "limit_max_vertices",
        "limit_max_extent_mm",
        "limit_max_zip_entries",
        "limit_max_inflated_mib",
        "limit_max_inflate_ratio",
    ];

    /// Sets one key; a value out of its range is an error that names the key and changes nothing.
    pub fn set(&mut self, key: &str, value: &str) -> Result<bool, CoreError> {
        match key {
            "limit_max_file_mib" => {
                self.max_file_bytes = (kv::int_in(key, value, 1, 4096)? as u64) << 20
            }
            "limit_max_triangles" => {
                self.max_triangles = kv::int_in(key, value, 1, 50_000_000)? as usize
            }
            "limit_max_vertices" => {
                self.max_vertices = kv::int_in(key, value, 1, 150_000_000)? as usize
            }
            "limit_max_extent_mm" => self.max_extent_mm = kv::float_in(key, value, 1.0, 100_000.0)?,
            "limit_max_zip_entries" => {
                self.max_zip_entries = kv::int_in(key, value, 1, 100_000)? as usize
            }
            "limit_max_inflated_mib" => {
                self.max_inflated_bytes = (kv::int_in(key, value, 1, 8192)? as u64) << 20
            }
            "limit_max_inflate_ratio" => {
                self.max_inflate_ratio = kv::int_in(key, value, 2, 100_000)? as u64
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Reads the keys it knows from the text of a settings file; a key it does not know is left to the program (`Ok` with it ignored).
    pub fn from_text(text: &str) -> Result<Limits, CoreError> {
        let mut l = Limits::default();
        for e in kv::parse(text)? {
            l.set(&e.key, &e.value)
                .map_err(|err| CoreError::Invalid(format!("line {}: {err}", e.line)))?;
        }
        Ok(l)
    }

    /// The settings as text, in the same form `from_text` reads.
    pub fn to_text(&self) -> String {
        format!(
            "limit_max_file_mib = {}\nlimit_max_triangles = {}\nlimit_max_vertices = {}\nlimit_max_extent_mm = {}\nlimit_max_zip_entries = {}\nlimit_max_inflated_mib = {}\nlimit_max_inflate_ratio = {}\n",
            self.max_file_bytes >> 20,
            self.max_triangles,
            self.max_vertices,
            self.max_extent_mm,
            self.max_zip_entries,
            self.max_inflated_bytes >> 20,
            self.max_inflate_ratio
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_conservative_and_round_trip() {
        let d = Limits::default();
        assert_eq!(Limits::from_text(&d.to_text()).unwrap(), d);
        assert!(d.max_triangles <= 10_000_000);
    }

    #[test]
    fn a_key_out_of_range_names_the_line() {
        let e = Limits::from_text("limit_max_triangles = 0\n")
            .unwrap_err()
            .to_string();
        assert!(e.contains("line 1") && e.contains("limit_max_triangles"));
        assert!(Limits::from_text("limit_max_extent_mm = nan\n").is_err());
    }

    #[test]
    fn an_unknown_key_is_left_alone() {
        let mut l = Limits::default();
        assert!(!l.set("server_address", "x").unwrap());
        assert!(Limits::from_text("server_address = a:1\n").is_ok());
    }

    #[test]
    fn every_key_is_known() {
        let mut l = Limits::default();
        for k in Limits::KEYS {
            assert_eq!(l.set(k, "5"), Ok(true), "{k}");
        }
    }
}
