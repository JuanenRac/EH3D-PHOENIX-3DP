//! ZIP packages (the container of 3MF): a reader of stored and deflated entries with limits against bombs, and a writer of stored entries.

use crate::crc32::crc32;
use crate::error::FormatError;
use crate::inflate::inflate;
use p3dp_core::Limits;

const EOCD: u32 = 0x0605_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const LOCAL: u32 = 0x0403_4b50;

/// One entry of the central directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    method: u16,
    crc: u32,
    packed: u64,
    size: u64,
    offset: u64,
}

impl Entry {
    /// The size once it is inflated, as the directory says.
    pub fn size(&self) -> u64 {
        self.size
    }
}

/// A package that has been opened: the directory is read, the entries are inflated one at a time.
#[derive(Debug)]
pub struct Archive<'a> {
    data: &'a [u8],
    pub entries: Vec<Entry>,
    limits: Limits,
}

fn u16_at(d: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(i..i + 2)?.try_into().ok()?))
}

fn u32_at(d: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(i..i + 4)?.try_into().ok()?))
}

fn bad(reason: &str) -> FormatError {
    FormatError::malformed("zip", reason)
}

impl<'a> Archive<'a> {
    /// Opens a package: finds the directory and checks every entry against the limits (their number, their inflated size, their ratio). Encryption and ZIP64 are refused.
    pub fn open(data: &'a [u8], limits: &Limits) -> Result<Archive<'a>, FormatError> {
        let n = data.len();
        if n < 22 {
            return Err(bad("the file is too short to be a package"));
        }
        let lowest = n.saturating_sub(22 + 65_535);
        let mut at = None;
        let mut i = n - 22;
        loop {
            if u32_at(data, i) == Some(EOCD) {
                at = Some(i);
                break;
            }
            if i == lowest {
                break;
            }
            i -= 1;
        }
        let Some(e) = at else {
            return Err(bad("no end of directory (not a ZIP file, or cut short)"));
        };
        let count = u16_at(data, e + 10).ok_or_else(|| bad("a cut directory"))? as usize;
        let cd_size = u32_at(data, e + 12).ok_or_else(|| bad("a cut directory"))? as usize;
        let cd_off = u32_at(data, e + 16).ok_or_else(|| bad("a cut directory"))? as usize;
        if count == 0xFFFF || cd_size == 0xFFFF_FFFF || cd_off == 0xFFFF_FFFF {
            return Err(FormatError::unsupported("zip", "ZIP64"));
        }
        if count > limits.max_zip_entries {
            return Err(bad(&format!(
                "{count} entries, more than the limit of {}",
                limits.max_zip_entries
            )));
        }
        if cd_off.checked_add(cd_size).is_none_or(|end| end > e) {
            return Err(bad("the directory is outside the file"));
        }
        let mut entries = Vec::with_capacity(count);
        let mut p = cd_off;
        for _ in 0..count {
            if u32_at(data, p) != Some(CENTRAL) {
                return Err(bad("a bad entry in the directory"));
            }
            let flags = u16_at(data, p + 8).ok_or_else(|| bad("a cut entry"))?;
            let method = u16_at(data, p + 10).ok_or_else(|| bad("a cut entry"))?;
            let crc = u32_at(data, p + 16).ok_or_else(|| bad("a cut entry"))?;
            let packed = u64::from(u32_at(data, p + 20).ok_or_else(|| bad("a cut entry"))?);
            let size = u64::from(u32_at(data, p + 24).ok_or_else(|| bad("a cut entry"))?);
            let nl = u16_at(data, p + 28).ok_or_else(|| bad("a cut entry"))? as usize;
            let xl = u16_at(data, p + 30).ok_or_else(|| bad("a cut entry"))? as usize;
            let cl = u16_at(data, p + 32).ok_or_else(|| bad("a cut entry"))? as usize;
            let offset = u64::from(u32_at(data, p + 42).ok_or_else(|| bad("a cut entry"))?);
            let name_bytes = data
                .get(p + 46..p + 46 + nl)
                .ok_or_else(|| bad("a cut name"))?;
            let name = String::from_utf8_lossy(name_bytes).into_owned();
            if flags & 1 != 0 {
                return Err(FormatError::unsupported("zip", "an encrypted entry"));
            }
            if method != 0 && method != 8 {
                return Err(FormatError::unsupported(
                    "zip",
                    format!("compression method {method}"),
                ));
            }
            if size > limits.max_inflated_bytes {
                return Err(bad(&format!(
                    "the entry {name} inflates to {size} bytes, more than the limit"
                )));
            }
            if size
                > packed
                    .saturating_mul(limits.max_inflate_ratio)
                    .saturating_add(1 << 16)
            {
                return Err(bad(&format!(
                    "the entry {name} inflates {size} from {packed} bytes: too much to be a model"
                )));
            }
            entries.push(Entry {
                name,
                method,
                crc,
                packed,
                size,
                offset,
            });
            p += 46 + nl + xl + cl;
        }
        Ok(Archive {
            data,
            entries,
            limits: limits.clone(),
        })
    }

    /// The entry with a name (the comparison ignores the case, and a leading slash).
    pub fn find(&self, name: &str) -> Option<&Entry> {
        let want = name.trim_start_matches('/').to_ascii_lowercase();
        self.entries
            .iter()
            .find(|e| e.name.trim_start_matches('/').to_ascii_lowercase() == want)
    }

    /// Reads one entry: inflates it, checks its size and its CRC.
    pub fn read(&self, entry: &Entry) -> Result<Vec<u8>, FormatError> {
        let d = self.data;
        let o = usize::try_from(entry.offset).map_err(|_| bad("an offset that is too large"))?;
        if u32_at(d, o) != Some(LOCAL) {
            return Err(bad(&format!(
                "the entry {} has no local header",
                entry.name
            )));
        }
        let nl = u16_at(d, o + 26).ok_or_else(|| bad("a cut header"))? as usize;
        let xl = u16_at(d, o + 28).ok_or_else(|| bad("a cut header"))? as usize;
        let start = o + 30 + nl + xl;
        let end = start
            .checked_add(entry.packed as usize)
            .ok_or_else(|| bad("a size that is too large"))?;
        let raw = d
            .get(start..end)
            .ok_or_else(|| bad(&format!("the entry {} is cut short", entry.name)))?;
        let out = if entry.method == 0 {
            raw.to_vec()
        } else {
            let cap = (entry.size as usize).min(self.limits.max_inflated_bytes as usize);
            inflate(raw, cap).map_err(|e| bad(&format!("the entry {}: {e}", entry.name)))?
        };
        if out.len() as u64 != entry.size {
            return Err(bad(&format!(
                "the entry {} has {} bytes, the directory says {}",
                entry.name,
                out.len(),
                entry.size
            )));
        }
        if crc32(&out) != entry.crc {
            return Err(bad(&format!(
                "the entry {} does not match its checksum",
                entry.name
            )));
        }
        Ok(out)
    }
}

/// Writes a package of stored entries. The date is fixed, so the same entries always give the same bytes.
#[derive(Debug, Default)]
pub struct Writer {
    body: Vec<u8>,
    central: Vec<u8>,
    count: u16,
    names: Vec<String>,
}

impl Writer {
    pub fn new() -> Writer {
        Writer::default()
    }

