//! Edge cases of the readers and writers of the files a model and a print come in: the checksum and the inflate, the ZIP package, the XML reader, STL, OBJ, 3MF, the pictures,
//! the figures of a print, the thumbnails of a G-code file and the header of a binary G-code file. Damaged, cut, hostile and odd files are given to every reader: it refuses them with
//! a reason and never panics. The random bytes come from a seeded generator, so every run sees the same files.

use p3dp_core::mesh::cube;
use p3dp_core::rng::Rng;
use p3dp_core::{Limits, Mesh};
use p3dp_formats::bgcode::{self, BlockKind, Checksum, Compression};
use p3dp_formats::crc32::crc32;
use p3dp_formats::detect::{read_mesh, write_mesh, Format};
use p3dp_formats::gcode_meta::{self, ScanLimits};
use p3dp_formats::inflate::inflate;
use p3dp_formats::picture::PictureFormat;
use p3dp_formats::summary::{duration_seconds, first_number, PrintSummary};
use p3dp_formats::zip::{Archive, Writer};
use p3dp_formats::{gcode_file, obj, png_write, stl, threemf, xml, FormatError};

fn limits() -> Limits {
    Limits::default()
}

fn malformed(e: &FormatError, format: &str) -> bool {
    matches!(e, FormatError::Malformed { format: f, .. } if *f == format)
}

// ---------------------------------------------------------------- crc32 and inflate

#[test]
fn the_checksum_of_nothing_is_zero() {
    assert_eq!(crc32(b""), 0);
}

#[test]
fn the_checksum_of_the_check_string_is_the_known_value() {
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
}

#[test]
fn the_checksum_of_one_zero_byte_is_the_known_value() {
    assert_eq!(crc32(&[0]), 0xD202_EF8D);
}

#[test]
fn the_checksum_changes_when_one_bit_changes() {
    let mut rng = Rng::new(0xC4C);
    for _ in 0..50 {
        let n = 1 + rng.below(64) as usize;
        let data = rng.bytes(n);
        let mut other = data.clone();
        let at = rng.below(n as u64) as usize;
        other[at] ^= 1 << rng.below(8);
        assert_ne!(crc32(&data), crc32(&other));
    }
}

#[test]
fn a_stored_block_is_copied() {
    let packed = [1, 3, 0, 0xFC, 0xFF, b'a', b'b', b'c'];
    assert_eq!(inflate(&packed, 100).unwrap(), b"abc");
}

#[test]
fn a_block_of_fixed_codes_is_inflated() {
    assert_eq!(inflate(&[0x4b, 0x04, 0x00], 100).unwrap(), b"a");
    assert_eq!(
        inflate(&[0x4b, 0x4c, 0x4a, 0x06, 0x00], 100).unwrap(),
        b"abc"
    );
    assert!(inflate(&[0x03, 0x00], 100).unwrap().is_empty());
}

#[test]
fn a_stored_block_with_a_wrong_complement_is_refused() {
    assert!(inflate(&[1, 3, 0, 0, 0, b'a', b'b', b'c'], 100).is_err());
}

#[test]
fn an_output_over_the_limit_is_refused_and_one_at_the_limit_is_not() {
    let packed = [1, 3, 0, 0xFC, 0xFF, b'a', b'b', b'c'];
    assert!(inflate(&packed, 2).is_err());
    assert!(inflate(&packed, 3).is_ok());
    assert!(inflate(&[0x4b, 0x04, 0x00], 0).is_err());
}

#[test]
fn a_stream_that_is_cut_or_empty_or_of_a_reserved_kind_is_refused() {
    assert!(inflate(&[], 100).is_err());
    assert!(inflate(&[0x4b], 100).is_err());
    assert!(inflate(&[1, 3, 0, 0xFC, 0xFF, b'a'], 100).is_err());
    assert!(inflate(&[0x07, 0, 0, 0], 100).is_err());
}

#[test]
fn any_bytes_are_inflated_or_refused_without_a_panic() {
    let mut rng = Rng::new(0x1F);
    for _ in 0..400 {
        let n = rng.below(80) as usize;
        let data = rng.bytes(n);
        if let Ok(out) = inflate(&data, 1 << 12) {
            assert!(out.len() <= 1 << 12);
        }
    }
}

// ---------------------------------------------------------------- zip

fn sample_zip() -> Vec<u8> {
    let mut w = Writer::new();
    w.add("a.txt", b"hello").unwrap();
    w.add("dir/b.bin", &[1, 2, 3, 4]).unwrap();
    w.add("empty", b"").unwrap();
    w.finish()
}

#[test]
fn what_a_package_writer_adds_a_reader_gives_back() {
    let z = sample_zip();
    let a = Archive::open(&z, &limits()).unwrap();
    assert_eq!(a.entries.len(), 3);
    let by_name = |n: &str| a.read(a.find(n).unwrap()).unwrap();
    assert_eq!(by_name("a.txt"), b"hello");
    assert_eq!(by_name("dir/b.bin"), [1, 2, 3, 4]);
    assert!(by_name("empty").is_empty());
    assert_eq!(a.find("a.txt").unwrap().size(), 5);
}

#[test]
fn the_same_entries_always_give_the_same_bytes() {
    assert_eq!(sample_zip(), sample_zip());
}

#[test]
fn an_entry_is_found_without_regard_to_the_case_or_a_leading_slash() {
    let z = sample_zip();
    let a = Archive::open(&z, &limits()).unwrap();
    assert!(a.find("A.TXT").is_some());
    assert!(a.find("/dir/B.bin").is_some());
    assert!(a.find("nothing").is_none());
    assert!(a.find("").is_none());
}

#[test]
fn a_name_that_is_not_safe_is_refused_when_adding() {
    let mut w = Writer::new();
    let long = "x".repeat(1001);
    for bad in ["", "a\\b", "../x", "a/../b", "/abs", long.as_str()] {
        assert!(w.add(bad, b"1").is_err(), "{bad:?}");
    }
    assert!(w.add("a/b.c", b"1").is_ok());
    assert!(w.add("..x", b"1").is_ok());
}

#[test]
fn a_name_used_twice_is_refused() {
    let mut w = Writer::new();
    w.add("a", b"1").unwrap();
    assert!(w.add("a", b"2").is_err());
}

#[test]
fn a_package_with_no_entry_is_valid_and_empty() {
    let z = Writer::new().finish();
    let a = Archive::open(&z, &limits()).unwrap();
    assert!(a.entries.is_empty());
}

#[test]
fn a_file_that_is_too_short_or_not_a_package_is_refused() {
    for n in 0..22 {
        assert!(Archive::open(&vec![0u8; n], &limits()).is_err());
    }
    assert!(Archive::open(&[7u8; 200], &limits()).is_err());
}

#[test]
fn a_package_cut_anywhere_is_refused() {
    let z = sample_zip();
    for len in 0..z.len() {
        assert!(Archive::open(&z[..len], &limits()).is_err(), "{len}");
    }
}

#[test]
fn a_changed_byte_of_an_entry_is_caught_by_its_checksum() {
    let mut w = Writer::new();
    w.add("a.txt", b"hello").unwrap();
    let mut z = w.finish();
    z[30 + 5] ^= 0xFF;
    let a = Archive::open(&z, &limits()).unwrap();
    let e = a.read(a.find("a.txt").unwrap()).unwrap_err();
    assert!(malformed(&e, "zip"), "{e}");
}

#[test]
fn more_entries_than_the_limit_or_a_bigger_entry_than_the_limit_are_refused() {
    let z = sample_zip();
    let few = Limits {
        max_zip_entries: 2,
        ..limits()
    };
    assert!(Archive::open(&z, &few).is_err());
    let exact = Limits {
        max_zip_entries: 3,
        ..limits()
    };
    assert!(Archive::open(&z, &exact).is_ok());
    let small = Limits {
        max_inflated_bytes: 3,
        ..limits()
    };
    assert!(Archive::open(&z, &small).is_err());
}

#[test]
fn an_entry_of_another_package_is_refused_and_does_not_stop_the_reader() {
    let big = sample_zip();
    let mut w = Writer::new();
    w.add("z", b"1").unwrap();
    let small = w.finish();
    let a_big = Archive::open(&big, &limits()).unwrap();
    let a_small = Archive::open(&small, &limits()).unwrap();
    for e in &a_big.entries {
        let _ = a_small.read(e);
    }
}

