//! Behaviour of the core at its exact edges: the JSON reader, the settings reader, the limits, the lengths, the mesh checks, the temperature limits and the seeded generator.

use p3dp_core::error::CoreError;
use p3dp_core::json::{self, Value, MAX_BYTES, MAX_DEPTH};
use p3dp_core::kv;
use p3dp_core::machine::Zone;
use p3dp_core::mesh::{cube, Issue};
use p3dp_core::rng::Rng;
use p3dp_core::units::{MAX_MM, NM_PER_MM};
use p3dp_core::{Dialect, Limits, Mesh, Nm, Point3, Secret, TempLimits};

// ---- JSON ----

#[test]
fn json_text_of_exactly_the_largest_size_is_read_and_one_byte_more_is_refused() {
    let at_limit = format!("1{}", " ".repeat(MAX_BYTES - 1));
    assert_eq!(at_limit.len(), MAX_BYTES);
    assert_eq!(json::parse(&at_limit).unwrap(), Value::Num(1.0));
    let over = format!("1{}", " ".repeat(MAX_BYTES));
    assert!(matches!(json::parse(&over), Err(CoreError::Invalid(m)) if m.contains("bytes")));
}

#[test]
fn json_nesting_edge_is_decided_by_the_leaf() {
    let ok = "[".repeat(MAX_DEPTH) + "1" + &"]".repeat(MAX_DEPTH);
    assert!(json::parse(&ok).is_ok());
    let deep = "[".repeat(MAX_DEPTH + 1) + "1" + &"]".repeat(MAX_DEPTH + 1);
    let err = json::parse(&deep).unwrap_err().to_string();
    assert!(err.contains("nesting"), "{err}");
}

#[test]
fn json_objects_nest_as_deep_as_arrays() {
    let ok = "{\"a\":".repeat(MAX_DEPTH) + "1" + &"}".repeat(MAX_DEPTH);
    assert!(json::parse(&ok).is_ok());
    let deep = "{\"a\":".repeat(MAX_DEPTH + 1) + "1" + &"}".repeat(MAX_DEPTH + 1);
    assert!(json::parse(&deep).is_err());
}

#[test]
fn json_negative_zero_is_written_as_zero() {
    let v = json::parse("-0").unwrap();
    assert_eq!(v.to_json(), "0");
    assert_eq!(json::parse("-0.0").unwrap().as_f64(), Some(0.0));
}

#[test]
fn json_number_too_large_for_a_double_is_an_error() {
    let e = json::parse("1e400").unwrap_err().to_string();
    assert!(e.contains("too large"), "{e}");
    assert!(json::parse("-1e400").is_err());
    assert_eq!(json::parse("1e-400").unwrap().as_f64(), Some(0.0));
}

#[test]
fn json_numbers_round_trip_through_the_writer() {
    for n in [0.1, 1e15, -1e15, 1e300, 1e-7, 123_456.789, -0.5, 42.0] {
        let v = Value::Num(n);
        assert_eq!(json::parse(&v.to_json()).unwrap(), v, "{n}");
    }
}

#[test]
fn json_infinite_numbers_are_written_as_null() {
    assert_eq!(Value::Num(f64::INFINITY).to_json(), "null");
    assert_eq!(Value::Num(f64::NEG_INFINITY).to_json(), "null");
    let a = Value::Arr(vec![Value::Num(f64::NAN), Value::Num(1.0)]);
    assert_eq!(a.to_json(), "[null,1]");
}