    /// Adds an entry. A name that is empty, has a backslash or `..`, or was already added is an error.
    pub fn add(&mut self, name: &str, data: &[u8]) -> Result<(), FormatError> {
        if name.is_empty()
            || name.contains('\\')
            || name.split('/').any(|p| p == "..")
            || name.starts_with('/')
            || name.len() > 1000
        {
            return Err(FormatError::malformed(
                "zip",
                format!("the name {name:?} is not a safe entry name"),
            ));
        }
        if self.names.iter().any(|n| n == name) {
            return Err(FormatError::malformed(
                "zip",
                format!("the name {name} is used twice"),
            ));
        }
        if self.count == 0xFFFE || data.len() >= 0xFFFF_FFFF || self.body.len() >= 0xFFFF_0000 {
            return Err(FormatError::unsupported(
                "zip",
                "a package that needs ZIP64",
            ));
        }
        let crc = crc32(data);
        let offset = self.body.len() as u32;
        let nb = name.as_bytes();
        let mut local = Vec::new();
        local.extend_from_slice(&LOCAL.to_le_bytes());
        local.extend_from_slice(&[20, 0, 0x00, 0x08, 0, 0, 0, 0, 0x21, 0]); // version, flags (UTF-8), method 0, time, date
        local.extend_from_slice(&crc.to_le_bytes());
        local.extend_from_slice(&(data.len() as u32).to_le_bytes());
        local.extend_from_slice(&(data.len() as u32).to_le_bytes());
        local.extend_from_slice(&(nb.len() as u16).to_le_bytes());
        local.extend_from_slice(&[0, 0]);
        local.extend_from_slice(nb);
        self.body.extend_from_slice(&local);
        self.body.extend_from_slice(data);
        let c = &mut self.central;
        c.extend_from_slice(&CENTRAL.to_le_bytes());
        c.extend_from_slice(&[20, 0, 20, 0, 0x00, 0x08, 0, 0, 0, 0, 0x21, 0]);
        c.extend_from_slice(&crc.to_le_bytes());
        c.extend_from_slice(&(data.len() as u32).to_le_bytes());
        c.extend_from_slice(&(data.len() as u32).to_le_bytes());
        c.extend_from_slice(&(nb.len() as u16).to_le_bytes());
        c.extend_from_slice(&[0; 12]); // extra, comment, disk, internal and external attributes
        c.extend_from_slice(&offset.to_le_bytes());
        c.extend_from_slice(nb);
        self.count += 1;
        self.names.push(name.to_string());
        Ok(())
    }

