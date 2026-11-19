//! 3MF, the basic part: reads the mesh objects of the core specification (units, build items with their transform) and writes the own package of the program.
//! What it does not read (components, the production extension with objects in other files) is refused with the name of what it found, never skipped.

use crate::error::FormatError;
use crate::xml::{self, Tag};
use crate::zip::{Archive, Writer};
use p3dp_core::{Limits, Mesh, Nm, Point3};

const MODEL: &str = "3D/3dmodel.model";

fn bad(reason: impl Into<String>) -> FormatError {
    FormatError::malformed("3mf", reason)
}

/// The scale of a unit of 3MF to millimetres.
fn unit_scale(unit: &str) -> Result<f64, FormatError> {
    Ok(match unit {
        "micron" => 0.001,
        "millimeter" => 1.0,
        "centimeter" => 10.0,
        "inch" => 25.4,
        "foot" => 304.8,
        "meter" => 1000.0,
        other => return Err(bad(format!("the unit {other:?} is not known"))),
    })
}

#[derive(Default)]
struct Object {
    id: String,
    vertices: Vec<[f64; 3]>,
    triangles: Vec<[u32; 3]>,
    has_mesh: bool,
}

fn parse_f64(tag: &Tag, key: &str) -> Result<f64, FormatError> {
    let t = tag
        .attr(key)
        .ok_or_else(|| bad(format!("<{}> has no {key}", tag.name)))?;
    let v: f64 = t
        .trim()
        .parse()
        .map_err(|_| bad(format!("{key}={t:?} is not a number")))?;
    if !v.is_finite() {
        return Err(bad(format!("{key} is not a finite number")));
    }
    Ok(v)
}

fn parse_index(tag: &Tag, key: &str) -> Result<u32, FormatError> {
    let t = tag
        .attr(key)
        .ok_or_else(|| bad(format!("<{}> has no {key}", tag.name)))?;
    t.trim()
        .parse()
        .map_err(|_| bad(format!("{key}={t:?} is not an index")))
}

/// The 12 numbers of a transform of 3MF: three rows of the matrix and then the translation.
fn parse_transform(text: &str) -> Result<[f64; 12], FormatError> {
    let nums: Vec<f64> = text
        .split_ascii_whitespace()
        .map(|t| t.parse::<f64>().ok().filter(|v| v.is_finite()))
        .collect::<Option<Vec<f64>>>()
        .ok_or_else(|| bad("a transform with a value that is not a finite number"))?;
    <[f64; 12]>::try_from(nums).map_err(|_| bad("a transform needs twelve numbers"))
}