#[test]
fn json_duplicate_keys_keep_their_order_and_the_first_wins() {
    let v = json::parse(r#"{"a":1,"b":2,"a":3}"#).unwrap();
    assert_eq!(v.get("a"), Some(&Value::Num(1.0)));
    match &v {
        Value::Obj(m) => {
            let keys: Vec<&str> = m.iter().map(|(k, _)| k.as_str()).collect();
            assert_eq!(keys, ["a", "b", "a"]);
        }
        other => panic!("not an object: {other:?}"),
    }
    assert_eq!(v.to_json(), r#"{"a":1,"b":2,"a":3}"#);
}

#[test]
fn json_whitespace_around_a_value_is_free_but_a_form_feed_is_not() {
    assert!(json::parse(" \t\r\n [1] \n ").is_ok());
    assert!(json::parse("\u{c}1").is_err());
    assert!(json::parse("1\u{a0}").is_err());
}

#[test]
fn json_accessors_of_the_wrong_kind_answer_none() {
    let n = Value::Num(1.0);
    assert!(n.get("a").is_none());
    assert!(n.at(0).is_none());
    assert!(n.as_str().is_none());
    assert!(n.as_bool().is_none());
    assert!(n.as_array().is_none());
    let s = Value::Str("x".to_string());
    assert!(s.as_f64().is_none());
    let a = Value::Arr(vec![Value::Null]);
    assert!(a.at(1).is_none());
    assert_eq!(a.at(0), Some(&Value::Null));
    assert!(a.get("0").is_none());
}

#[test]
fn json_an_empty_path_is_the_value_itself_and_an_empty_key_is_a_key() {
    let v = json::parse(r#"{"":7}"#).unwrap();
    assert_eq!(v.path(&[]), Some(&v));
    assert_eq!(v.get("").and_then(Value::as_f64), Some(7.0));
    assert!(v.path(&["", ""]).is_none());
}

#[test]
fn json_control_characters_are_escaped_and_come_back() {
    let s = "a\u{8}b\u{c}c\u{1f}d\u{7f}e";
    let text = Value::Str(s.to_string()).to_json();
    assert!(text.contains("\\u0008"));
    assert!(text.contains("\\u001f"));
    assert!(text.contains('\u{7f}'));
    assert_eq!(json::parse(&text).unwrap().as_str(), Some(s));
}

#[test]
fn json_lone_surrogates_are_errors() {
    assert!(json::parse(r#""\udc00""#).is_err());
    assert!(json::parse(r#""\ud800x""#).is_err());
    assert!(json::parse(r#""\ud800A""#).is_err());
    assert!(json::parse(r#""\ud800\ud800""#).is_err());
    assert!(json::parse(r#""\u12""#).is_err());
    assert!(json::parse(r#""\u12zz""#).is_err());
}

#[test]
fn json_every_prefix_of_a_document_is_an_error_and_never_a_panic() {
    let doc = r#"{"a":[1,2,{"b":"xé"}],"c":null}"#;
    assert!(json::parse(doc).is_ok());
    for n in 0..doc.len() {
        assert!(json::parse(&doc[..n]).is_err(), "prefix of {n} bytes");
    }
}

#[test]
fn json_text_after_the_value_is_named() {
    let e = json::parse("{} x").unwrap_err().to_string();
    assert!(e.contains("after the value"), "{e}");
    assert!(json::parse("[] []").is_err());
}

#[test]
fn json_literals_must_be_whole() {
    for bad in [
        "nul", "truee", "fals", "True", "NULL", "nan", "+1", ".5", "0x10",
    ] {
        assert!(json::parse(bad).is_err(), "{bad}");
    }
    assert_eq!(json::parse("true").unwrap().as_bool(), Some(true));
    assert_eq!(json::parse("false").unwrap().as_bool(), Some(false));
}

// ---- settings files ----

#[test]
fn kv_windows_line_endings_are_read_like_unix_ones() {
    let e = kv::parse("a=1\r\nb = 2\r\n").unwrap();
    assert_eq!(e.len(), 2);
    assert_eq!(e[0].value, "1");
    assert_eq!(e[1].value, "2");
    assert_eq!(e[1].line, 2);
}

#[test]
fn kv_the_value_keeps_every_equals_sign_after_the_first() {
    let e = kv::parse("k = a=b=c\n").unwrap();
    assert_eq!(e[0].key, "k");
    assert_eq!(e[0].value, "a=b=c");
}

#[test]
fn kv_a_comment_needs_a_space_before_the_hash() {
    let e = kv::parse("a = 3#x\nb = 4 # note\nc = # only a note\n").unwrap();
    assert_eq!(e[0].value, "3#x");
    assert_eq!(e[1].value, "4");
    assert_eq!(e[2].value, "");
}

#[test]
fn kv_line_numbers_count_blank_and_comment_lines() {
    let err = kv::parse("# one\n\n   # three\nbad line\n")
        .unwrap_err()
        .to_string();
    assert!(err.contains("line 4"), "{err}");
}

#[test]
fn kv_the_largest_file_is_read_and_one_byte_more_is_refused() {
    let at_limit = "#".repeat(MAX_BYTES_KV);
    assert!(kv::parse(&at_limit).unwrap().is_empty());
    let over = "#".repeat(MAX_BYTES_KV + 1);
    assert!(kv::parse(&over).is_err());
}

const MAX_BYTES_KV: usize = kv::MAX_BYTES;

#[test]
fn kv_an_empty_text_has_no_entries() {
    assert!(kv::parse("").unwrap().is_empty());
    assert!(kv::parse("\n\n# only comments\n").unwrap().is_empty());
}

#[test]
fn kv_whole_numbers_are_inclusive_at_both_ends() {
    assert_eq!(kv::int_in("k", "1", 1, 10).unwrap(), 1);
    assert_eq!(kv::int_in("k", "10", 1, 10).unwrap(), 10);
    assert!(kv::int_in("k", "0", 1, 10).is_err());
    assert!(kv::int_in("k", "11", 1, 10).is_err());
    assert_eq!(kv::int_in("k", " 7 ", 1, 10).unwrap(), 7);
    assert_eq!(kv::int_in("k", "-5", -10, 0).unwrap(), -5);
}

#[test]
fn kv_a_number_that_is_not_whole_or_is_too_big_is_an_error_that_names_the_key() {
    for bad in ["1.5", "", "1e3", "99999999999999999999", "0x10"] {
        let e = kv::int_in("limit_x", bad, 0, i64::MAX)
            .unwrap_err()
            .to_string();
        assert!(e.contains("limit_x"), "{bad:?}: {e}");
    }
}

#[test]
fn kv_decimals_are_inclusive_and_refuse_what_is_not_finite() {
    assert_eq!(kv::float_in("k", "0", 0.0, 1.0).unwrap(), 0.0);
    assert_eq!(kv::float_in("k", "1", 0.0, 1.0).unwrap(), 1.0);
    assert_eq!(kv::float_in("k", "1e3", 0.0, 2000.0).unwrap(), 1000.0);
    assert!(kv::float_in("k", "1.0000001", 0.0, 1.0).is_err());
    assert!(kv::float_in("k", "-0.0000001", 0.0, 1.0).is_err());
    assert!(kv::float_in("k", "-inf", -1.0, 1.0).is_err());
    assert!(kv::float_in("k", "abc", 0.0, 1.0).is_err());
}

#[test]
fn kv_on_off_accepts_every_spelling_and_nothing_else() {
    for yes in ["on", "ON", "True", "1", "yes", " on "] {
        assert!(kv::on_off("k", yes).unwrap(), "{yes:?}");
    }
    for no in ["off", "OFF", "False", "0", "No", " off "] {
        assert!(!kv::on_off("k", no).unwrap(), "{no:?}");
    }
    for bad in ["", "2", "onn", "y", "enabled"] {
        assert!(kv::on_off("k", bad).is_err(), "{bad:?}");
    }
}

// ---- limits ----

#[test]
fn limits_every_key_has_an_exact_maximum_and_one_more_is_refused() {
    let tops = [
        ("limit_max_file_mib", "4096", "4097"),
        ("limit_max_triangles", "50000000", "50000001"),
        ("limit_max_vertices", "150000000", "150000001"),
        ("limit_max_extent_mm", "100000", "100000.5"),
        ("limit_max_zip_entries", "100000", "100001"),
        ("limit_max_inflated_mib", "8192", "8193"),
        ("limit_max_inflate_ratio", "100000", "100001"),
    ];
    for (k, top, over) in tops {
        let mut l = Limits::default();
        assert_eq!(l.set(k, top), Ok(true), "{k}");
        let after = l.clone();
        assert!(l.set(k, over).is_err(), "{k}");
        assert_eq!(l, after, "{k} changed after a refused value");
    }
}

#[test]
fn limits_every_key_has_an_exact_minimum_and_one_less_is_refused() {
    let bottoms = [
        ("limit_max_file_mib", "1", "0"),
        ("limit_max_triangles", "1", "0"),
        ("limit_max_vertices", "1", "0"),
        ("limit_max_extent_mm", "1", "0.5"),
        ("limit_max_zip_entries", "1", "0"),
        ("limit_max_inflated_mib", "1", "0"),
        ("limit_max_inflate_ratio", "2", "1"),
    ];
    for (k, low, under) in bottoms {
        let mut l = Limits::default();
        assert_eq!(l.set(k, low), Ok(true), "{k}");
        assert!(l.set(k, under).is_err(), "{k}");
    }
}

#[test]
fn limits_megabyte_keys_are_stored_in_bytes() {
    let mut l = Limits::default();
    l.set("limit_max_file_mib", "4096").unwrap();
    assert_eq!(l.max_file_bytes, 4096u64 << 20);
    l.set("limit_max_inflated_mib", "3").unwrap();
    assert_eq!(l.max_inflated_bytes, 3 * 1024 * 1024);
}

#[test]
fn limits_the_last_line_of_a_file_wins_and_the_text_round_trips() {
    let l = Limits::from_text("limit_max_triangles = 10\nlimit_max_triangles = 20\n").unwrap();
    assert_eq!(l.max_triangles, 20);
    assert_eq!(Limits::from_text(&l.to_text()).unwrap(), l);
}

#[test]
fn limits_the_text_lists_the_keys_in_their_order_with_a_line_each() {
    let text = Limits::default().to_text();
    let keys: Vec<&str> = text
        .lines()
        .map(|l| l.split(" = ").next().unwrap_or(""))
        .collect();
    assert_eq!(keys, Limits::KEYS);
    assert!(text.ends_with('\n'));
}

#[test]
fn limits_a_fractional_extent_round_trips() {
    let l = Limits::from_text("limit_max_extent_mm = 12.5\n").unwrap();
    assert_eq!(l.max_extent_mm, 12.5);
    assert!(l.to_text().contains("limit_max_extent_mm = 12.5\n"));
}

#[test]
fn limits_a_line_with_no_equals_sign_is_an_error() {
    assert!(Limits::from_text("limit_max_triangles 5\n").is_err());
    assert_eq!(Limits::from_text("").unwrap(), Limits::default());
}

// ---- lengths ----

#[test]
fn nm_the_largest_length_is_accepted_and_a_hair_more_is_not() {
    assert_eq!(Nm::from_mm(MAX_MM).unwrap(), Nm(1_000_000 * NM_PER_MM));
    assert_eq!(Nm::from_mm(-MAX_MM).unwrap(), Nm(-1_000_000 * NM_PER_MM));
    assert!(Nm::from_mm(MAX_MM + 0.001).is_err());
    assert!(Nm::from_mm(-MAX_MM - 0.001).is_err());
    assert!(Nm::from_mm(f64::NEG_INFINITY).is_err());
}

#[test]
fn nm_decimal_text_for_the_smallest_steps() {
    assert_eq!(Nm(1).to_decimal_mm(), "0.000001");
    assert_eq!(Nm(-1_000_000).to_decimal_mm(), "-1");
    assert_eq!(Nm(1_000_001).to_decimal_mm(), "1.000001");
    assert_eq!(Nm(100_000).to_decimal_mm(), "0.1");
    assert_eq!(Nm(10_000_000).to_decimal_mm(), "10");
    assert_eq!(Nm(i64::MIN).to_decimal_mm(), "-9223372036854.775808");
    assert_eq!(Nm(i64::MAX).to_decimal_mm(), "9223372036854.775807");
}

#[test]
fn nm_the_decimal_text_reads_back_to_the_same_length() {
    let mut r = Rng::new(31_415);
    for _ in 0..300 {
        let n = Nm(r.below(2_000_000_000) as i64 - 1_000_000_000);
        let back = Nm::from_mm(n.to_decimal_mm().parse::<f64>().unwrap()).unwrap();
        assert_eq!(back, n);
    }
}

#[test]
fn nm_display_adds_the_unit() {
    assert_eq!(Nm(1_500_000).to_string(), "1.5 mm");
    assert_eq!(Nm(0).to_string(), "0 mm");
    assert_eq!(Nm(-250_000).to_string(), "-0.25 mm");
}

#[test]
fn nm_micrometres_round_away_from_zero_at_the_half() {
    assert_eq!(Nm(1499).to_um(), 1);
    assert_eq!(Nm(1500).to_um(), 2);
    assert_eq!(Nm(-1499).to_um(), -1);
    assert_eq!(Nm(-1500).to_um(), -2);
    assert_eq!(Nm(0).to_um(), 0);
}

#[test]
fn nm_the_most_negative_length_has_an_absolute_value_that_does_not_panic() {
    assert_eq!(Nm(i64::MIN).abs(), Nm(i64::MAX));
    assert_eq!(-Nm(i64::MIN), Nm(i64::MAX));
    assert_eq!(Nm(5) - Nm(7), Nm(-2));
    assert_eq!(Nm::default(), Nm(0));
}

#[test]
fn nm_lengths_sort_by_value() {
    let mut v = vec![Nm(5), Nm(-3), Nm(0), Nm(2_000_000), Nm(-3)];
    v.sort();
    assert_eq!(v, [Nm(-3), Nm(-3), Nm(0), Nm(5), Nm(2_000_000)]);
}

// ---- the mesh ----

#[test]
fn mesh_a_cube_has_the_numbers_of_a_cube() {
    let m = cube(10.0);
    assert_eq!(m.vertices.len(), 8);
    assert_eq!(m.triangles.len(), 12);
    assert!((m.volume_mm3() - 1000.0).abs() < 1e-6);
    assert!((m.surface_area_mm2() - 600.0).abs() < 1e-6);
    assert_eq!(m.size_mm(), Some([10.0, 10.0, 10.0]));
    assert_eq!(m.open_edges(), 0);
    assert_eq!(m.degenerate_count(), 0);
    assert!(m.validate(&Limits::default()).issues.is_empty());
}

#[test]
fn mesh_a_cube_of_no_size_or_of_a_size_that_is_not_a_length_is_handled() {
    let flat = cube(0.0);
    assert_eq!(flat.vertices.len(), 1);
    assert_eq!(flat.degenerate_count(), 12);
    assert!(flat.validate(&Limits::default()).is_ok());
    assert!(cube(f64::NAN).triangles.is_empty());
    assert!(cube(1.0e9).triangles.is_empty());
}

#[test]
fn mesh_an_empty_mesh_answers_without_panicking() {
    let m = Mesh::new();
    assert!(m.bounds().is_none());
    assert!(m.size_mm().is_none());
    assert_eq!(m.volume_mm3(), 0.0);
    assert_eq!(m.surface_area_mm2(), 0.0);
    assert_eq!(m.open_edges(), 0);
    assert!(m.fits_in([0.0, 0.0, 0.0]));
    assert_eq!(m.at_origin(), m);
    let r = m.validate(&Limits::default());
    assert_eq!(r.issues, [Issue::Empty]);
    assert!(!r.is_ok());
    assert_eq!(r.errors().count(), 1);
    assert_eq!(r.warnings().count(), 0);
}

#[test]
fn mesh_moving_to_the_origin_is_idempotent_and_keeps_the_shape() {
    let mut soup = Vec::new();
    for tri in cube(4.0).triangles {
        let mut t = [[0.0; 3]; 3];
        for (k, i) in tri.iter().enumerate() {
            let p = cube(4.0).vertices[*i as usize].mm();
            t[k] = [p[0] + 100.0, p[1] - 50.0, p[2] + 7.5];
        }
        soup.push(t);
    }
    let m = Mesh::from_soup(&soup, &Limits::default()).unwrap();
    let once = m.at_origin();
    assert_eq!(once.at_origin(), once);
    let (lo, _) = once.bounds().unwrap();
    assert_eq!(lo, Point3::default());
    assert_eq!(once.size_mm(), m.size_mm());
    assert!((once.volume_mm3() - m.volume_mm3()).abs() < 1e-6);
}

#[test]
fn mesh_the_triangle_limit_is_exact() {
    let soup = [
        [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        [[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
    ];
    let two = Limits {
        max_triangles: 2,
        ..Limits::default()
    };
    assert_eq!(Mesh::from_soup(&soup, &two).unwrap().triangles.len(), 2);
    let one = Limits {
        max_triangles: 1,
        ..Limits::default()
    };
    assert_eq!(
        Mesh::from_soup(&soup, &one),
        Err(CoreError::TooMany {
            what: "triangles",
            count: 2,
            max: 1
        })
    );
}

#[test]
fn mesh_the_vertex_limit_counts_welded_corners_once() {
    let soup = [
        [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        [[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
    ];
    let four = Limits {
        max_vertices: 4,
        ..Limits::default()
    };
    let m = Mesh::from_soup(&soup, &four).unwrap();
    assert_eq!(m.vertices.len(), 4);
    assert_eq!(m.triangles, [[0, 1, 2], [1, 3, 2]]);
    let three = Limits {
        max_vertices: 3,
        ..Limits::default()
    };
    assert!(matches!(
        Mesh::from_soup(&soup, &three),
        Err(CoreError::TooMany {
            what: "vertices",
            max: 3,
            ..
        })
    ));
}

#[test]
fn mesh_a_corner_that_is_not_finite_or_not_a_length_is_refused() {
    let nan = [[[f64::NAN, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]];
    assert!(Mesh::from_soup(&nan, &Limits::default()).is_err());
    let far = [[[2.0e6, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]];
    assert!(Mesh::from_soup(&far, &Limits::default()).is_err());
    assert!(Mesh::from_soup(&[], &Limits::default())
        .unwrap()
        .triangles
        .is_empty());
}

#[test]
fn mesh_a_single_triangle_has_three_open_edges_and_no_inverted_warning() {
    let soup = [[[0.0, 0.0, 0.0], [3.0, 0.0, 0.0], [0.0, 4.0, 0.0]]];
    let m = Mesh::from_soup(&soup, &Limits::default()).unwrap();
    assert!((m.surface_area_mm2() - 6.0).abs() < 1e-9);
    assert_eq!(m.volume_mm3(), 0.0);
    assert_eq!(m.open_edges(), 3);
    let r = m.validate(&Limits::default());
    assert_eq!(r.issues, [Issue::OpenEdges(3)]);
    assert!(r.is_ok());
    assert_eq!(r.warnings().count(), 1);
}

#[test]
fn mesh_reversed_faces_are_an_inverted_warning_with_the_volume_negated() {
    let mut m = cube(10.0);
    for t in &mut m.triangles {
        t.swap(1, 2);
    }
    assert!((m.volume_mm3() + 1000.0).abs() < 1e-6);
    assert_eq!(m.open_edges(), 0);
    let r = m.validate(&Limits::default());
    assert_eq!(r.issues, [Issue::Inverted]);
    assert!(r.is_ok());
}

#[test]
fn mesh_the_extent_limit_is_exclusive_at_the_exact_size() {
    let m = cube(10.0);
    let exact = Limits {
        max_extent_mm: 10.0,
        ..Limits::default()
    };
    assert!(m.validate(&exact).issues.is_empty());
    let small = Limits {
        max_extent_mm: 9.0,
        ..Limits::default()
    };
    let r = m.validate(&small);
    let axes: Vec<char> = r
        .errors()
        .filter_map(|i| match i {
            Issue::TooLarge { axis, .. } => Some(*axis),
            _ => None,
        })
        .collect();
    assert_eq!(axes, ['x', 'y', 'z']);
    assert!(!r.is_ok());
}

#[test]
fn mesh_a_bad_index_stops_the_checks_at_the_first_triangle_that_has_one() {
    let m = Mesh {
        vertices: vec![Point3::default()],
        triangles: vec![[0, 0, 0], [0, 0, 5], [0, 9, 0]],
    };
    let r = m.validate(&Limits::default());
    assert_eq!(
        r.issues,
        [Issue::BadIndex {
            triangle: 1,
            index: 5
        }]
    );
    assert!(!r.is_ok());
    assert_eq!(m.volume_mm3(), 0.0);
    assert_eq!(m.surface_area_mm2(), 0.0);
    let text = r.issues[0].to_string();
    assert!(
        text.contains("triangle 1") && text.contains("vertex 5"),
        "{text}"
    );
}

#[test]
fn mesh_which_issues_are_errors_and_which_are_warnings() {
    let errors = [
        Issue::Empty,
        Issue::BadIndex {
            triangle: 0,
            index: 0,
        },
        Issue::TooManyTriangles { count: 2, max: 1 },
        Issue::TooManyVertices { count: 2, max: 1 },
        Issue::TooLarge {
            axis: 'x',
            mm: 2.0,
            max: 1.0,
        },
    ];
    for i in &errors {
        assert!(i.is_error(), "{i}");
        assert!(!i.to_string().is_empty());
    }
    for i in [Issue::Degenerate(1), Issue::OpenEdges(1), Issue::Inverted] {
        assert!(!i.is_error(), "{i}");
    }
}

#[test]
fn mesh_too_many_triangles_is_reported_with_the_numbers() {
    let m = cube(1.0);
    let l = Limits {
        max_triangles: 11,
        ..Limits::default()
    };
    let r = m.validate(&l);
    assert!(r
        .issues
        .contains(&Issue::TooManyTriangles { count: 12, max: 11 }));
    let l = Limits {
        max_vertices: 7,
        ..Limits::default()
    };
    assert!(m
        .validate(&l)
        .issues
        .contains(&Issue::TooManyVertices { count: 8, max: 7 }));
}

#[test]
fn mesh_fits_in_is_inclusive_on_every_axis() {
    let m = cube(10.0);
    assert!(m.fits_in([10.0, 10.0, 10.0]));
    assert!(!m.fits_in([10.0, 10.0, 9.999]));
    assert!(!m.fits_in([9.999, 10.0, 10.0]));
    assert!(!m.fits_in([10.0, 9.999, 10.0]));
}

#[test]
fn mesh_scaling_a_cube_scales_volume_by_the_cube_and_area_by_the_square() {
    for s in [1.0, 2.5, 7.0, 40.0] {
        let m = cube(s);
        assert!((m.volume_mm3() - s * s * s).abs() < 1e-6 * s * s * s, "{s}");
        assert!(
            (m.surface_area_mm2() - 6.0 * s * s).abs() < 1e-6 * s * s,
            "{s}"
        );
    }
}

#[test]
fn point_from_mm_reads_back_and_refuses_what_is_not_a_length() {
    let p = Point3::from_mm(1.5, -2.25, 0.0).unwrap();
    assert_eq!(p.mm(), [1.5, -2.25, 0.0]);
    assert!(Point3::from_mm(0.0, f64::NAN, 0.0).is_err());
    assert!(Point3::from_mm(0.0, 0.0, 5.0e6).is_err());
}

// ---- temperatures and dialects ----

const LIMITS: TempLimits = TempLimits {
    nozzle_c: 300.0,
    bed_c: 100.0,
    chamber_c: Some(60.0),
};

#[test]
fn temp_the_limit_itself_is_allowed_and_a_hair_over_is_not() {
    assert_eq!(LIMITS.check(Zone::Nozzle, 300.0), Ok(300.0));
    assert!(LIMITS.check(Zone::Nozzle, 300.000_001).is_err());
    assert_eq!(LIMITS.check(Zone::Bed, 100.0), Ok(100.0));
    assert!(LIMITS.check(Zone::Bed, 100.1).is_err());
    assert_eq!(LIMITS.check(Zone::Chamber, 60.0), Ok(60.0));
    assert!(LIMITS.check(Zone::Chamber, 60.1).is_err());
}

#[test]
fn temp_a_value_that_is_not_a_number_is_refused_before_anything_else() {
    for z in [Zone::Nozzle, Zone::Bed, Zone::Chamber] {
        for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let e = LIMITS.check(z, v).unwrap_err().to_string();
            assert!(e.contains(z.name()) && e.contains("finite"), "{e}");
        }
        assert!(LIMITS.check(z, -0.5).is_err());
        assert_eq!(LIMITS.check(z, 0.0), Ok(0.0));
    }
}

#[test]
fn temp_a_printer_with_no_chamber_accepts_off_and_refuses_any_heat() {
    let l = TempLimits {
        chamber_c: None,
        ..LIMITS
    };
    assert_eq!(l.check(Zone::Chamber, 0.0), Ok(0.0));
    let e = l.check(Zone::Chamber, 0.5).unwrap_err().to_string();
    assert!(e.contains("no heated chamber"), "{e}");
}

#[test]
fn temp_the_message_names_the_value_the_limit_and_the_zone() {
    let e = LIMITS.check(Zone::Nozzle, 350.0).unwrap_err().to_string();
    assert!(
        e.contains("nozzle") && e.contains("350") && e.contains("300"),
        "{e}"
    );
    let e = LIMITS.check(Zone::Bed, -5.0).unwrap_err().to_string();
    assert!(e.contains("bed") && e.contains("below zero"), "{e}");
}

#[test]
fn dialect_names_are_read_without_regard_to_case_or_spaces() {
    assert_eq!(Dialect::from_name("  KLIPPER "), Some(Dialect::Klipper));
    assert_eq!(Dialect::from_name("RepRapFirmware"), Some(Dialect::Rrf));
    assert_eq!(Dialect::from_name("Marlin"), Some(Dialect::Marlin));
    assert_eq!(Dialect::from_name(""), None);
    assert_eq!(Dialect::from_name("marlin2"), None);
    assert_eq!(Dialect::from_name("grbl"), None);
}

#[test]
fn dialect_all_lists_each_dialect_once_with_a_name_that_reads_back() {
    let mut names: Vec<&str> = Dialect::ALL.iter().map(|d| d.name()).collect();
    for d in Dialect::ALL {
        assert_eq!(Dialect::from_name(&d.to_string()), Some(d));
    }
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), Dialect::ALL.len());
}

#[test]
fn zone_names_are_the_words_of_the_messages() {
    assert_eq!(Zone::Nozzle.name(), "nozzle");
    assert_eq!(Zone::Bed.name(), "bed");
    assert_eq!(Zone::Chamber.name(), "chamber");
}

// ---- secret and generator ----

#[test]
fn secret_hides_in_every_format_and_inside_containers() {
    let s = Secret::new("p4ss-w0rd");
    let all = format!(
        "{s} {s:?} {s:#?} {:?} {:?}",
        vec![s.clone()],
        Some(s.clone())
    );
    assert!(!all.contains("p4ss"), "{all}");
    assert_eq!(s.to_string(), "***");
    assert_eq!(format!("{s:?}"), "Secret(***)");
}

#[test]
fn secret_compares_by_content_and_keeps_every_character() {
    assert_eq!(Secret::new("a"), Secret::new("a"));
    assert_ne!(Secret::new("a"), Secret::new("b"));
    let odd = Secret::new("  \u{e9}\u{4e2d}\n ");
    assert_eq!(odd.expose(), "  \u{e9}\u{4e2d}\n ");
    assert!(Secret::new("").is_empty());
    assert!(!Secret::new(" ").is_empty());
}

#[test]
fn rng_a_zero_seed_is_the_replacement_seed() {
    let mut a = Rng::new(0);
    let mut b = Rng::new(0x9E37_79B9_7F4A_7C15);
    for _ in 0..10 {
        assert_eq!(a.next_u64(), b.next_u64());
    }
}

#[test]
fn rng_below_and_unit_stay_inside_their_ranges() {
    let mut r = Rng::new(2_718);
    assert_eq!(r.below(0), 0);
    assert_eq!(r.below(1), 0);
    for _ in 0..1000 {
        assert!(r.below(7) < 7);
        let u = r.unit();
        assert!((0.0..1.0).contains(&u));
        let x = r.range(-3.0, 5.0);
        assert!((-3.0..5.0).contains(&x));
    }
    assert_eq!(r.range(2.0, 2.0), 2.0);
}

#[test]
fn rng_bytes_have_the_length_asked_and_a_clone_goes_on_the_same() {
    let mut r = Rng::new(9);
    assert!(r.bytes(0).is_empty());
    assert_eq!(r.bytes(33).len(), 33);
    let mut c = r.clone();
    assert_eq!(r.bytes(16), c.bytes(16));
    assert_eq!(r.next_u64(), c.next_u64());
}

#[test]
fn rng_different_seeds_give_different_streams() {
    let a: Vec<u64> = {
        let mut r = Rng::new(1);
        (0..8).map(|_| r.next_u64()).collect()
    };
    let b: Vec<u64> = {
        let mut r = Rng::new(2);
        (0..8).map(|_| r.next_u64()).collect()
    };
    assert_ne!(a, b);
}
