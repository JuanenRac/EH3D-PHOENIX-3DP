//! OBJ (Wavefront): the vertices and the faces; the faces with more than three corners are cut into triangles. Everything else (normals, textures, materials) is ignored.

use crate::error::FormatError;
use p3dp_core::{Limits, Mesh, Point3};

fn bad(line: usize, reason: impl Into<String>) -> FormatError {
    FormatError::malformed("obj", format!("line {line}: {}", reason.into()))
}

/// Reads an OBJ file. An index that is zero or does not exist, a coordinate that is not finite and more vertices or triangles than the limits are errors that name the line.
pub fn read(text: &str, limits: &Limits) -> Result<Mesh, FormatError> {
    let mut mesh = Mesh::new();
    for (i, raw) in text.lines().enumerate() {
        let ln = i + 1;
        let line = raw.split('#').next().unwrap_or("").trim();
        let mut parts = line.split_ascii_whitespace();
        match parts.next() {
            Some("v") => {
                let mut c = [0.0f64; 3];
                for slot in &mut c {
                    let t = parts
                        .next()
                        .ok_or_else(|| bad(ln, "a vertex has fewer than three numbers"))?;
                    *slot = t
                        .parse()
                        .map_err(|_| bad(ln, format!("{t:?} is not a number")))?;
                }
                if mesh.vertices.len() >= limits.max_vertices {
                    return Err(bad(
                        ln,
                        format!("more than {} vertices", limits.max_vertices),
                    ));
                }
                mesh.vertices
                    .push(Point3::from_mm(c[0], c[1], c[2]).map_err(|e| bad(ln, e.to_string()))?);
            }
            Some("f") => {
                let mut idx: Vec<u32> = Vec::new();
                for t in parts {
                    let first = t.split('/').next().unwrap_or("");
                    let n: i64 = first
                        .parse()
                        .map_err(|_| bad(ln, format!("{t:?} is not a vertex index")))?;
                    let count = mesh.vertices.len() as i64;
                    let k = if n > 0 {
                        n - 1
                    } else if n < 0 {
                        count + n
                    } else {
                        return Err(bad(ln, "the index 0 does not exist (they start at 1)"));
                    };
                    if k < 0 || k >= count {
                        return Err(bad(
                            ln,
                            format!("the vertex {n} does not exist (there are {count} so far)"),
                        ));
                    }
                    idx.push(k as u32);
                }
                if idx.len() < 3 {
                    return Err(bad(ln, "a face has fewer than three corners"));
                }
                for w in 1..idx.len() - 1 {
                    if mesh.triangles.len() >= limits.max_triangles {
                        return Err(bad(
                            ln,
                            format!("more than {} triangles", limits.max_triangles),
                        ));
                    }
                    mesh.triangles.push([idx[0], idx[w], idx[w + 1]]);
                }
            }
            _ => {}
        }
    }
    if mesh.triangles.is_empty() {
        return Err(FormatError::malformed("obj", "the file has no faces"));
    }
    Ok(mesh)
}

/// Writes an OBJ file with the exact coordinates.
pub fn write(mesh: &Mesh) -> Result<String, FormatError> {
    let mut s = String::from("# PHOENIX-3DP\n");
    for v in &mesh.vertices {
        s.push_str(&format!(
            "v {} {} {}\n",
            v.x.to_decimal_mm(),
            v.y.to_decimal_mm(),
            v.z.to_decimal_mm()
        ));
    }
    for t in &mesh.triangles {
        if t.iter().any(|&i| i as usize >= mesh.vertices.len()) {
            return Err(FormatError::malformed(
                "obj",
                "a triangle uses a vertex that does not exist",
            ));
        }
        s.push_str(&format!("f {} {} {}\n", t[0] + 1, t[1] + 1, t[2] + 1));
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_core::mesh::cube;
    use p3dp_core::rng::Rng;

    #[test]
    fn round_trip_is_exact() {
        let c = cube(12.5);
        let text = write(&c).unwrap();
        let back = read(&text, &Limits::default()).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn quads_are_cut_and_slashes_and_negatives_work() {
        let t = "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nvt 0 0\nvn 0 0 1\nf 1/1/1 2/1/1 3/1/1 4/1/1 # a quad\nf -4 -3 -2\n";
        let m = read(t, &Limits::default()).unwrap();
        assert_eq!(m.triangles, vec![[0, 1, 2], [0, 2, 3], [0, 1, 2]]);
    }

    #[test]
    fn bad_input_names_the_line() {
        let cases = [
            ("v 0 0\n", "line 1"),
            ("v 0 0 x\n", "line 1"),
            ("v 0 0 0\nf 1 2 3\n", "line 2"),
            ("v 0 0 0\nv 1 0 0\nv 0 1 0\nf 0 1 2\n", "line 4"),
            ("v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2\n", "line 4"),
            ("v 0 0 nan\n", "line 1"),
            ("v 0 0 0\n", "no faces"),
        ];
        for (t, want) in cases {
            let e = read(t, &Limits::default()).unwrap_err().to_string();
            assert!(e.contains(want), "{t:?} -> {e}");
        }
    }

    #[test]
    fn limits_apply() {
        let l = Limits {
            max_vertices: 2,
            ..Limits::default()
        };
        assert!(read("v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n", &l).is_err());
        let l = Limits {
            max_triangles: 1,
            ..Limits::default()
        };
        assert!(read("v 0 0 0\nv 1 0 0\nv 0 1 0\nv 1 1 0\nf 1 2 3 4\n", &l).is_err());
    }

    #[test]
    fn random_text_never_panics() {
        let mut r = Rng::new(808);
        for _ in 0..300 {
            let n = r.below(120) as usize;
            let s: String = (0..n)
                .map(|_| {
                    [
                        "v ",
                        "f ",
                        "1 ",
                        "-1 ",
                        "0 ",
                        "2/3/4 ",
                        "\n",
                        "# ",
                        "x ",
                        "9999999999 ",
                    ][r.below(10) as usize]
                })
                .collect();
            let _ = read(&s, &Limits::default());
        }
    }
}
