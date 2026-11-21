//! STL: reads the binary and the ASCII form (it tells them apart by the size, not by the word `solid`), and writes the binary form.

use crate::error::FormatError;
use p3dp_core::{Limits, Mesh};

fn bad(reason: impl Into<String>) -> FormatError {
    FormatError::malformed("stl", reason)
}

/// Reads an STL file. The number of triangles is checked against the limit before anything is allocated; a value that is not finite is an error.
pub fn read(bytes: &[u8], limits: &Limits) -> Result<Mesh, FormatError> {
    if bytes.len() >= 84 {
        let n = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
        if 84usize.checked_add(n.saturating_mul(50)) == Some(bytes.len()) {
            return read_binary(bytes, n, limits);
        }
    }
    let head = &bytes[..bytes.len().min(512)];
    let looks_ascii = String::from_utf8_lossy(head)
        .trim_start()
        .to_ascii_lowercase()
        .starts_with("solid");
    if looks_ascii {
        // only the words `vertex` and the numbers count: a name in another encoding (Latin-1) must not refuse the file
        let text = String::from_utf8_lossy(bytes);
        return read_ascii(&text, limits);
    }
    if bytes.len() < 84 {
        return Err(bad("the file is too short to be an STL"));
    }
    Err(bad(
        "the size does not match the number of triangles in the header, and it is not text",
    ))
}

fn read_binary(bytes: &[u8], n: usize, limits: &Limits) -> Result<Mesh, FormatError> {
    if n == 0 {
        return Err(bad("the file has no triangles"));
    }
    if n > limits.max_triangles {
        return Err(bad(format!(
            "{n} triangles, more than the limit of {}",
            limits.max_triangles
        )));
    }
    let mut soup = Vec::with_capacity(n);
    for t in 0..n {
        let base = 84 + t * 50 + 12; // skip the normal
        let mut tri = [[0.0f64; 3]; 3];
        for (v, corner) in tri.iter_mut().enumerate() {
            for (k, c) in corner.iter_mut().enumerate() {
                let o = base + (v * 3 + k) * 4;
                let f = f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
                if !f.is_finite() {
                    return Err(bad(format!(
                        "triangle {t} has a coordinate that is not a finite number"
                    )));
                }
                *c = f64::from(f);
            }
        }
        soup.push(tri);
    }
    Ok(Mesh::from_soup(&soup, limits)?)
}

fn read_ascii(text: &str, limits: &Limits) -> Result<Mesh, FormatError> {
    let mut words = text.split_ascii_whitespace();
    let mut corners: Vec<[f64; 3]> = Vec::new();
    while let Some(w) = words.next() {
        if w.eq_ignore_ascii_case("vertex") {
            let mut c = [0.0f64; 3];
            for slot in &mut c {
                let t = words.next().ok_or_else(|| bad("a vertex is cut short"))?;
                let v: f64 = t
                    .parse()
                    .map_err(|_| bad(format!("{t:?} is not a number")))?;
                if !v.is_finite() {
                    return Err(bad("a coordinate is not a finite number"));
                }
                *slot = v;
            }
            corners.push(c);
            if corners.len() / 3 > limits.max_triangles {
                return Err(bad(format!("more than {} triangles", limits.max_triangles)));
            }
        }
    }
    if corners.is_empty() {
        return Err(bad("the file has no triangles"));
    }
    let (triangles, rest) = corners.as_chunks::<3>();
    if !rest.is_empty() {
        return Err(bad(format!(
            "{} vertices are not a whole number of triangles",
            corners.len()
        )));
    }
    let soup: Vec<[[f64; 3]; 3]> = triangles.to_vec();
    Ok(Mesh::from_soup(&soup, limits)?)
}