/// Reads the mesh objects of a 3MF package, built from the items of `<build>` (or every object if there are none), in millimetres.
pub fn read(bytes: &[u8], limits: &Limits) -> Result<Mesh, FormatError> {
    let archive = Archive::open(bytes, limits)?;
    let entry = archive
        .find(MODEL)
        .or_else(|| {
            archive
                .entries
                .iter()
                .find(|e| e.name.to_ascii_lowercase().ends_with(".model"))
        })
        .ok_or_else(|| bad("the package has no 3D model"))?;
    let xml_bytes = archive.read(entry)?;
    let text = String::from_utf8(xml_bytes).map_err(|_| bad("the model is not valid UTF-8"))?;
    let tags = xml::tags(&text)?;
    let mut scale = 1.0;
    let mut objects: Vec<Object> = Vec::new();
    let mut items: Vec<(String, Option<[f64; 12]>)> = Vec::new();
    let mut total_vertices = 0usize;
    let mut total_triangles = 0usize;
    for tag in &tags {
        if tag.closing {
            continue;
        }
        match tag.name.as_str() {
            "model" => {
                if let Some(u) = tag.attr("unit") {
                    scale = unit_scale(u)?;
                }
            }
            "object" => {
                objects.push(Object {
                    id: tag.attr("id").unwrap_or("").to_string(),
                    ..Object::default()
                });
            }
            "mesh" => {
                if let Some(o) = objects.last_mut() {
                    o.has_mesh = true;
                }
            }
            "component" | "components" => {
                return Err(FormatError::unsupported(
                    "3mf",
                    "components (an object made of other objects)",
                ))
            }
            "vertex" => {
                total_vertices += 1;
                if total_vertices > limits.max_vertices {
                    return Err(bad(format!("more than {} vertices", limits.max_vertices)));
                }
                let o = objects
                    .last_mut()
                    .ok_or_else(|| bad("a vertex outside an object"))?;
                o.vertices.push([
                    parse_f64(tag, "x")?,
                    parse_f64(tag, "y")?,
                    parse_f64(tag, "z")?,
                ]);
            }
            "triangle" => {
                total_triangles += 1;
                if total_triangles > limits.max_triangles {
                    return Err(bad(format!("more than {} triangles", limits.max_triangles)));
                }
                let o = objects
                    .last_mut()
                    .ok_or_else(|| bad("a triangle outside an object"))?;
                o.triangles.push([
                    parse_index(tag, "v1")?,
                    parse_index(tag, "v2")?,
                    parse_index(tag, "v3")?,
                ]);
            }
            "item" => {
                if tag.attr("p:path").is_some()
                    || tag.attrs.iter().any(|(k, _)| k.ends_with(":path"))
                {
                    return Err(FormatError::unsupported(
                        "3mf",
                        "the production extension (objects in other files)",
                    ));
                }
                let id = tag
                    .attr("objectid")
                    .ok_or_else(|| bad("an item with no objectid"))?
                    .to_string();
                let tr = match tag.attr("transform") {
                    Some(t) => Some(parse_transform(t)?),
                    None => None,
                };
                items.push((id, tr));
            }
            _ => {}
        }
    }
    if !objects.is_empty() && objects.iter().all(|o| !o.has_mesh) {
        return Err(FormatError::unsupported("3mf", "objects without a mesh"));
    }
    let chosen: Vec<(&Object, Option<[f64; 12]>)> = if items.is_empty() {
        objects
            .iter()
            .filter(|o| o.has_mesh)
            .map(|o| (o, None))
            .collect()
    } else {
        let mut v = Vec::new();
        for (id, tr) in &items {
            let o = objects.iter().find(|o| &o.id == id).ok_or_else(|| {
                bad(format!(
                    "the item uses the object {id}, which is not in the model"
                ))
            })?;
            if !o.has_mesh {
                return Err(FormatError::unsupported(
                    "3mf",
                    format!("the object {id} has no mesh"),
                ));
            }
            v.push((o, *tr));
        }
        v
    };
    let mut mesh = Mesh::new();
    for (o, tr) in chosen {
        let base = mesh.vertices.len() as u32;
        for p in &o.vertices {
            let mut q = [p[0] * scale, p[1] * scale, p[2] * scale];
            if let Some(m) = tr {
                let (x, y, z) = (p[0], p[1], p[2]);
                q = [
                    (x * m[0] + y * m[3] + z * m[6] + m[9]) * scale,
                    (x * m[1] + y * m[4] + z * m[7] + m[10]) * scale,
                    (x * m[2] + y * m[5] + z * m[8] + m[11]) * scale,
                ];
            }
            mesh.vertices.push(Point3::from_mm(q[0], q[1], q[2])?);
        }
        for t in &o.triangles {
            if t.iter().any(|&i| i as usize >= o.vertices.len()) {
                return Err(bad(format!(
                    "a triangle of the object {} uses a vertex that does not exist",
                    o.id
                )));
            }
            mesh.triangles.push([base + t[0], base + t[1], base + t[2]]);
        }
    }
    if mesh.triangles.is_empty() {
        return Err(bad("the package has no triangles"));
    }
    Ok(mesh)
}

/// The information that goes into a package that this program writes.
#[derive(Debug, Clone, Default)]
pub struct Meta {
    pub title: String,
    /// The id of the printer profile the project was made for (empty when none).
    pub printer: String,
}