#[test]
fn bytes_that_look_like_a_package_never_stop_the_reader() {
    let mut rng = Rng::new(0x21B);
    for _ in 0..300 {
        let n = 22 + rng.below(200) as usize;
        let mut data = rng.bytes(n);
        let at = n - 22;
        data[at..at + 4].copy_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
        if let Ok(a) = Archive::open(&data, &limits()) {
            for e in &a.entries {
                let _ = a.read(e);
            }
        }
    }
}

// ---------------------------------------------------------------- xml

#[test]
fn the_tags_of_a_small_document_are_read_with_their_attributes() {
    let t = xml::tags("<a x=\"1\" y='2'><b/></a>").unwrap();
    assert_eq!(t.len(), 3);
    assert_eq!(t[0].name, "a");
    assert_eq!(t[0].attr("x"), Some("1"));
    assert_eq!(t[0].attr("y"), Some("2"));
    assert_eq!(t[0].attr("z"), None);
    assert!(!t[0].closing && !t[0].self_closing);
    assert!(t[1].self_closing && t[1].name == "b");
    assert!(t[2].closing && t[2].name == "a");
}

#[test]
fn comments_declarations_and_cdata_are_skipped() {
    let t = xml::tags("<?xml version=\"1.0\"?><!-- <fake/> --><!DOCTYPE x><![CDATA[ <no/> ]]><a/>")
        .unwrap();
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].name, "a");
}

#[test]
fn an_entity_declaration_is_refused() {
    let doc = "<!DOCTYPE x [<!ENTITY a \"b\">]><x/>";
    assert!(matches!(
        xml::tags(doc),
        Err(FormatError::Unsupported { format: "xml", .. })
    ));
}

#[test]
fn a_document_that_is_cut_or_broken_is_refused() {
    for doc in [
        "<a",
        "<a x",
        "<a x=1>",
        "<a x=\"1>",
        "<!-- never closed",
        "<![CDATA[ never closed",
        "<? never closed",
        "<>",
        "<",
        "<a / >",
    ] {
        assert!(xml::tags(doc).is_err(), "{doc:?}");
    }
}

#[test]
fn a_text_with_no_tags_has_no_tags() {
    assert!(xml::tags("").unwrap().is_empty());
    assert!(xml::tags("just text").unwrap().is_empty());
}

#[test]
fn the_values_of_the_attributes_are_unescaped() {
    let t = xml::tags("<a v=\"&lt;&amp;&gt;&quot;&apos;&#65;&#x42;\"/>").unwrap();
    assert_eq!(t[0].attr("v"), Some("<&>\"'AB"));
}

#[test]
fn a_reference_that_is_not_one_stays_as_it_is() {
    assert_eq!(xml::unescape("a & b"), "a & b");
    assert_eq!(xml::unescape("&unknown;"), "&unknown;");
    assert_eq!(xml::unescape("&#xD800;"), "&#xD800;");
    assert_eq!(xml::unescape("&#1114112;"), "&#1114112;");
    assert_eq!(xml::unescape("&"), "&");
    assert_eq!(xml::unescape("&;"), "&;");
    assert_eq!(xml::unescape("no references"), "no references");
}

#[test]
fn escaping_turns_the_five_characters_into_references() {
    assert_eq!(
        xml::escape("<a href=\"x\">&'"),
        "&lt;a href=&quot;x&quot;&gt;&amp;&apos;"
    );
    assert_eq!(xml::escape("é日本"), "é日本");
}

#[test]
fn escaping_drops_the_control_characters_but_keeps_the_line_break_and_the_tab() {
    assert_eq!(xml::escape("a\u{1}b\u{0}c\u{1f}"), "abc");
    assert_eq!(xml::escape("a\nb\tc"), "a\nb\tc");
}

#[test]
fn what_is_escaped_is_unescaped_to_the_same_text() {
    let alphabet: Vec<char> = "&<>\"' ab;#x1é日".chars().collect();
    let mut rng = Rng::new(0xE5C);
    for _ in 0..300 {
        let n = rng.below(24) as usize;
        let s: String = (0..n)
            .map(|_| alphabet[rng.below(alphabet.len() as u64) as usize])
            .collect();
        assert_eq!(xml::unescape(&xml::escape(&s)), s, "{s:?}");
    }
}

#[test]
fn any_text_of_a_small_alphabet_is_read_without_a_panic() {
    let alphabet: Vec<char> = "<>/=\"' ab!-?[]é".chars().collect();
    let mut rng = Rng::new(0x71A);
    for _ in 0..500 {
        let n = rng.below(40) as usize;
        let s: String = (0..n)
            .map(|_| alphabet[rng.below(alphabet.len() as u64) as usize])
            .collect();
        let _ = xml::tags(&s);
    }
}

// ---------------------------------------------------------------- stl

fn stl_bytes(triangles: &[[f32; 9]]) -> Vec<u8> {
    let mut b = vec![0u8; 80];
    b.extend_from_slice(&(triangles.len() as u32).to_le_bytes());
    for t in triangles {
        b.extend_from_slice(&[0u8; 12]);
        for v in t {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&[0u8; 2]);
    }
    b
}

const TRI: [f32; 9] = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];

const TEXT_TRI: &str = "solid x\n facet normal 0 0 0\n  outer loop\n   vertex 0 0 0\n   vertex 1 0 0\n   vertex 0 1 0\n  endloop\n endfacet\nendsolid x\n";

#[test]
fn a_cube_goes_through_a_binary_stl_and_back() {
    let c = cube(10.0);
    let bytes = stl::write_binary(&c).unwrap();
    assert_eq!(bytes.len(), 84 + 12 * 50);
    let back = stl::read(&bytes, &limits()).unwrap();
    assert_eq!(back.triangles.len(), 12);
    assert_eq!(back.vertices.len(), 8);
    assert!((back.volume_mm3() - 1000.0).abs() < 1e-6);
}

#[test]
fn a_binary_stl_whose_header_says_solid_is_read_as_binary_when_its_size_fits() {
    let mut bytes = stl::write_binary(&cube(5.0)).unwrap();
    bytes[..5].copy_from_slice(b"solid");
    assert_eq!(stl::read(&bytes, &limits()).unwrap().triangles.len(), 12);
}

#[test]
fn a_binary_stl_with_no_triangle_is_refused() {
    let e = stl::read(&stl_bytes(&[]), &limits()).unwrap_err();
    assert!(malformed(&e, "stl"));
}

#[test]
fn a_binary_stl_with_a_coordinate_that_is_not_finite_is_refused() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut t = TRI;
        t[4] = bad;
        assert!(stl::read(&stl_bytes(&[t]), &limits()).is_err());
    }
}

#[test]
fn a_binary_stl_with_a_count_that_does_not_fit_the_size_is_refused() {
    let mut bytes = stl_bytes(&[TRI, TRI]);
    bytes[80] = 3;
    assert!(stl::read(&bytes, &limits()).is_err());
    bytes[80..84].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(stl::read(&bytes, &limits()).is_err());
    let cut = stl_bytes(&[TRI, TRI]);
    assert!(stl::read(&cut[..cut.len() - 1], &limits()).is_err());
}

#[test]
fn more_triangles_than_the_limit_are_refused_in_binary_and_in_text() {
    let l = Limits {
        max_triangles: 1,
        ..limits()
    };
    assert!(stl::read(&stl_bytes(&[TRI, TRI]), &l).is_err());
    assert!(stl::read(&stl_bytes(&[TRI]), &l).is_ok());
    let two = format!("{TEXT_TRI}{TEXT_TRI}");
    assert!(stl::read(two.as_bytes(), &l).is_err());
    assert!(stl::read(TEXT_TRI.as_bytes(), &l).is_ok());
}

#[test]
fn a_text_stl_is_read_with_any_indentation_case_and_line_end() {
    let mesh = stl::read(TEXT_TRI.as_bytes(), &limits()).unwrap();
    assert_eq!(mesh.triangles.len(), 1);
    let crlf = TEXT_TRI.replace('\n', "\r\n");
    assert_eq!(stl::read(crlf.as_bytes(), &limits()).unwrap(), mesh);
    let upper = TEXT_TRI
        .replace("vertex", "VERTEX")
        .replace("solid", "SOLID");
    assert_eq!(stl::read(upper.as_bytes(), &limits()).unwrap(), mesh);
}