/// Writes a binary STL (the coordinates are single precision, as the format says; the normals are worked out).
pub fn write_binary(mesh: &Mesh) -> Result<Vec<u8>, FormatError> {
    let n = mesh.triangles.len();
    let mut out = Vec::with_capacity(84 + n * 50);
    let mut head = [0u8; 80];
    let tag = b"PHOENIX-3DP binary STL";
    head[..tag.len()].copy_from_slice(tag);
    out.extend_from_slice(&head);
    out.extend_from_slice(&(n as u32).to_le_bytes());
    for t in &mesh.triangles {
        let mut p = [[0.0f64; 3]; 3];
        for (k, &i) in t.iter().enumerate() {
            let v = mesh.vertices.get(i as usize).ok_or_else(|| {
                FormatError::malformed(
                    "stl",
                    format!("a triangle uses vertex {i}, which does not exist"),
                )
            })?;
            p[k] = v.mm();
        }
        let u = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
        let w = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
        let mut nrm = [
            u[1] * w[2] - u[2] * w[1],
            u[2] * w[0] - u[0] * w[2],
            u[0] * w[1] - u[1] * w[0],
        ];
        let len = (nrm[0] * nrm[0] + nrm[1] * nrm[1] + nrm[2] * nrm[2]).sqrt();
        if len > 0.0 {
            for c in &mut nrm {
                *c /= len;
            }
        }
        for c in nrm.iter().chain(p.iter().flatten()) {
            out.extend_from_slice(&(*c as f32).to_le_bytes());
        }
        out.extend_from_slice(&[0, 0]);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_core::mesh::cube;
    use p3dp_core::rng::Rng;

    const ASCII: &str = "solid cube\n facet normal 0 0 -1\n  outer loop\n   vertex 0 0 0\n   vertex 0 10 0\n   vertex 10 10 0\n  endloop\n endfacet\n facet normal 0 0 -1\n  outer loop\n   vertex 0 0 0\n   vertex 10 10 0\n   vertex 10 0 0\n  endloop\n endfacet\nendsolid cube\n";

    #[test]
    fn reads_ascii() {
        let m = read(ASCII.as_bytes(), &Limits::default()).unwrap();
        assert_eq!(m.triangles.len(), 2);
        assert_eq!(m.vertices.len(), 4);
        assert_eq!(m.size_mm(), Some([10.0, 10.0, 0.0]));
    }

    #[test]
    fn binary_round_trip_of_a_cube() {
        let c = cube(20.0);
        let b = write_binary(&c).unwrap();
        assert_eq!(b.len(), 84 + 12 * 50);
        let back = read(&b, &Limits::default()).unwrap();
        assert_eq!(back.triangles.len(), 12);
        assert_eq!(back.vertices.len(), 8);
        assert!((back.volume_mm3() - 8000.0).abs() < 1e-3);
    }

    #[test]
    fn a_binary_file_that_starts_with_solid_is_still_binary() {
        let mut b = write_binary(&cube(5.0)).unwrap();
        b[..5].copy_from_slice(b"solid");
        assert_eq!(read(&b, &Limits::default()).unwrap().triangles.len(), 12);
    }

    #[test]
    fn a_header_that_promises_too_much_is_refused_before_allocating() {
        let mut b = vec![0u8; 84];
        b[80..84].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(read(&b, &Limits::default()).is_err());
        let mut b2 = write_binary(&cube(5.0)).unwrap();
        let l = Limits {
            max_triangles: 5,
            ..Limits::default()
        };
        assert!(read(&b2, &l).is_err());
        b2[80..84].copy_from_slice(&0u32.to_le_bytes());
        b2.truncate(84);
        assert!(read(&b2, &Limits::default()).is_err());
    }

    #[test]
    fn nonsense_is_refused_with_a_reason() {
        assert!(read(b"", &Limits::default()).is_err());
        assert!(read(b"solid x\nvertex 1 2\n", &Limits::default()).is_err());
        assert!(read(
            b"solid x\nvertex 1 2 nan\nvertex 0 0 0\nvertex 1 1 1\n",
            &Limits::default()
        )
        .is_err());
        assert!(read(b"solid x\nvertex 1 2 3\nvertex 0 0 0\n", &Limits::default()).is_err());
        assert!(read(b"solid x\nendsolid x\n", &Limits::default()).is_err());
        assert!(read(&[7u8; 200], &Limits::default()).is_err());
        let mut b = write_binary(&cube(5.0)).unwrap();
        b[100..104].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(read(&b, &Limits::default())
            .unwrap_err()
            .to_string()
            .contains("finite"));
    }

    #[test]
    fn random_bytes_never_panic() {
        let mut r = Rng::new(4242);
        let good = write_binary(&cube(5.0)).unwrap();
        for _ in 0..300 {
            let n = r.below(300) as usize;
            let _ = read(&r.bytes(n), &Limits::default());
            let mut d = good.clone();
            let i = r.below(d.len() as u64) as usize;
            d[i] = r.next_u64() as u8;
            let _ = read(&d, &Limits::default());
        }
        for _ in 0..100 {
            let n = r.below(200) as usize;
            let s: String = (0..n)
                .map(|_| {
                    [
                        "solid ", "vertex ", "1 ", "-2.5e3 ", "\n", "nan ", "facet ", "x ",
                    ][r.below(8) as usize]
                })
                .collect();
            let _ = read(s.as_bytes(), &Limits::default());
            let _ = read(format!("solid a {s}").as_bytes(), &Limits::default());
        }
    }
}