    /// The finished package.
    pub fn finish(self) -> Vec<u8> {
        let mut out = self.body;
        let cd_off = out.len() as u32;
        let cd_size = self.central.len() as u32;
        out.extend_from_slice(&self.central);
        out.extend_from_slice(&EOCD.to_le_bytes());
        out.extend_from_slice(&[0, 0, 0, 0]);
        out.extend_from_slice(&self.count.to_le_bytes());
        out.extend_from_slice(&self.count.to_le_bytes());
        out.extend_from_slice(&cd_size.to_le_bytes());
        out.extend_from_slice(&cd_off.to_le_bytes());
        out.extend_from_slice(&[0, 0]);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_core::rng::Rng;

    fn sample() -> Vec<u8> {
        let mut w = Writer::new();
        w.add("a.txt", b"hello").unwrap();
        w.add("dir/b.bin", &[1, 2, 3, 4]).unwrap();
        w.add("empty", b"").unwrap();
        w.finish()
    }

    #[test]
    fn what_is_written_is_read() {
        let z = sample();
        let a = Archive::open(&z, &Limits::default()).unwrap();
        assert_eq!(a.entries.len(), 3);
        assert_eq!(a.read(a.find("A.TXT").unwrap()).unwrap(), b"hello");
        assert_eq!(
            a.read(a.find("/dir/b.bin").unwrap()).unwrap(),
            vec![1, 2, 3, 4]
        );
        assert_eq!(a.read(a.find("empty").unwrap()).unwrap(), Vec::<u8>::new());
        assert!(a.find("nothing").is_none());
    }

    #[test]
    fn the_same_entries_give_the_same_bytes() {
        assert_eq!(sample(), sample());
    }

    #[test]
    fn a_deflated_package_from_a_real_tool() {
        let z = include_bytes!("../tests/fixtures/cube_deflated.3mf");
        let a = Archive::open(z, &Limits::default()).unwrap();
        assert_eq!(a.entries.len(), 3);
        let m = a.read(a.find("3D/3dmodel.model").unwrap()).unwrap();
        assert!(String::from_utf8(m).unwrap().contains("<vertex"));
    }

    #[test]
    fn unsafe_names_and_repeats_are_refused() {
        let mut w = Writer::new();
        for bad in ["", "../x", "a/../b", "/abs", "a\\b"] {
            assert!(w.add(bad, b"x").is_err(), "{bad}");
        }
        w.add("ok", b"x").unwrap();
        assert!(w.add("ok", b"y").is_err());
    }

    #[test]
    fn a_bomb_and_too_many_entries_are_refused() {
        let z = include_bytes!("../tests/fixtures/cube_deflated.3mf");
        let tight = Limits {
            max_inflated_bytes: 100,
            ..Limits::default()
        };
        assert!(Archive::open(z, &tight).is_err());
        let few = Limits {
            max_zip_entries: 2,
            ..Limits::default()
        };
        assert!(Archive::open(z, &few).is_err());
        let ratio = Limits {
            max_inflate_ratio: 2,
            ..Limits::default()
        };
        // the sample is small enough to stay under the allowance of 64 KiB, so the ratio is not what stops it
        assert!(Archive::open(z, &ratio).is_ok());
    }

    #[test]
    fn damage_is_an_error() {
        let z = sample();
        assert!(Archive::open(&z[..10], &Limits::default()).is_err());
        assert!(Archive::open(&[], &Limits::default()).is_err());
        let mut c = z.clone();
        c[35] ^= 0xFF; // the first byte of the data of the first entry
        let a = Archive::open(&c, &Limits::default()).unwrap();
        let first = a.entries[0].clone();
        assert!(a.read(&first).is_err());
        let mut no_local = z.clone();
        no_local[0] = 0;
        let a = Archive::open(&no_local, &Limits::default()).unwrap();
        assert!(a.read(&a.entries[0].clone()).is_err());
    }

    #[test]
    fn random_damage_never_panics() {
        let z = include_bytes!("../tests/fixtures/cube_deflated.3mf");
        let mut r = Rng::new(31337);
        for _ in 0..400 {
            let mut d = z.to_vec();
            for _ in 0..1 + r.below(6) {
                let i = r.below(d.len() as u64) as usize;
                d[i] = r.next_u64() as u8;
            }
            if r.below(4) == 0 {
                d.truncate(r.below(d.len() as u64) as usize);
            }
            if let Ok(a) = Archive::open(&d, &Limits::default()) {
                for e in a.entries.clone() {
                    let _ = a.read(&e);
                }
            }
        }
    }
}