#[test]
fn a_text_stl_whose_name_is_not_utf8_is_still_read() {
    let mut bytes = b"solid caf\xe9\n".to_vec();
    bytes.extend_from_slice(&TEXT_TRI.as_bytes()["solid x\n".len()..]);
    let mesh = stl::read(&bytes, &limits()).unwrap();
    assert_eq!(mesh.triangles.len(), 1);
    assert_eq!(read_mesh(&bytes, Some("a.stl"), &limits()).unwrap(), mesh);
}

#[test]
fn a_text_stl_with_a_bad_vertex_or_a_cut_triangle_is_refused() {
    let nan = TEXT_TRI.replace("vertex 0 1 0", "vertex 0 nan 0");
    assert!(stl::read(nan.as_bytes(), &limits()).is_err());
    let word = TEXT_TRI.replace("vertex 0 1 0", "vertex 0 one 0");
    assert!(stl::read(word.as_bytes(), &limits()).is_err());
    let cut = "solid x\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1";
    assert!(stl::read(cut.as_bytes(), &limits()).is_err());
    let two = "solid x\nvertex 0 0 0\nvertex 1 0 0\nendsolid";
    assert!(stl::read(two.as_bytes(), &limits()).is_err());
    assert!(stl::read(b"solid x\nendsolid x\n", &limits()).is_err());
}

#[test]
fn two_triangles_that_share_corners_share_vertices() {
    let two = "solid x\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nvertex 1 0 0\nvertex 1 1 0\nvertex 0 1 0\n";
    let mesh = stl::read(two.as_bytes(), &limits()).unwrap();
    assert_eq!((mesh.vertices.len(), mesh.triangles.len()), (4, 2));
}

#[test]
fn a_file_that_is_neither_binary_nor_text_is_refused_with_the_format_named() {
    for bytes in [&b""[..], &b"hello"[..], &[0xFF; 100][..], &[0u8; 83][..]] {
        let e = stl::read(bytes, &limits()).unwrap_err();
        assert!(malformed(&e, "stl"), "{e}");
    }
}

#[test]
fn a_mesh_with_a_triangle_that_uses_a_missing_vertex_is_not_written() {
    let bad = Mesh {
        vertices: Vec::new(),
        triangles: vec![[0, 1, 2]],
    };
    assert!(stl::write_binary(&bad).is_err());
    assert!(obj::write(&bad).is_err());
    assert!(threemf::write(&bad, &threemf::Meta::default()).is_err());
}

// ---------------------------------------------------------------- obj

const QUAD: &str = "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3 4\n";

#[test]
fn a_face_with_more_corners_is_cut_into_triangles_in_a_fan() {
    let mesh = obj::read(QUAD, &limits()).unwrap();
    assert_eq!(mesh.triangles, [[0, 1, 2], [0, 2, 3]]);
}

#[test]
fn the_indices_may_count_from_the_end_and_carry_texture_and_normal() {
    let text = "v 0 0 0\nv 1 0 0\nv 0 1 0\nf -3/1/1 -2//2 -1\n";
    assert_eq!(obj::read(text, &limits()).unwrap().triangles, [[0, 1, 2]]);
    let slashes = "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1/1/1 2/2/2 3/3/3\n";
    assert_eq!(
        obj::read(slashes, &limits()).unwrap().triangles,
        [[0, 1, 2]]
    );
}

#[test]
fn a_comment_ends_the_line_and_other_words_are_ignored() {
    let text = "# model\no thing\nv 0 0 0 # origin\nv 1 0 0\nv 0 1 0\nvn 0 0 1\nvt 0 0\nusemtl x\ns off\nf 1 2 3 # one face\n";
    let mesh = obj::read(text, &limits()).unwrap();
    assert_eq!((mesh.vertices.len(), mesh.triangles.len()), (3, 1));
}

#[test]
fn an_index_that_is_zero_or_missing_or_not_a_number_is_refused_with_its_line() {
    for (text, line) in [
        ("v 0 0 0\nv 1 0 0\nv 0 1 0\nf 0 1 2\n", "line 4"),
        ("v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 4\n", "line 4"),
        ("v 0 0 0\nv 1 0 0\nv 0 1 0\nf -4 1 2\n", "line 4"),
        ("v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 x\n", "line 4"),
        ("v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2\n", "line 4"),
        ("f 1 2 3\n", "line 1"),
    ] {
        let e = obj::read(text, &limits()).unwrap_err();
        assert!(e.to_string().contains(line), "{text:?}: {e}");
    }
}

#[test]
fn a_vertex_that_is_not_a_usable_number_is_refused_with_its_line() {
    for text in [
        "v 1 2\n",
        "v 1 2 x\n",
        "v 1 2 nan\n",
        "v 1 inf 3\n",
        "v 1e7 0 0\n",
        "v -1e7 0 0\n",
    ] {
        let e = obj::read(text, &limits()).unwrap_err();
        assert!(e.to_string().contains("line 1"), "{text:?}: {e}");
    }
}

#[test]
fn a_file_with_no_face_or_no_text_is_refused() {
    assert!(obj::read("", &limits()).is_err());
    assert!(obj::read("v 0 0 0\nv 1 0 0\n", &limits()).is_err());
    assert!(obj::read("# only a comment\n", &limits()).is_err());
}

#[test]
fn more_vertices_or_triangles_than_the_limit_are_refused() {
    let few_vertices = Limits {
        max_vertices: 3,
        ..limits()
    };
    assert!(obj::read(QUAD, &few_vertices).is_err());
    let few_triangles = Limits {
        max_triangles: 1,
        ..limits()
    };
    assert!(obj::read(QUAD, &few_triangles).is_err());
    let exact = Limits {
        max_vertices: 4,
        max_triangles: 2,
        ..limits()
    };
    assert!(obj::read(QUAD, &exact).is_ok());
}

#[test]
fn a_mesh_goes_through_an_obj_file_and_back_exactly() {
    let mut rng = Rng::new(0x0B7);
    let soup: Vec<[[f64; 3]; 3]> = (0..20)
        .map(|_| {
            let mut t = [[0.0; 3]; 3];
            for corner in &mut t {
                for c in corner.iter_mut() {
                    *c = (rng.range(-50.0, 50.0) * 1000.0).round() / 1000.0;
                }
            }
            t
        })
        .collect();
    let mesh = Mesh::from_soup(&soup, &limits()).unwrap();
    let text = obj::write(&mesh).unwrap();
    assert_eq!(
        text.lines().filter(|l| l.starts_with("v ")).count(),
        mesh.vertices.len()
    );
    assert_eq!(
        text.lines().filter(|l| l.starts_with("f ")).count(),
        mesh.triangles.len()
    );
    assert_eq!(obj::read(&text, &limits()).unwrap(), mesh);
}

#[test]
fn an_obj_file_that_is_written_twice_is_the_same_text() {
    let c = cube(3.0);
    assert_eq!(obj::write(&c).unwrap(), obj::write(&c).unwrap());
    assert!(obj::write(&c).unwrap().starts_with("# PHOENIX-3DP\n"));
}

// ---------------------------------------------------------------- detect

#[test]
fn the_extension_of_a_name_gives_the_format_in_any_case() {
    assert_eq!(Format::from_extension("STL"), Some(Format::Stl));
    assert_eq!(Format::from_extension(".Obj"), Some(Format::Obj));
    assert_eq!(Format::from_extension("3mf"), Some(Format::ThreeMf));
    assert_eq!(Format::from_extension("gcode"), None);
    assert_eq!(Format::from_extension(""), None);
    assert_eq!(Format::from_extension("stl "), None);
}

#[test]
fn the_format_of_a_path_is_the_one_of_its_last_extension() {
    assert_eq!(Format::from_path("a/b.c/d.stl"), Some(Format::Stl));
    assert_eq!(Format::from_path("model.v2.obj"), Some(Format::Obj));
    assert_eq!(Format::from_path("noext"), None);
    assert_eq!(Format::from_path("dir.stl/file"), None);
    assert_eq!(Format::from_path(""), None);
    assert_eq!(Format::from_path("x.stl.bak"), None);
}

#[test]
fn every_format_has_the_extension_that_names_it() {
    for f in [Format::Stl, Format::Obj, Format::ThreeMf] {
        assert_eq!(Format::from_extension(f.extension()), Some(f));
    }
}

