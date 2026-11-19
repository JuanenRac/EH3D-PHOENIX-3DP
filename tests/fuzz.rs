//! Fuzz tests with a fixed seed: valid files of every format are cut, damaged and mixed, and the readers must answer with an error or a mesh, never panic and never allocate
//! what the header promises. The same seed gives the same files on every run.

use p3dp_core::mesh::cube;
use p3dp_core::rng::Rng;
use p3dp_core::Limits;
use p3dp_formats::{read_mesh, write_mesh, Format};

fn seeds() -> Vec<(Format, Vec<u8>)> {
    let c = cube(10.0);
    vec![
        (Format::Stl, write_mesh(&c, Format::Stl, "").unwrap()),
        (Format::Obj, write_mesh(&c, Format::Obj, "").unwrap()),
        (
            Format::ThreeMf,
            write_mesh(&c, Format::ThreeMf, "seed").unwrap(),
        ),
        (
            Format::ThreeMf,
            include_bytes!("../crates/p3dp-formats/tests/fixtures/cube_deflated.3mf").to_vec(),
        ),
    ]
}

fn small() -> Limits {
    Limits {
        max_file_bytes: 1 << 20,
        max_triangles: 10_000,
        max_vertices: 30_000,
        max_zip_entries: 64,
        max_inflated_bytes: 4 << 20,
        ..Limits::default()
    }
}

#[test]
fn a_file_cut_at_every_position_is_an_error_or_a_mesh() {
    for (f, bytes) in seeds() {
        for cut in 0..bytes.len() {
            let _ = read_mesh(&bytes[..cut], None, &small());
        }
        assert!(
            read_mesh(&bytes, None, &small()).is_ok(),
            "{f:?}: the whole file must read"
        );
    }
}

#[test]
fn a_whole_file_with_a_byte_changed_never_panics() {
    let mut rng = Rng::new(0xF00D);
    for (_, bytes) in seeds() {
        for _ in 0..1500 {
            let mut d = bytes.clone();
            for _ in 0..1 + rng.below(4) {
                let i = rng.below(d.len() as u64) as usize;
                d[i] = match rng.below(3) {
                    0 => rng.next_u64() as u8,
                    1 => 0xFF,
                    _ => 0,
                };
            }
            if let Ok(m) = read_mesh(&d, None, &small()) {
                // whatever came out must be safe to ask about
                let r = m.validate(&small());
                let _ = r.is_ok();
                let _ = m.volume_mm3();
                let _ = m.open_edges();
            }
        }
    }
}

#[test]
fn files_made_of_nothing_but_noise_never_panic() {
    let mut rng = Rng::new(0xBEEF);
    for _ in 0..2000 {
        let n = rng.below(400) as usize;
        let d = rng.bytes(n);
        let _ = read_mesh(&d, None, &small());
        for name in ["a.stl", "a.obj", "a.3mf"] {
            let _ = read_mesh(&d, Some(name), &small());
        }
    }
}

#[test]
fn the_header_of_a_binary_stl_cannot_ask_for_gigabytes() {
    for n in [u32::MAX, 0x7FFF_FFFF, 10_001, 5_000_000] {
        let mut b = vec![0u8; 84 + 50];
        b[80..84].copy_from_slice(&n.to_le_bytes());
        assert!(read_mesh(&b, Some("x.stl"), &small()).is_err(), "{n}");
    }
}

#[test]
fn text_formats_made_of_pieces_of_their_own_words_never_panic() {
    let mut rng = Rng::new(0xCAFE);
    let obj = [
        "v ",
        "f ",
        "vn ",
        "1 ",
        "2 ",
        "3 ",
        "-1 ",
        "0 ",
        "1/2/3 ",
        "\n",
        "# c\n",
        "nan ",
        "1e400 ",
        "99999999999 ",
    ];
    let stl = [
        "solid ",
        "facet normal ",
        "vertex ",
        "endloop ",
        "0 ",
        "1.5 ",
        "-2e3 ",
        "\n",
        "endsolid ",
        "inf ",
        "x ",
    ];
    for _ in 0..2000 {
        let o: String = (0..rng.below(60))
            .map(|_| obj[rng.below(obj.len() as u64) as usize])
            .collect();
        let _ = read_mesh(o.as_bytes(), Some("a.obj"), &small());
        let s: String = std::iter::once("solid a ")
            .chain((0..rng.below(60)).map(|_| stl[rng.below(stl.len() as u64) as usize]))
            .collect();
        let _ = read_mesh(s.as_bytes(), Some("a.stl"), &small());
    }
}

#[test]
fn zip_bombs_and_deep_nesting_are_refused() {
    // a stored entry that claims a size far beyond its bytes
    let mut z = p3dp_formats::zip::Writer::new();
    z.add("3D/3dmodel.model", &[b' '; 100]).unwrap();
    let mut bytes = z.finish();
    // the directory says 4 GB of data in the entry
    let central = bytes
        .windows(4)
        .position(|w| w == [0x50, 0x4b, 0x01, 0x02])
        .unwrap();
    bytes[central + 24..central + 28].copy_from_slice(&0xFFFF_FFF0u32.to_le_bytes());
    assert!(read_mesh(&bytes, Some("x.3mf"), &small()).is_err());
    // an XML nested ten thousand deep is only tags to the reader: it must finish and refuse the model
    let deep = format!("<model>{}</model>", "<a>".repeat(10_000));
    let mut z = p3dp_formats::zip::Writer::new();
    z.add("3D/3dmodel.model", deep.as_bytes()).unwrap();
    assert!(read_mesh(&z.finish(), Some("x.3mf"), &small()).is_err());
}