/// Writes the own 3MF package: the mesh in millimetres with the exact decimals, the title and the printer as metadata.
pub fn write(mesh: &Mesh, meta: &Meta) -> Result<Vec<u8>, FormatError> {
    if mesh
        .triangles
        .iter()
        .any(|t| t.iter().any(|&i| i as usize >= mesh.vertices.len()))
    {
        return Err(bad("a triangle uses a vertex that does not exist"));
    }
    let mut m = String::with_capacity(64 + mesh.vertices.len() * 64 + mesh.triangles.len() * 48);
    m.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<model unit=\"millimeter\" xml:lang=\"en-US\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n");
    m.push_str(&format!(
        " <metadata name=\"Title\">{}</metadata>\n",
        xml::escape(&meta.title)
    ));
    m.push_str(" <metadata name=\"Application\">PHOENIX-3DP</metadata>\n");
    if !meta.printer.is_empty() {
        m.push_str(&format!(
            " <metadata name=\"Phoenix3dpPrinter\">{}</metadata>\n",
            xml::escape(&meta.printer)
        ));
    }
    m.push_str(" <resources>\n  <object id=\"1\" type=\"model\">\n   <mesh>\n    <vertices>\n");
    for v in &mesh.vertices {
        m.push_str(&format!(
            "     <vertex x=\"{}\" y=\"{}\" z=\"{}\"/>\n",
            dec(v.x),
            dec(v.y),
            dec(v.z)
        ));
    }
    m.push_str("    </vertices>\n    <triangles>\n");
    for t in &mesh.triangles {
        m.push_str(&format!(
            "     <triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>\n",
            t[0], t[1], t[2]
        ));
    }
    m.push_str("    </triangles>\n   </mesh>\n  </object>\n </resources>\n <build>\n  <item objectid=\"1\"/>\n </build>\n</model>\n");
    let types = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"model\" ContentType=\"application/vnd.ms-package.3dmanufacturing-3dmodel+xml\"/></Types>";
    let rels = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Target=\"/3D/3dmodel.model\" Id=\"rel0\" Type=\"http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel\"/></Relationships>";
    let mut z = Writer::new();
    z.add("[Content_Types].xml", types.as_bytes())?;
    z.add("_rels/.rels", rels.as_bytes())?;
    z.add(MODEL, m.as_bytes())?;
    Ok(z.finish())
}

fn dec(n: Nm) -> String {
    n.to_decimal_mm()
}