#[test]
fn the_start_of_a_file_tells_its_format() {
    assert_eq!(Format::sniff(b"PK\x03\x04rest"), Some(Format::ThreeMf));
    assert_eq!(Format::sniff(b"  solid x"), Some(Format::Stl));
    assert_eq!(Format::sniff(b"SOLID x"), Some(Format::Stl));
    assert_eq!(Format::sniff(b"v 1 2 3"), Some(Format::Obj));
    assert_eq!(Format::sniff(b"# a comment"), Some(Format::Obj));
    assert_eq!(Format::sniff(b"o name"), Some(Format::Obj));
    assert_eq!(Format::sniff(b"mtllib x.mtl"), Some(Format::Obj));
    assert_eq!(Format::sniff(b""), None);
    assert_eq!(Format::sniff(b"hello world"), None);
}

#[test]
fn a_binary_stl_is_told_by_its_size_whatever_its_header_starts_with() {
    for head in [
        "# exported by a tool",
        "v 1 2 3",
        "o thing",
        "mtllib x",
        "PK12",
    ] {
        let mut bytes = stl::write_binary(&cube(4.0)).unwrap();
        bytes[..head.len()].copy_from_slice(head.as_bytes());
        if head.starts_with("PK") {
            continue;
        }
        assert_eq!(Format::sniff(&bytes), Some(Format::Stl), "{head}");
        assert_eq!(
            read_mesh(&bytes, None, &limits()).unwrap().triangles.len(),
            12,
            "{head}"
        );
    }
}

#[test]
fn every_format_is_written_and_read_back_by_the_one_function() {
    let mut rng = Rng::new(0xDE7);
    for _ in 0..6 {
        let side = 1.0 + rng.range(0.0, 40.0);
        let c = cube((side * 100.0).round() / 100.0);
        for f in [Format::Stl, Format::Obj, Format::ThreeMf] {
            let b = write_mesh(&c, f, "title").unwrap();
            let back = read_mesh(&b, None, &limits()).unwrap();
            assert_eq!(back.triangles.len(), 12, "{f:?}");
            assert!(
                (back.volume_mm3() - c.volume_mm3()).abs() < 1e-5 * c.volume_mm3(),
                "{f:?}"
            );
        }
    }
}

#[test]
fn a_name_hint_with_the_wrong_format_does_not_decide() {
    let b = write_mesh(&cube(5.0), Format::Obj, "t").unwrap();
    assert!(read_mesh(&b, Some("model.stl"), &limits()).is_ok());
    assert!(read_mesh(&b, Some("model.3mf"), &limits()).is_ok());
    assert!(read_mesh(&b, Some("noext"), &limits()).is_ok());
}

#[test]
fn a_file_larger_than_the_limit_is_refused_before_it_is_read() {
    let l = Limits {
        max_file_bytes: 10,
        ..limits()
    };
    assert!(matches!(
        read_mesh(&[b'v'; 11], None, &l),
        Err(FormatError::TooLarge { bytes: 11, max: 10 })
    ));
    assert!(!matches!(
        read_mesh(&[b'v'; 10], None, &l),
        Err(FormatError::TooLarge { .. })
    ));
}

#[test]
fn a_file_that_is_no_model_is_refused_with_the_word_model() {
    for bytes in [&b""[..], &b"hello"[..], &b"\x00\x01\x02"[..]] {
        let e = read_mesh(bytes, None, &limits()).unwrap_err();
        assert!(malformed(&e, "model"), "{e}");
    }
}

#[test]
fn an_obj_file_with_a_comment_in_latin_1_is_read() {
    let mut text = b"# caf\xe9\n".to_vec();
    text.extend_from_slice(QUAD.as_bytes());
    let mesh = read_mesh(&text, Some("a.obj"), &limits()).unwrap();
    assert_eq!(mesh.triangles.len(), 2);
}

#[test]
fn any_bytes_are_read_as_a_model_without_a_panic() {
    let mut rng = Rng::new(0xA11);
    for i in 0..300 {
        let n = rng.below(300) as usize;
        let mut data = rng.bytes(n);
        match i % 3 {
            0 => {
                if data.len() >= 4 {
                    data[..4].copy_from_slice(b"PK\x03\x04");
                }
            }
            1 => {
                if data.len() >= 5 {
                    data[..5].copy_from_slice(b"solid");
                }
            }
            _ => {
                if data.len() >= 2 {
                    data[..2].copy_from_slice(b"v ");
                }
            }
        }
        let _ = read_mesh(&data, None, &limits());
    }
}

// ---------------------------------------------------------------- 3mf

fn package(model: &str) -> Vec<u8> {
    let mut w = Writer::new();
    w.add("3D/3dmodel.model", model.as_bytes()).unwrap();
    w.finish()
}

const OBJECT1: &str = "<object id=\"1\"><mesh><vertices><vertex x=\"0\" y=\"0\" z=\"0\"/><vertex x=\"1\" y=\"0\" z=\"0\"/><vertex x=\"0\" y=\"1\" z=\"0\"/></vertices><triangles><triangle v1=\"0\" v2=\"1\" v3=\"2\"/></triangles></mesh></object>";

fn model(unit: &str, resources: &str, build: &str) -> String {
    format!("<?xml version=\"1.0\"?><model unit=\"{unit}\"><resources>{resources}</resources><build>{build}</build></model>")
}

#[test]
fn a_single_triangle_is_read_with_no_build_section_items() {
    let mesh = threemf::read(&package(&model("millimeter", OBJECT1, "")), &limits()).unwrap();
    assert_eq!((mesh.vertices.len(), mesh.triangles.len()), (3, 1));
    assert_eq!(mesh.vertices[1].mm(), [1.0, 0.0, 0.0]);
}

#[test]
fn the_unit_of_the_model_scales_the_coordinates_to_millimetres() {
    for (unit, scale) in [
        ("millimeter", 1.0),
        ("centimeter", 10.0),
        ("meter", 1000.0),
        ("inch", 25.4),
        ("micron", 0.001),
    ] {
        let mesh = threemf::read(
            &package(&model(unit, OBJECT1, "<item objectid=\"1\"/>")),
            &limits(),
        )
        .unwrap();
        assert_eq!(mesh.vertices[1].mm()[0], scale, "{unit}");
    }
}

#[test]
fn a_unit_that_is_not_known_is_refused() {
    let e = threemf::read(&package(&model("furlong", OBJECT1, "")), &limits()).unwrap_err();
    assert!(malformed(&e, "3mf"));
}

#[test]
fn the_transform_of_an_item_moves_and_turns_the_object() {
    let moved = model(
        "millimeter",
        OBJECT1,
        "<item objectid=\"1\" transform=\"1 0 0 0 1 0 0 0 1 5 6 7\"/>",
    );
    let mesh = threemf::read(&package(&moved), &limits()).unwrap();
    assert_eq!(mesh.vertices[0].mm(), [5.0, 6.0, 7.0]);
    assert_eq!(mesh.vertices[1].mm(), [6.0, 6.0, 7.0]);
    let turned = model(
        "millimeter",
        OBJECT1,
        "<item objectid=\"1\" transform=\"0 1 0 -1 0 0 0 0 1 0 0 0\"/>",
    );
    let mesh = threemf::read(&package(&turned), &limits()).unwrap();
    assert_eq!(mesh.vertices[1].mm(), [0.0, 1.0, 0.0]);
}

#[test]
fn a_transform_that_is_not_twelve_finite_numbers_is_refused() {
    for t in [
        "1 0 0 0 1 0 0 0 1 0 0",
        "1 0 0 0 1 0 0 0 1 0 0 0 0",
        "a b c d e f g h i j k l",
        "1 0 0 0 1 0 0 0 1 0 0 nan",
        "",
    ] {
        let m = model(
            "millimeter",
            OBJECT1,
            &format!("<item objectid=\"1\" transform=\"{t}\"/>"),
        );
        assert!(threemf::read(&package(&m), &limits()).is_err(), "{t:?}");
    }
}

#[test]
fn an_object_used_by_two_items_is_in_the_mesh_twice() {
    let m = model(
        "millimeter",
        OBJECT1,
        "<item objectid=\"1\"/><item objectid=\"1\"/>",
    );
    let mesh = threemf::read(&package(&m), &limits()).unwrap();
    assert_eq!((mesh.vertices.len(), mesh.triangles.len()), (6, 2));
    assert_eq!(mesh.triangles[1], [3, 4, 5]);
}

#[test]
fn an_item_for_an_object_that_is_not_there_is_refused() {
    let m = model("millimeter", OBJECT1, "<item objectid=\"9\"/>");
    assert!(threemf::read(&package(&m), &limits()).is_err());
    let none = model("millimeter", OBJECT1, "<item/>");
    assert!(threemf::read(&package(&none), &limits()).is_err());
}

#[test]
fn what_the_reader_does_not_read_is_refused_by_name_and_not_skipped() {
    let parts = "<object id=\"1\"><components><component objectid=\"2\"/></components></object>";
    assert!(matches!(
        threemf::read(&package(&model("millimeter", parts, "")), &limits()),
        Err(FormatError::Unsupported { format: "3mf", .. })
    ));
    let path = model(
        "millimeter",
        OBJECT1,
        "<item objectid=\"1\" p:path=\"/a.model\"/>",
    );
    assert!(matches!(
        threemf::read(&package(&path), &limits()),
        Err(FormatError::Unsupported { format: "3mf", .. })
    ));
    let no_mesh = model("millimeter", "<object id=\"1\"/>", "");
    assert!(matches!(
        threemf::read(&package(&no_mesh), &limits()),
        Err(FormatError::Unsupported { format: "3mf", .. })
    ));
}

#[test]
fn a_vertex_or_a_triangle_that_is_wrong_is_refused() {
    let outside = model("millimeter", "<vertex x=\"0\" y=\"0\" z=\"0\"/>", "");
    assert!(threemf::read(&package(&outside), &limits()).is_err());
    for (from, to) in [
        ("v3=\"2\"", "v3=\"5\""),
        ("v3=\"2\"", "v3=\"-1\""),
        ("v3=\"2\"", "v3=\"x\""),
        ("x=\"1\"", "x=\"nan\""),
        ("x=\"1\"", "x=\"inf\""),
        ("x=\"1\"", "x=\"2000000\""),
        ("x=\"1\"", ""),
    ] {
        let o = OBJECT1.replace(from, to);
        assert!(
            threemf::read(&package(&model("millimeter", &o, "")), &limits()).is_err(),
            "{to}"
        );
    }
}

#[test]
fn a_package_with_no_model_or_no_triangle_is_refused() {
    let mut w = Writer::new();
    w.add("a.txt", b"x").unwrap();
    assert!(threemf::read(&w.finish(), &limits()).is_err());
    let empty = "<object id=\"1\"><mesh><vertices/><triangles/></mesh></object>";
    assert!(threemf::read(&package(&model("millimeter", empty, "")), &limits()).is_err());
    assert!(threemf::read(b"not a package at all, not even close", &limits()).is_err());
}

#[test]
fn a_model_under_another_name_is_found_by_its_extension() {
    let mut w = Writer::new();
    w.add(
        "Objects/part.model",
        model("millimeter", OBJECT1, "").as_bytes(),
    )
    .unwrap();
    assert!(threemf::read(&w.finish(), &limits()).is_ok());
}

#[test]
fn more_vertices_or_triangles_than_the_limit_are_refused_in_a_package() {
    let m = package(&model("millimeter", OBJECT1, ""));
    let few_triangles = Limits {
        max_triangles: 0,
        ..limits()
    };
    assert!(threemf::read(&m, &few_triangles).is_err());
    let few_vertices = Limits {
        max_vertices: 2,
        ..limits()
    };
    assert!(threemf::read(&m, &few_vertices).is_err());
}

#[test]
fn a_package_written_by_the_program_is_read_back_and_is_always_the_same_bytes() {
    let c = cube(12.0);
    let meta = threemf::Meta {
        title: "A part".into(),
        printer: "prusa-mk4s".into(),
    };
    let a = threemf::write(&c, &meta).unwrap();
    assert_eq!(a, threemf::write(&c, &meta).unwrap());
    let back = threemf::read(&a, &limits()).unwrap();
    assert_eq!(back, c);
}

#[test]
fn the_title_and_the_printer_of_a_package_come_back_with_their_special_characters() {
    let meta = threemf::Meta {
        title: "A & B <c> \"d\" 'e' é日".into(),
        printer: "x&y".into(),
    };
    let bytes = threemf::write(&cube(2.0), &meta).unwrap();
    let back = threemf::read_meta(&bytes, &limits()).unwrap();
    assert_eq!(back.title, meta.title);
    assert_eq!(back.printer, meta.printer);
}

#[test]
fn a_title_with_a_control_character_loses_it_and_an_empty_one_stays_empty() {
    let meta = threemf::Meta {
        title: "a\u{1}b".into(),
        printer: String::new(),
    };
    let bytes = threemf::write(&cube(2.0), &meta).unwrap();
    let back = threemf::read_meta(&bytes, &limits()).unwrap();
    assert_eq!(back.title, "ab");
    assert_eq!(back.printer, "");
    let bare = threemf::write(&cube(2.0), &threemf::Meta::default()).unwrap();
    assert_eq!(threemf::read_meta(&bare, &limits()).unwrap().title, "");
}

#[test]
fn the_title_of_a_package_that_is_not_one_is_refused() {
    assert!(threemf::read_meta(b"nothing", &limits()).is_err());
    let mut w = Writer::new();
    w.add("x", b"y").unwrap();
    assert!(threemf::read_meta(&w.finish(), &limits()).is_err());
}

#[test]
fn a_package_cut_anywhere_is_never_read_as_a_model() {
    let bytes = threemf::write(&cube(3.0), &threemf::Meta::default()).unwrap();
    for len in 0..bytes.len() {
        assert!(threemf::read(&bytes[..len], &limits()).is_err(), "{len}");
    }
}

// ---------------------------------------------------------------- pictures

#[test]
fn a_picture_is_known_by_its_code_its_name_and_its_first_bytes() {
    for (code, name, f) in [
        (0, "png", PictureFormat::Png),
        (1, "jpg", PictureFormat::Jpg),
        (2, "qoi", PictureFormat::Qoi),
    ] {
        assert_eq!(PictureFormat::from_code(code), Some(f));
        assert_eq!(PictureFormat::from_name(name), Some(f));
        assert_eq!(f.extension(), name);
    }
    assert_eq!(PictureFormat::from_code(3), None);
    assert_eq!(PictureFormat::from_code(u16::MAX), None);
    assert_eq!(PictureFormat::from_name("JPEG"), Some(PictureFormat::Jpg));
    assert_eq!(PictureFormat::from_name(""), None);
}

#[test]
fn bytes_that_do_not_start_as_the_format_says_do_not_match() {
    let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    assert!(PictureFormat::Png.matches(&png));
    assert!(!PictureFormat::Png.matches(&png[..7]));
    assert!(!PictureFormat::Jpg.matches(&[0xFF, 0xD8]));
    assert!(PictureFormat::Jpg.matches(&[0xFF, 0xD8, 0xFF]));
    assert!(!PictureFormat::Qoi.matches(b"qoi"));
    assert!(PictureFormat::Qoi.matches(b"qoif"));
    assert_eq!(PictureFormat::sniff(&[]), None);
    assert_eq!(PictureFormat::sniff(b"GIF89a"), None);
}

// ---------------------------------------------------------------- the figures of a print

#[test]
fn the_first_number_of_a_text_is_read_and_a_list_gives_its_first() {
    assert_eq!(first_number("12.5"), Some(12.5));
    assert_eq!(first_number("0.4,0.4"), Some(0.4));
    assert_eq!(first_number("1e3"), Some(1000.0));
    assert_eq!(first_number("3; 4"), Some(3.0));
    assert_eq!(first_number("7 g"), Some(7.0));
    assert_eq!(first_number("0"), Some(0.0));
}

#[test]
fn a_number_with_blanks_around_it_is_read() {
    assert_eq!(first_number(" 5"), Some(5.0));
    assert_eq!(first_number("  0.2 "), Some(0.2));
    assert_eq!(first_number("\t8"), Some(8.0));
}

#[test]
fn a_number_that_is_negative_or_not_finite_or_not_a_number_is_left_out() {
    for text in ["", "-1", "-0.5", "inf", "nan", "NaN", "abc", ",5", "1x"] {
        assert_eq!(first_number(text), None, "{text:?}");
    }
}

#[test]
fn a_duration_is_a_count_of_seconds_or_parts_in_order() {
    assert_eq!(duration_seconds("90"), Some(90));
    assert_eq!(duration_seconds("1d 2h 3m 4s"), Some(93_784));
    assert_eq!(duration_seconds("5m"), Some(300));
    assert_eq!(duration_seconds(" 7 "), Some(7));
    assert_eq!(duration_seconds("0s"), Some(0));
    assert_eq!(duration_seconds("1h 1h"), Some(7_200));
}