/// The title and the printer that a package written by `write` carries (empty when there are none).
pub fn read_meta(bytes: &[u8], limits: &Limits) -> Result<Meta, FormatError> {
    let archive = Archive::open(bytes, limits)?;
    let Some(entry) = archive.find(MODEL) else {
        return Err(bad("the package has no 3D model"));
    };
    let text =
        String::from_utf8(archive.read(entry)?).map_err(|_| bad("the model is not valid UTF-8"))?;
    let mut meta = Meta::default();
    for key in ["Title", "Phoenix3dpPrinter"] {
        let open = format!("<metadata name=\"{key}\">");
        if let Some(s) = text.find(&open) {
            let from = s + open.len();
            if let Some(e) = text[from..].find("</metadata>") {
                let value = xml::unescape(&text[from..from + e]);
                if key == "Title" {
                    meta.title = value;
                } else {
                    meta.printer = value;
                }
            }
        }
    }
    Ok(meta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_core::mesh::cube;
    use p3dp_core::rng::Rng;

    #[test]
    fn what_the_program_writes_it_reads_back_exactly() {
        let c = cube(12.345678);
        let pkg = write(
            &c,
            &Meta {
                title: "a <cube> & more".into(),
                printer: "bambu-lab-a1".into(),
            },
        )
        .unwrap();
        assert_eq!(read(&pkg, &Limits::default()).unwrap(), c);
        let m = read_meta(&pkg, &Limits::default()).unwrap();
        assert_eq!(m.title, "a <cube> & more");
        assert_eq!(m.printer, "bambu-lab-a1");
    }

    #[test]
    fn a_package_of_a_slicer_with_deflate_and_a_transform() {
        let z = include_bytes!("../tests/fixtures/cube_deflated.3mf");
        let m = read(z, &Limits::default()).unwrap();
        assert_eq!(m.triangles.len(), 12);
        let (lo, hi) = m.bounds().unwrap();
        // the item moves the cube 5 mm along x
        assert_eq!(lo.x.to_mm(), 5.0);
        assert_eq!(hi.x.to_mm(), 15.0);
        assert!((m.volume_mm3() - 1000.0).abs() < 1e-6);
    }

    fn package(model: &str) -> Vec<u8> {
        let mut z = Writer::new();
        z.add(MODEL, model.as_bytes()).unwrap();
        z.finish()
    }

    #[test]
    fn units_are_converted() {
        let model = "<model unit=\"inch\"><resources><object id=\"1\"><mesh><vertices><vertex x=\"0\" y=\"0\" z=\"0\"/><vertex x=\"1\" y=\"0\" z=\"0\"/><vertex x=\"0\" y=\"1\" z=\"0\"/></vertices><triangles><triangle v1=\"0\" v2=\"1\" v3=\"2\"/></triangles></mesh></object></resources></model>";
        let m = read(&package(model), &Limits::default()).unwrap();
        assert_eq!(m.size_mm(), Some([25.4, 25.4, 0.0]));
        assert!(read(
            &package(&model.replace("inch", "furlong")),
            &Limits::default()
        )
        .is_err());
    }

    #[test]
    fn what_is_not_supported_is_named() {
        let comp = "<model><resources><object id=\"1\"><components><component objectid=\"2\"/></components></object></resources></model>";
        let e = read(&package(comp), &Limits::default())
            .unwrap_err()
            .to_string();
        assert!(e.contains("components"), "{e}");
        let prod = "<model><resources><object id=\"1\"><mesh/></object></resources><build><item objectid=\"1\" p:path=\"/3D/Objects/o.model\"/></build></model>";
        let e = read(&package(prod), &Limits::default())
            .unwrap_err()
            .to_string();
        assert!(e.contains("production"), "{e}");
    }

    #[test]
    fn broken_models_are_refused() {
        let v = "<vertices><vertex x=\"0\" y=\"0\" z=\"0\"/></vertices>";
        let cases = [
            format!("<model><resources><object id=\"1\"><mesh>{v}<triangles><triangle v1=\"0\" v2=\"0\" v3=\"5\"/></triangles></mesh></object></resources></model>"),
            "<model><resources><object id=\"1\"><mesh><vertices><vertex x=\"a\" y=\"0\" z=\"0\"/></vertices></mesh></object></resources></model>".to_string(),
            format!("<model><resources><object id=\"1\"><mesh>{v}</mesh></object></resources></model>"),
            format!("<model><resources><object id=\"1\"><mesh>{v}</mesh></object></resources><build><item objectid=\"9\"/></build></model>"),
            "<model><resources><object id=\"1\"><mesh><vertices><vertex x=\"nan\" y=\"0\" z=\"0\"/></vertices></mesh></object></resources></model>".to_string(),
            "<model><build><item objectid=\"1\" transform=\"1 2 3\"/></build></model>".to_string(),
            "<model".to_string(),
        ];
        for c in cases {
            assert!(read(&package(&c), &Limits::default()).is_err(), "{c}");
        }
        assert!(read(&[], &Limits::default()).is_err());
        let mut z = Writer::new();
        z.add("other.txt", b"x").unwrap();
        assert!(read(&z.finish(), &Limits::default()).is_err());
    }

    #[test]
    fn limits_apply_to_the_model() {
        let c = cube(5.0);
        let pkg = write(&c, &Meta::default()).unwrap();
        let l = Limits {
            max_triangles: 5,
            ..Limits::default()
        };
        assert!(read(&pkg, &l).is_err());
        let l = Limits {
            max_vertices: 4,
            ..Limits::default()
        };
        assert!(read(&pkg, &l).is_err());
    }

    #[test]
    fn damaged_packages_never_panic() {
        let z = include_bytes!("../tests/fixtures/cube_deflated.3mf");
        let own = write(&cube(5.0), &Meta::default()).unwrap();
        let mut r = Rng::new(555);
        for _ in 0..300 {
            for src in [&z[..], &own[..]] {
                let mut d = src.to_vec();
                for _ in 0..1 + r.below(5) {
                    let i = r.below(d.len() as u64) as usize;
                    d[i] = r.next_u64() as u8;
                }
                let _ = read(&d, &Limits::default());
            }
        }
    }
}