#[test]
fn a_duration_that_is_wrong_or_too_big_is_left_out() {
    for text in [
        "",
        "   ",
        "5x",
        "1h30m",
        "-5s",
        "é",
        "5é",
        "s",
        "1.5h",
        "18446744073709551615m",
    ] {
        assert_eq!(duration_seconds(text), None, "{text:?}");
    }
    assert_eq!(duration_seconds("18446744073709551615s"), Some(u64::MAX));
    assert_eq!(duration_seconds("18446744073709551615s 1s"), None);
}

#[test]
fn a_summary_of_nothing_is_empty_and_says_nothing() {
    let s = PrintSummary::from_pairs(Vec::<(&str, &str)>::new());
    assert!(s.is_empty());
    assert!(s.lines().is_empty());
    assert_eq!(s, PrintSummary::default());
}

#[test]
fn the_names_of_the_figures_are_read_in_any_letter_case() {
    let s = PrintSummary::from_pairs([
        ("PRINTER_MODEL", "MK4"),
        ("Filament_Type", "PETG"),
        ("LAYER_HEIGHT", "0.2"),
        ("Nozzle_Diameter", "0.4,0.4"),
        ("filament used [g]", "12.5"),
        ("estimated printing time (normal mode)", "1h 2m 3s"),
    ]);
    assert_eq!(s.printer_model.as_deref(), Some("MK4"));
    assert_eq!(s.filament_type.as_deref(), Some("PETG"));
    assert_eq!(s.layer_height_mm, Some(0.2));
    assert_eq!(s.nozzle_mm, Some(0.4));
    assert_eq!(s.filament_g, Some(12.5));
    assert_eq!(s.seconds, Some(3_723));
    assert!(!s.is_empty());
}

#[test]
fn a_figure_that_cannot_be_read_is_skipped_for_the_next_one_with_the_name() {
    let s = PrintSummary::from_pairs([
        ("layer_height", "abc"),
        ("layer_height", "0.15"),
        ("printer_model", "   "),
        ("printer_settings_id", "Backup"),
    ]);
    assert_eq!(s.layer_height_mm, Some(0.15));
    assert_eq!(s.printer_model.as_deref(), Some("Backup"));
    assert_eq!(s.filament_mm, None);
}

#[test]
fn the_lines_of_a_summary_start_with_the_printer() {
    let s = PrintSummary::from_pairs([("printer_model", "X1"), ("layer_height", "0.2")]);
    let lines = s.lines();
    assert_eq!(lines[0], ("printer", "X1".to_string()));
    assert!(lines.len() >= 2);
}

// ---------------------------------------------------------------- the file of G-code

#[test]
fn a_file_with_no_header_and_no_body_is_empty_text() {
    assert_eq!(gcode_file::write(&[], &[]).unwrap(), "");
}

#[test]
fn the_header_lines_are_comments_and_a_blank_line_comes_after_them() {
    let text =
        gcode_file::write(&["a".to_string(), "b: c".to_string()], &["G28".to_string()]).unwrap();
    assert_eq!(text, "; a\n; b: c\n\nG28\n");
}

#[test]
fn a_line_of_the_longest_size_is_written_and_one_byte_more_is_not() {
    let ok = "é".repeat(gcode_file::MAX_LINE / 2);
    assert_eq!(ok.len(), gcode_file::MAX_LINE);
    assert!(gcode_file::write(&[], &[ok]).is_ok());
    let long = format!("{}x", "é".repeat(gcode_file::MAX_LINE / 2));
    assert!(gcode_file::write(&[long], &[]).is_err());
}

#[test]
fn a_line_with_a_break_or_a_zero_in_the_header_or_the_body_is_refused_by_name() {
    for bad in ["a\nb", "a\rb", "a\0b", "\n", "M104 S0\nM104 S999"] {
        let h = gcode_file::write(&[bad.to_string()], &[]).unwrap_err();
        let b = gcode_file::write(&[], &[bad.to_string()]).unwrap_err();
        assert!(h.to_string().starts_with("gcode: "), "{h}");
        assert!(malformed(&b, "gcode"), "{b}");
    }
}

// ---------------------------------------------------------------- png

fn be32(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

#[test]
fn a_picture_of_one_pixel_is_a_png_with_its_three_chunks() {
    let png = png_write::encode_rgba(1, 1, &[1, 2, 3, 4]).unwrap();
    assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    assert_eq!(be32(&png, 8), 13);
    assert_eq!(&png[12..16], b"IHDR");
    assert_eq!((be32(&png, 16), be32(&png, 20)), (1, 1));
    assert_eq!((png[24], png[25]), (8, 6));
    assert_eq!(
        &png[png.len() - 12..],
        &[0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82]
    );
    assert!(PictureFormat::Png.matches(&png));
}

#[test]
fn the_data_of_a_picture_is_inflated_back_row_by_row_with_the_filter_byte() {
    let mut rng = Rng::new(0x9A7);
    for (w, h) in [(1u32, 2u32), (3, 3), (200, 100), (7, 1)] {
        let rgba = rng.bytes(w as usize * h as usize * 4);
        let png = png_write::encode_rgba(w, h, &rgba).unwrap();
        assert_eq!(&png[37..41], b"IDAT");
        let len = be32(&png, 33) as usize;
        let z = &png[41..41 + len];
        let raw = inflate(&z[2..z.len() - 4], 1 << 22).unwrap();
        let mut want = Vec::new();
        for row in rgba.chunks(w as usize * 4) {
            want.push(0);
            want.extend_from_slice(row);
        }
        assert_eq!(raw, want, "{w}x{h}");
    }
}

#[test]
fn a_picture_with_a_size_that_is_not_allowed_is_refused() {
    for (w, h) in [(0u32, 1u32), (1, 0), (16_385, 1), (1, 16_385), (0, 0)] {
        assert!(png_write::encode_rgba(w, h, &[0; 4]).is_err(), "{w}x{h}");
    }
}

#[test]
fn a_picture_whose_bytes_are_not_its_size_is_refused_without_allocating_it() {
    assert!(png_write::encode_rgba(2, 2, &[0; 15]).is_err());
    assert!(png_write::encode_rgba(2, 2, &[0; 17]).is_err());
    assert!(png_write::encode_rgba(16_384, 16_384, &[]).is_err());
    assert!(png_write::encode_rgba(1, 1, &[]).is_err());
}

#[test]
fn the_same_picture_is_always_the_same_bytes() {
    let rgba = [9u8; 16];
    assert_eq!(
        png_write::encode_rgba(2, 2, &rgba).unwrap(),
        png_write::encode_rgba(2, 2, &rgba).unwrap()
    );
}

// ---------------------------------------------------------------- the thumbnails and the notes of a G-code file

fn encode64(data: &[u8]) -> String {
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

const PNG_HEAD: [u8; 12] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3, 4];

fn thumb_block(tag: &str, w: u32, h: u32, data: &[u8], width: usize) -> String {
    let text = encode64(data);
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
fn base64_is_decoded_for_every_length_and_refused_when_it_is_not_base64() {
    let mut rng = Rng::new(0xB64);
    for n in 0..30usize {
        let data = rng.bytes(n);
        assert_eq!(
            gcode_meta::base64_decode(&encode64(&data)).unwrap(),
            data,
            "{n}"
        );
    }
    assert_eq!(gcode_meta::base64_decode("QQ==").unwrap(), b"A");
    assert_eq!(gcode_meta::base64_decode("QUI=").unwrap(), b"AB");
    for bad in ["Q", "Q!==", "QQ===", "QR==", "QU I", "é", "Q\n"] {
        assert_eq!(gcode_meta::base64_decode(bad), None, "{bad:?}");
    }
}

#[test]
fn a_thumbnail_is_found_decoded_and_checked_against_its_format() {
    let text = format!(
        "; generated by a slicer\n{}G28\n",
        thumb_block("thumbnail", 16, 8, &PNG_HEAD, 10)
    );
    let scan = gcode_meta::thumbnails(&text, &ScanLimits::default());
    assert!(scan.problems.is_empty(), "{:?}", scan.problems);
    assert_eq!(scan.thumbnails.len(), 1);
    let t = &scan.thumbnails[0];
    assert_eq!((t.format, t.width, t.height), (PictureFormat::Png, 16, 8));
    assert_eq!(t.data, PNG_HEAD);
}

#[test]
fn the_tags_of_the_other_pictures_are_read_with_their_format() {
    let jpg = [0xFF, 0xD8, 0xFF, 1, 2, 3];
    let qoi = *b"qoif\0\0\0\0";
    let text = format!(
        "{}{}",
        thumb_block("thumbnail_JPG", 4, 4, &jpg, 8),
        thumb_block("thumbnail_QOI", 5, 6, &qoi, 8)
    );
    let scan = gcode_meta::thumbnails(&text, &ScanLimits::default());
    assert!(scan.problems.is_empty(), "{:?}", scan.problems);
    let formats: Vec<PictureFormat> = scan.thumbnails.iter().map(|t| t.format).collect();
    assert_eq!(formats, [PictureFormat::Jpg, PictureFormat::Qoi]);
}

#[test]
fn a_thumbnail_that_is_wrong_is_listed_as_a_problem_with_its_line_and_not_handed_out() {
    let good = thumb_block("thumbnail", 16, 8, &PNG_HEAD, 10);
    let wrong_size = good.replacen(&format!(" {}\n", encode64(&PNG_HEAD).len()), " 99\n", 1);
    let not_png = thumb_block("thumbnail", 16, 8, b"not a picture!", 10);
    let not_base64 = "; thumbnail begin 4x4 4\n; Q!==\n; thumbnail end\n".to_string();
    let not_closed = "; thumbnail begin 4x4 8\n; QUJDREVG\nG1 X1\n".to_string();
    let zero = "; thumbnail begin 0x8 4\n; QUJD\n; thumbnail end\n".to_string();
    for (text, what) in [
        (wrong_size, "size"),
        (not_png, "format"),
        (not_base64, "base64"),
        (not_closed, "closed"),
        (zero, "zero"),
    ] {
        let scan = gcode_meta::thumbnails(&text, &ScanLimits::default());
        assert!(scan.thumbnails.is_empty(), "{what}");
        assert_eq!(scan.problems.len(), 1, "{what}: {:?}", scan.problems);
        assert!(
            scan.problems[0].starts_with("line 1: "),
            "{what}: {:?}",
            scan.problems
        );
    }
}

#[test]
fn the_number_and_the_size_of_the_thumbnails_are_limited() {
    let text = format!(
        "{}{}",
        thumb_block("thumbnail", 16, 8, &PNG_HEAD, 10),
        thumb_block("thumbnail", 16, 8, &PNG_HEAD, 10)
    );
    let one = ScanLimits {
        max_thumbnails: 1,
        ..ScanLimits::default()
    };
    let scan = gcode_meta::thumbnails(&text, &one);
    assert_eq!(scan.thumbnails.len(), 1);
    assert_eq!(scan.problems.len(), 1);
    let small = ScanLimits {
        max_thumbnail_bytes: 3,
        ..ScanLimits::default()
    };
    let scan = gcode_meta::thumbnails(&text, &small);
    assert!(scan.thumbnails.is_empty());
    assert_eq!(scan.problems.len(), 2);
}

#[test]
fn text_with_no_thumbnail_gives_nothing_and_any_text_is_scanned_without_a_panic() {
    assert_eq!(
        gcode_meta::thumbnails("G28\nG1 X1\n", &ScanLimits::default()),
        gcode_meta::ThumbnailScan::default()
    );
    let tiny = ScanLimits {
        head_bytes: 1,
        tail_bytes: 1,
        ..ScanLimits::default()
    };
    let _ = gcode_meta::thumbnails("é; thumbnail begin 1x1 4\n", &tiny);
    let _ = gcode_meta::notes("é; a = b", &tiny);
    let alphabet: Vec<char> = "; thumbnail begin end 1x4=\nQAé".chars().collect();
    let mut rng = Rng::new(0x7B5);
    for _ in 0..300 {
        let n = rng.below(80) as usize;
        let s: String = (0..n)
            .map(|_| alphabet[rng.below(alphabet.len() as u64) as usize])
            .collect();
        let _ = gcode_meta::thumbnails(&s, &ScanLimits::default());
        let _ = gcode_meta::notes(&s, &ScanLimits::default());
    }
}

#[test]
fn the_notes_of_a_file_are_its_comment_lines_with_an_equal_sign() {
    let text = "; a = b\n; just a comment\n; d=e\nG1 X1 ; f = g\n;\n; = nothing\n; thumbnail = x\n";
    let notes = gcode_meta::notes(text, &ScanLimits::default());
    assert_eq!(
        notes,
        [
            ("a".to_string(), "b".to_string()),
            ("d".to_string(), "e".to_string())
        ]
    );
}

#[test]
fn the_most_notes_of_a_part_of_the_file_is_limited_and_the_end_of_a_big_file_is_read() {
    let few = ScanLimits {
        max_notes: 2,
        ..ScanLimits::default()
    };
    let text = "; a = 1\n; b = 2\n; c = 3\n; d = 4\n";
    assert_eq!(gcode_meta::notes(text, &few).len(), 2);
    let mut big = String::from("; first = 1\n");
    big.push_str(&"G1 X1 Y1\n".repeat(2_000));
    big.push_str("; last = 9\n");
    let ends = ScanLimits {
        head_bytes: 100,
        tail_bytes: 100,
        ..ScanLimits::default()
    };
    let notes = gcode_meta::notes(&big, &ends);
    assert_eq!(
        notes,
        [
            ("first".to_string(), "1".to_string()),
            ("last".to_string(), "9".to_string())
        ]
    );
}

#[test]
fn a_long_note_is_cut_to_a_short_length() {
    let text = format!("; k = {}\n", "x".repeat(1000));
    let notes = gcode_meta::notes(&text, &ScanLimits::default());
    assert_eq!(notes[0].1.chars().count(), 300);
}

#[test]
fn the_summary_of_a_plain_file_comes_from_its_comments() {
    let text = "; layer_height = 0.2\n; filament used [g] = 3.5\n; estimated printing time (normal mode) = 12m 30s\nG28\n";
    let s = gcode_meta::summary(text, &ScanLimits::default());
    assert_eq!(s.layer_height_mm, Some(0.2));
    assert_eq!(s.filament_g, Some(3.5));
    assert_eq!(s.seconds, Some(750));
}

// ---------------------------------------------------------------- binary G-code

fn block(
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

fn file(version: u32, checksum_kind: u16, blocks: &[Vec<u8>]) -> Vec<u8> {
    let mut f = bgcode::MAGIC.to_vec();
    f.extend_from_slice(&version.to_le_bytes());
    f.extend_from_slice(&checksum_kind.to_le_bytes());
    for b in blocks {
        f.extend_from_slice(b);
    }
    f
}

fn meta_block(kind: u16, text: &str, crc: bool) -> Vec<u8> {
    block(
        kind,
        0,
        &0u16.to_le_bytes(),
        text.as_bytes(),
        text.len() as u32,
        crc,
    )
}

fn thumb(format: u16, w: u16, h: u16, data: &[u8], compression: u16, crc: bool) -> Vec<u8> {
    let mut params = Vec::new();
    params.extend_from_slice(&format.to_le_bytes());
    params.extend_from_slice(&w.to_le_bytes());
    params.extend_from_slice(&h.to_le_bytes());
    block(5, compression, &params, data, data.len() as u32, crc)
}

const PNG8: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

#[test]
fn the_names_of_the_kinds_of_blocks_and_of_the_compressions_are_the_ones_of_the_reports() {
    assert_eq!(BlockKind::GCode.name(), "g-code");
    assert_eq!(BlockKind::Thumbnail.name(), "thumbnail");
    assert_eq!(BlockKind::Unknown(9).name(), "unknown");
    assert_eq!(Compression::None.name(), "none");
    assert_eq!(Compression::Heatshrink12.name(), "heatshrink 12,4");
    assert!(bgcode::is_bgcode(b"GCDE1234"));
    assert!(!bgcode::is_bgcode(b"GCD"));
}

#[test]
fn a_header_with_a_block_of_metadata_is_read_with_or_without_checksums() {
    for (crc, kind) in [(true, 1u16), (false, 0u16)] {
        let f = file(
            1,
            kind,
            &[meta_block(
                3,
                "printer_model=MK4\nfilament_type = PLA\n",
                crc,
            )],
        );
        let h = bgcode::read_header(&f, &limits()).unwrap();
        assert_eq!(h.version, 1);
        assert_eq!(
            h.checksum,
            if crc { Checksum::Crc32 } else { Checksum::None }
        );
        assert_eq!(h.value("printer_model"), Some("MK4"));
        assert_eq!(h.value("filament_type"), Some("PLA"));
        assert_eq!(h.value("nothing"), None);
        assert_eq!(h.count(BlockKind::PrinterMetadata), 1);
        assert_eq!(h.blocks[0].offset, 10);
        assert_eq!(h.blocks[0].checksum_ok, crc.then_some(true));
        assert!(h.all_checksums_ok());
    }
}

#[test]
fn a_file_that_is_not_binary_g_code_or_has_no_block_is_refused() {
    let e = bgcode::read_header(b"G28\nG1 X1\n", &limits()).unwrap_err();
    assert!(malformed(&e, "bgcode"));
    assert!(bgcode::read_header(b"GCDE", &limits()).is_err());
    assert!(bgcode::read_header(&file(1, 0, &[]), &limits()).is_err());
    assert!(bgcode::read_header(&[], &limits()).is_err());
}

#[test]
fn a_version_or_a_kind_of_checksum_or_block_or_compression_that_is_not_known_is_unsupported() {
    let m = meta_block(0, "a=b\n", false);
    assert!(matches!(
        bgcode::read_header(&file(2, 0, &[m.clone()]), &limits()),
        Err(FormatError::Unsupported {
            format: "bgcode",
            ..
        })
    ));
    assert!(matches!(
        bgcode::read_header(&file(1, 2, &[m]), &limits()),
        Err(FormatError::Unsupported {
            format: "bgcode",
            ..
        })
    ));
    let unknown_kind = block(9, 0, &[0, 0], b"x", 1, false);
    assert!(matches!(
        bgcode::read_header(&file(1, 0, &[unknown_kind]), &limits()),
        Err(FormatError::Unsupported {
            format: "bgcode",
            ..
        })
    ));
    let unknown_compression = block(1, 9, &[0, 0], b"x", 1, false);
    assert!(matches!(
        bgcode::read_header(&file(1, 0, &[unknown_compression]), &limits()),
        Err(FormatError::Unsupported {
            format: "bgcode",
            ..
        })
    ));
}

#[test]
fn a_block_that_runs_past_the_end_of_the_file_is_refused() {
    let mut f = file(1, 0, &[block(1, 0, &[0, 0], b"G28\nG1\n", 7, false)]);
    f.truncate(f.len() - 1);
    assert!(bgcode::read_header(&f, &limits()).is_err());
    let huge = block(1, 1, &[0, 0], b"xx", 2, false);
    let mut g = file(1, 0, &[huge]);
    g[18..22].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(bgcode::read_header(&g, &limits()).is_err());
}

#[test]
fn a_file_cut_anywhere_is_never_read_as_a_whole_and_never_stops_the_reader() {
    let f = file(
        1,
        1,
        &[
            meta_block(0, "Producer=x\n", true),
            thumb(0, 8, 8, &PNG8, 0, true),
            block(1, 1, &[0, 0], &[0u8; 20], 200, true),
        ],
    );
    assert!(bgcode::read_header(&f, &limits()).is_ok());
    for len in 0..f.len() - 1 {
        let _ = bgcode::read_header(&f[..len], &limits());
    }
    assert!(bgcode::read_header(&f[..f.len() - 1], &limits()).is_err());
}

#[test]
fn a_changed_byte_is_a_block_whose_checksum_is_not_ok_and_the_file_says_so() {
    let mut f = file(
        1,
        1,
        &[meta_block(0, "a=bcd\n", true), meta_block(3, "x=y\n", true)],
    );
    f[10 + 10] ^= 0x01;
    let h = bgcode::read_header(&f, &limits()).unwrap();
    assert_eq!(h.blocks[0].checksum_ok, Some(false));
    assert_eq!(h.blocks[1].checksum_ok, Some(true));
    assert!(!h.all_checksums_ok());
}

#[test]
fn a_thumbnail_is_listed_and_its_bytes_are_given_only_when_they_are_the_picture_it_says() {
    let f = file(
        1,
        0,
        &[
            thumb(0, 16, 8, &PNG8, 0, false),
            thumb(0, 32, 32, b"not a png", 0, false),
            thumb(0, 64, 64, &PNG8, 1, false),
            thumb(1, 4, 4, &[0xFF, 0xD8, 0xFF, 9], 0, false),
        ],
    );
    let h = bgcode::read_header(&f, &limits()).unwrap();
    assert_eq!(h.thumbnails.len(), 4);
    assert_eq!(h.thumbnail_data(&f, 0), Some(&PNG8[..]));
    assert_eq!(h.thumbnail_data(&f, 1), None);
    assert_eq!(h.thumbnail_data(&f, 2), None);
    assert_eq!(h.thumbnail_data(&f, 3), Some(&[0xFF, 0xD8, 0xFF, 9][..]));
    assert_eq!(h.thumbnail_data(&f, 4), None);
    assert!(h.thumbnails[2].packed && !h.thumbnails[0].packed);
    let (i, t) = h.largest_thumbnail().unwrap();
    assert_eq!((i, t.width, t.height), (2, 64, 64));
}

#[test]
fn a_thumbnail_with_a_picture_format_that_does_not_exist_is_unsupported() {
    let f = file(1, 0, &[thumb(7, 4, 4, b"x", 0, false)]);
    assert!(matches!(
        bgcode::read_header(&f, &limits()),
        Err(FormatError::Unsupported {
            format: "bgcode",
            ..
        })
    ));
}

#[test]
fn a_file_with_no_thumbnail_has_no_largest_one() {
    let f = file(1, 0, &[meta_block(0, "a=b\n", false)]);
    assert!(bgcode::read_header(&f, &limits())
        .unwrap()
        .largest_thumbnail()
        .is_none());
}

#[test]
fn only_so_many_lines_of_metadata_are_kept_and_each_is_cut_short() {
    let many: String = (0..600).map(|i| format!("k{i}=v{i}\n")).collect();
    let f = file(1, 0, &[meta_block(0, &many, false)]);
    let h = bgcode::read_header(&f, &limits()).unwrap();
    assert_eq!(h.metadata.len(), bgcode::MAX_METADATA_ENTRIES);
    let long = format!("k={}\n", "y".repeat(1000));
    let g = file(1, 0, &[meta_block(0, &long, false)]);
    let h = bgcode::read_header(&g, &limits()).unwrap();
    assert_eq!(h.value("k").unwrap().chars().count(), bgcode::MAX_TEXT);
}

#[test]
fn metadata_that_is_packed_or_has_no_equal_sign_is_not_read_as_text() {
    let packed = block(0, 1, &0u16.to_le_bytes(), b"a=b\n", 4, false);
    let h = bgcode::read_header(&file(1, 0, &[packed]), &limits()).unwrap();
    assert!(h.metadata.is_empty());
    let h = bgcode::read_header(
        &file(1, 0, &[meta_block(0, "no equal sign\n\n", false)]),
        &limits(),
    )
    .unwrap();
    assert!(h.metadata.is_empty());
}

#[test]
fn the_time_of_a_binary_file_is_read_from_its_metadata() {
    let f = file(
        1,
        0,
        &[meta_block(
            4,
            "estimated printing time (normal mode)=1h 2m 3s\nfilament used [g]=4.5\n",
            false,
        )],
    );
    let h = bgcode::read_header(&f, &limits()).unwrap();
    assert_eq!(h.estimated_seconds(), Some(3_723));
    assert_eq!(h.summary().filament_g, Some(4.5));
}

#[test]
fn a_file_larger_than_the_limit_is_refused_before_its_blocks_are_read() {
    let f = file(1, 0, &[meta_block(0, "a=b\n", false)]);
    let l = Limits {
        max_file_bytes: 5,
        ..limits()
    };
    assert!(matches!(
        bgcode::read_header(&f, &l),
        Err(FormatError::TooLarge { .. })
    ));
}

#[test]
fn a_duration_of_binary_g_code_is_made_of_parts_and_a_bare_count_is_not_one() {
    assert_eq!(bgcode::parse_duration("1d 2h 3m 4s"), Some(93_784));
    assert_eq!(bgcode::parse_duration("45s"), Some(45));
    assert_eq!(bgcode::parse_duration("90"), None);
    assert_eq!(bgcode::parse_duration(""), None);
    assert_eq!(bgcode::parse_duration("1h x"), None);
    assert_eq!(bgcode::parse_duration("18446744073709551615m"), None);
}

#[test]
fn bytes_that_start_like_binary_g_code_never_stop_the_reader() {
    let mut rng = Rng::new(0xB6C);
    for i in 0..400 {
        let n = rng.below(120) as usize;
        let mut f = file(1, (i % 2) as u16, &[]);
        f.extend_from_slice(&rng.bytes(n));
        let _ = bgcode::read_header(&f, &limits());
    }
}
