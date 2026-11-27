//! Edge cases of the profiles: the catalog of printers and how it is read, the checks of one profile, the file an owner writes and edits, what a printer says about several
//! materials, the filaments and the resins with their checks against a printer. A profile is written to a file and read back; a value that is out of its range, a name that could
//! begin another line of the file and a limit that is raised are all given to the code, which refuses or tells.

use p3dp_core::mesh::cube;
use p3dp_core::{Dialect, TempLimits};
use p3dp_profiles::filament::{Advice, Issue, Range};
use p3dp_profiles::{
    caps_of, Catalog, Connectivity, FilamentCatalog, FilamentProfile, LimitSource, Material,
    MultiKind, Notice, Nozzle, PrinterProfile, ProfileError, ResinCatalog, ResinKind, ResinProfile,
    Technology, Wash,
};

fn catalog() -> Catalog {
    Catalog::builtin().unwrap()
}

fn mk4s() -> PrinterProfile {
    catalog().find("prusa-mk4s").unwrap().clone()
}

fn resin_printer() -> PrinterProfile {
    catalog().find("elegoo-mars-4").unwrap().clone()
}

// ---------------------------------------------------------------- the names

#[test]
fn every_way_of_connecting_is_known_by_its_name_in_any_letter_case() {
    let mut seen = std::collections::HashSet::new();
    for c in Connectivity::ALL {
        assert!(seen.insert(c.name()), "{} twice", c.name());
        assert_eq!(Connectivity::from_name(c.name()), Some(c));
        assert_eq!(
            Connectivity::from_name(&c.name().to_ascii_uppercase()),
            Some(c)
        );
        assert_eq!(
            Connectivity::from_name(&format!("  {} ", c.name())),
            Some(c)
        );
    }
    assert_eq!(seen.len(), 9);
}

#[test]
fn a_way_of_connecting_that_is_not_known_is_none() {
    for bad in ["", "wifi", "moon raker", "sdcard", "bambu_lan"] {
        assert_eq!(Connectivity::from_name(bad), None, "{bad:?}");
    }
    assert_eq!(Connectivity::from_name("sd"), Some(Connectivity::SdCard));
}

#[test]
fn the_technologies_and_the_sources_of_the_limits_are_known_by_their_names() {
    for t in [Technology::Fdm, Technology::Msla] {
        assert_eq!(Technology::from_name(t.name()), Some(t));
    }
    assert_eq!(Technology::from_name(" FDM "), Some(Technology::Fdm));
    assert_eq!(Technology::from_name("sla"), None);
    for s in [
        LimitSource::Maker,
        LimitSource::Conservative,
        LimitSource::Owner,
    ] {
        assert_eq!(LimitSource::from_name(s.name()), Some(s));
    }
    assert_eq!(
        LimitSource::from_name("conservative"),
        Some(LimitSource::Conservative)
    );
    assert_eq!(LimitSource::Conservative.name(), "default");
    assert_eq!(LimitSource::from_name("someone"), None);
}

// ---------------------------------------------------------------- the catalog

#[test]
fn the_built_in_catalog_has_printers_whose_profiles_are_all_valid() {
    let c = catalog();
    assert!(!c.is_empty());
    assert_eq!(c.len(), c.profiles.len());
    for p in &c.profiles {
        p.validate().unwrap_or_else(|e| panic!("{}: {e}", p.id));
    }
}

#[test]
fn the_ids_of_the_catalog_are_different_and_in_lower_case() {
    let c = catalog();
    let mut seen = std::collections::HashSet::new();
    for p in &c.profiles {
        assert!(seen.insert(p.id.as_str()), "{} twice", p.id);
        assert_eq!(p.id, p.id.to_ascii_lowercase());
    }
}

#[test]
fn a_printer_is_found_by_its_id_in_any_case_and_with_blanks() {
    let c = catalog();
    let p = c.find("prusa-mk4s").unwrap();
    assert_eq!(c.find("PRUSA-MK4S").unwrap().id, p.id);
    assert_eq!(c.find("  prusa-mk4s\n").unwrap().id, p.id);
    assert!(c.find("").is_none());
    assert!(c.find("prusa").is_none());
}

#[test]
fn a_search_with_nothing_gives_every_printer_and_one_with_a_word_gives_the_ones_that_have_it() {
    let c = catalog();
    assert_eq!(c.search("").len(), c.len());
    assert_eq!(c.search("   ").len(), c.len());
    let prusa = c.search("PRUSA");
    assert!(!prusa.is_empty());
    for p in prusa {
        let all = format!("{} {} {}", p.id, p.vendor, p.model).to_ascii_lowercase();
        assert!(all.contains("prusa"), "{all}");
    }
    assert!(c.search("zzz-no-such-printer").is_empty());
}

#[test]
fn the_vendors_are_listed_once_each_and_the_technologies_make_up_the_catalog() {
    let c = catalog();
    let vendors = c.vendors();
    let set: std::collections::HashSet<&str> = vendors.iter().copied().collect();
    assert_eq!(vendors.len(), set.len());
    assert!(vendors.iter().all(|v| !v.is_empty()));
    assert_eq!(
        c.technology(Technology::Fdm).len() + c.technology(Technology::Msla).len(),
        c.len()
    );
    assert!(c
        .technology(Technology::Msla)
        .iter()
        .all(|p| p.dialect == Dialect::Msla));
}

#[test]
fn the_printers_that_can_be_reached_a_way_all_have_it() {
    let c = catalog();
    for way in Connectivity::ALL {
        for p in c.reachable_by(way) {
            assert!(p.has(way));
        }
    }
    assert!(!c.reachable_by(Connectivity::Moonraker).is_empty());
}

const FDM_ROW: &str =
    "acme-ace|Acme|Ace 1|fdm|200|200|200|0.4|marlin|serial||260|100|-|false|maker";
const MSLA_ROW: &str = "acme-resin|Acme|Resin 1|msla|100|60|150||msla|sd||||-|false|default";

#[test]
fn a_catalog_of_two_rows_is_read_with_comments_and_blank_lines() {
    let text = format!("# a comment\n\n{FDM_ROW}\n   \n{MSLA_ROW}\n");
    let c = Catalog::parse(&text).unwrap();
    assert_eq!(c.len(), 2);
    let a = c.find("acme-ace").unwrap();
    assert_eq!(
        (a.volume_mm, a.nozzles_mm.clone(), a.dialect),
        ([200.0; 3], vec![0.4], Dialect::Marlin)
    );
    assert!(!a.verified && a.multimaterial.is_none());
    assert_eq!(a.limit_source, LimitSource::Maker);
    let m = c.find("acme-resin").unwrap();
    assert_eq!(m.technology, Technology::Msla);
    assert!(m.limits.is_none());
}

#[test]
fn a_catalog_with_a_row_that_is_wrong_is_refused_with_its_line() {
    for (row, line) in [
        ("a|b|c", "line 1: 3 columns, expected 16"),
        (
            FDM_ROW.replace("|fdm|", "|sla|").as_str(),
            "line 1: technology",
        ),
        (
            FDM_ROW.replace("|marlin|", "|gcode|").as_str(),
            "line 1: dialect",
        ),
        (
            FDM_ROW.replace("|serial|", "|wifi|").as_str(),
            "line 1: connectivity",
        ),
        (
            FDM_ROW.replace("|false|", "|maybe|").as_str(),
            "line 1: verified",
        ),
        (
            FDM_ROW.replace("|maker", "|owner").as_str(),
            "line 1: the source",
        ),
        (
            FDM_ROW.replace("|260|", "|hot|").as_str(),
            "line 1: nozzle limit",
        ),
        (
            FDM_ROW.replace("|200|200|200|", "|200|x|200|").as_str(),
            "line 1: y",
        ),
    ] {
        let e = Catalog::parse(row).unwrap_err();
        assert!(e.to_string().starts_with(line), "{row}: {e}");
    }
}

#[test]
fn a_catalog_with_nothing_or_with_a_repeated_id_is_refused() {
    assert!(Catalog::parse("").is_err());
    assert!(Catalog::parse("# only a comment\n").is_err());
    let twice = format!("{FDM_ROW}\n{FDM_ROW}\n");
    let e = Catalog::parse(&twice).unwrap_err();
    assert!(e.to_string().contains("used twice"), "{e}");
}

#[test]
fn a_row_whose_profile_does_not_pass_its_own_checks_is_refused_with_its_line() {
    let e = Catalog::parse(&FDM_ROW.replace("|200|200|200|", "|0|200|200|")).unwrap_err();
    assert!(e.to_string().starts_with("line 1: profile acme-ace"), "{e}");
    let e = Catalog::parse(&MSLA_ROW.replace("|msla|sd|", "|marlin|sd|")).unwrap_err();
    assert!(e.to_string().contains("msla dialect"), "{e}");
    let upper = FDM_ROW.replace("acme-ace", "ACME-ace");
    assert!(Catalog::parse(&upper).is_err());
}

// ---------------------------------------------------------------- one profile

#[test]
fn the_profile_of_a_catalog_printer_passes_and_a_change_of_one_value_does_not() {
    assert!(mk4s().validate().is_ok());
    invalid("id", |p| p.id = "Upper".into());
    invalid("empty id", |p| p.id = String::new());
    invalid("space in id", |p| p.id = "a b".into());
    invalid("vendor", |p| p.vendor = String::new());
    invalid("model", |p| p.model = String::new());
    invalid("volume 0", |p| p.volume_mm[1] = 0.0);
    invalid("volume negative", |p| p.volume_mm[0] = -1.0);
    invalid("volume nan", |p| p.volume_mm[2] = f64::NAN);
    invalid("volume inf", |p| p.volume_mm[2] = f64::INFINITY);
    invalid("volume big", |p| p.volume_mm[0] = 5000.5);
    invalid("nozzle small", |p| p.nozzles_mm = vec![0.05]);
    invalid("nozzle big", |p| p.nozzles_mm = vec![0.4, 2.5]);
    invalid("nozzle nan", |p| p.nozzles_mm = vec![f64::NAN]);
    invalid("no nozzle", |p| p.nozzles_mm = Vec::new());
    invalid("no limits", |p| p.limits = None);
    invalid("no way", |p| p.connectivity = Vec::new());
    invalid("msla dialect on fdm", |p| p.dialect = Dialect::Msla);
}

fn invalid(what: &str, change: impl FnOnce(&mut PrinterProfile)) {
    let mut p = mk4s();
    change(&mut p);
    assert!(p.validate().is_err(), "{what}");
}

#[test]
fn the_limits_of_the_heaters_have_their_edges() {
    let mut p = mk4s();
    for (nozzle, bed, chamber, ok) in [
        (600.0, 200.0, Some(200.0), true),
        (0.0, 100.0, None, false),
        (600.5, 100.0, None, false),
        (f64::NAN, 100.0, None, false),
        (300.0, 0.0, None, false),
        (300.0, 200.5, None, false),
        (300.0, f64::INFINITY, None, false),
        (300.0, 100.0, Some(0.0), false),
        (300.0, 100.0, Some(200.5), false),
        (300.0, 100.0, Some(f64::NAN), false),
    ] {
        p.limits = Some(TempLimits {
            nozzle_c: nozzle,
            bed_c: bed,
            chamber_c: chamber,
        });
        assert_eq!(p.validate().is_ok(), ok, "{nozzle} {bed} {chamber:?}");
    }
}

#[test]
fn the_volume_and_the_nozzles_are_valid_at_their_edges() {
    let mut p = mk4s();
    p.volume_mm = [5000.0, 0.001, 1.0];
    p.nozzles_mm = vec![0.1, 2.0];
    assert!(p.validate().is_ok());
}

#[test]
fn a_resin_printer_has_the_msla_dialect_and_no_nozzle_and_no_heater() {
    let base = resin_printer();
    assert!(base.validate().is_ok());
    let mut with_limits = base.clone();
    with_limits.limits = mk4s().limits;
    assert!(with_limits.validate().is_err());
    let mut with_nozzle = base.clone();
    with_nozzle.nozzles_mm = vec![0.4];
    assert!(with_nozzle.validate().is_err());
    let mut marlin = base;
    marlin.dialect = Dialect::Marlin;
    assert!(marlin.validate().is_err());
}

#[test]
fn a_name_that_would_begin_another_line_of_the_file_is_refused() {
    for bad in [
        "A\nverified = on",
        "A\r\nB",
        "A\tB",
        "A\0B",
        "A #comment",
        "x\u{7f}y",
    ] {
        let mut p = mk4s();
        p.model = bad.to_string();
        assert!(p.validate().is_err(), "model {bad:?}");
        let mut q = mk4s();
        q.vendor = bad.to_string();
        assert!(q.validate().is_err(), "vendor {bad:?}");
    }
    let mut ok = mk4s();
    ok.model = "MK4S#2 (a=b) é".to_string();
    assert!(ok.validate().is_ok());
}

#[test]
fn the_default_nozzle_is_04_when_it_is_offered_and_else_the_first() {
    let mut p = mk4s();
    p.nozzles_mm = vec![0.25, 0.4, 0.6];
    assert_eq!(p.default_nozzle_mm(), Some(0.4));
    p.nozzles_mm = vec![0.6, 0.25];
    assert_eq!(p.default_nozzle_mm(), Some(0.6));
    p.nozzles_mm = Vec::new();
    assert_eq!(p.default_nozzle_mm(), None);
}

#[test]
fn a_model_fits_when_its_box_is_inside_the_volume_on_every_axis() {
    let p = mk4s();
    assert!(p.fits(&cube(100.0)));
    assert!(p.fits(&cube(210.0)));
    assert!(!p.fits(&cube(221.0)));
    assert!(!p.fits(&cube(300.0)));
    assert!(p.fits(&p3dp_core::Mesh::new()));
}

#[test]
fn the_title_does_not_say_the_vendor_twice() {
    let mut p = mk4s();
    p.vendor = "Prusa".into();
    p.model = "MK4S".into();
    assert_eq!(p.title(), "Prusa MK4S");
    p.model = "Prusa MK4S".into();
    assert_eq!(p.title(), "Prusa MK4S");
}

#[test]
fn a_printer_that_has_a_way_of_connecting_says_so() {
    let mut p = mk4s();
    p.connectivity = vec![Connectivity::Duet];
    assert!(p.has(Connectivity::Duet));
    assert!(!p.has(Connectivity::Serial));
}

// ---------------------------------------------------------------- the file of a profile

#[test]
fn every_profile_of_the_catalog_is_written_and_read_back_the_same() {
    for p in &catalog().profiles {
        let back =
            PrinterProfile::from_text(&p.to_text()).unwrap_or_else(|e| panic!("{}: {e}", p.id));
        assert_eq!(&back, p, "{}", p.id);
    }
}

#[test]
fn the_text_of_a_profile_has_every_key_once() {
    let text = mk4s().to_text();
    let mut keys: Vec<&str> = text
        .lines()
        .filter_map(|l| l.split_once(" = ").map(|(k, _)| k))
        .collect();
    let n = keys.len();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), n);
    for key in [
        "id",
        "vendor",
        "model",
        "technology",
        "volume_x_mm",
        "dialect",
        "connectivity",
        "limit_source",
        "verified",
    ] {
        assert!(keys.contains(&key), "{key}");
    }
}

#[test]
fn a_file_with_a_key_that_is_not_known_or_one_that_is_missing_is_refused() {
    let good = mk4s().to_text();
    let extra = format!("{good}colour = red\n");
    let e = PrinterProfile::from_text(&extra).unwrap_err();
    assert!(e.to_string().contains("colour"), "{e}");
    let without_id: String = good
        .lines()
        .filter(|l| !l.starts_with("id = "))
        .map(|l| format!("{l}\n"))
        .collect();
    let e = PrinterProfile::from_text(&without_id).unwrap_err();
    assert!(e.to_string().contains("id"), "{e}");
    assert!(PrinterProfile::from_text("").is_err());
    assert!(PrinterProfile::from_text("nonsense").is_err());
}

#[test]
fn a_file_without_the_flags_is_not_verified_and_its_limits_are_the_owners() {
    let text: String = mk4s()
        .to_text()
        .lines()
        .filter(|l| !l.starts_with("verified") && !l.starts_with("limit_source"))
        .map(|l| format!("{l}\n"))
        .collect();
    let p = PrinterProfile::from_text(&text).unwrap();
    assert!(!p.verified);
    assert_eq!(p.limit_source, LimitSource::Owner);
}

#[test]
fn a_value_out_of_its_range_in_a_file_is_refused_naming_the_key() {
    let good = mk4s().to_text();
    for (from, to, key) in [
        ("volume_x_mm = 250", "volume_x_mm = 5001", "volume_x_mm"),
        ("volume_x_mm = 250", "volume_x_mm = 0.5", "volume_x_mm"),
        ("volume_x_mm = 250", "volume_x_mm = x", "volume_x_mm"),
        ("volume_x_mm = 250", "volume_x_mm = nan", "volume_x_mm"),
        ("nozzle_max_c = 290", "nozzle_max_c = 601", "nozzle_max_c"),
        ("nozzle_max_c = 290", "nozzle_max_c = 0.5", "nozzle_max_c"),
        ("bed_max_c = 120", "bed_max_c = 201", "bed_max_c"),
        (
            "nozzles_mm = 0.25,0.4,0.6",
            "nozzles_mm = 0.25,3",
            "nozzles_mm",
        ),
        ("technology = fdm", "technology = sla", "technology"),
        ("dialect = marlin", "dialect = basic", "dialect"),
        ("verified = off", "verified = perhaps", "verified"),
    ] {
        assert!(good.contains(from), "{from}");
        let e = PrinterProfile::from_text(&good.replace(from, to)).unwrap_err();
        assert!(e.to_string().contains(key), "{to}: {e}");
    }
}

#[test]
fn the_ways_of_connecting_of_a_file_are_a_list_and_one_that_is_not_known_refuses_it() {
    let good = mk4s().to_text();
    let line = good
        .lines()
        .find(|l| l.starts_with("connectivity"))
        .unwrap()
        .to_string();
    let wifi = good.replace(&line, "connectivity = serial, wifi");
    assert!(PrinterProfile::from_text(&wifi).is_err());
    let none = good.replace(&line, "connectivity = ");
    assert!(PrinterProfile::from_text(&none).is_err());
    let two = good.replace(&line, "connectivity =  serial ,, sd ");
    assert_eq!(
        PrinterProfile::from_text(&two).unwrap().connectivity,
        [Connectivity::Serial, Connectivity::SdCard]
    );
}

#[test]
fn a_profile_with_a_heated_chamber_keeps_it_in_the_file() {
    let mut p = mk4s();
    p.limits = Some(TempLimits {
        nozzle_c: 300.0,
        bed_c: 110.0,
        chamber_c: Some(60.0),
    });
    let back = PrinterProfile::from_text(&p.to_text()).unwrap();
    assert_eq!(back.limits.unwrap().chamber_c, Some(60.0));
}

// ---------------------------------------------------------------- editing a profile

#[test]
fn an_edit_that_changes_nothing_gives_the_same_profile_and_no_notice() {
    let p = mk4s();
    let e = p.edited(&[]).unwrap();
    assert_eq!(e.profile, p);
    assert!(e.notices.is_empty());
    let same = p.edited(&[("bed_max_c", "120")]).unwrap();
    assert_eq!(same.profile, p);
}

#[test]
fn the_keys_that_say_which_model_it_is_cannot_be_edited() {
    for key in ["id", "vendor", "model", "technology"] {
        let e = mk4s().edited(&[(key, "x")]).unwrap_err();
        assert!(e.to_string().starts_with(key), "{e}");
    }
}

#[test]
fn a_key_that_is_not_one_of_the_profile_cannot_be_edited() {
    assert!(mk4s().edited(&[("colour", "red")]).is_err());
    assert!(mk4s().edited(&[("", "x")]).is_err());
}

#[test]
fn a_resin_printer_has_no_nozzle_or_heater_key_to_edit() {
    for key in ["nozzles_mm", "nozzle_max_c", "bed_max_c", "chamber_max_c"] {
        assert!(resin_printer().edited(&[(key, "1")]).is_err(), "{key}");
    }
}

#[test]
fn a_limit_that_is_raised_says_so_and_the_limits_become_the_owners() {
    let p = mk4s();
    let e = p.edited(&[("nozzle_max_c", "300")]).unwrap();
    assert_eq!(e.profile.limits.unwrap().nozzle_c, 300.0);
    assert_eq!(e.profile.limit_source, LimitSource::Owner);
    assert_eq!(
        e.notices,
        [Notice::LimitRaised {
            key: "nozzle_max_c",
            from: 290.0,
            to: 300.0,
            above_maker: true
        }]
    );
}

#[test]
fn a_limit_that_is_lowered_gives_no_notice_but_is_still_the_owners() {
    let e = mk4s().edited(&[("bed_max_c", "90")]).unwrap();
    assert!(e.notices.is_empty());
    assert_eq!(e.profile.limit_source, LimitSource::Owner);
    assert_eq!(e.profile.limits.unwrap().bed_c, 90.0);
}

#[test]
fn a_source_that_is_named_in_the_edit_is_kept() {
    let e = mk4s()
        .edited(&[("nozzle_max_c", "250"), ("limit_source", "maker")])
        .unwrap();
    assert_eq!(e.profile.limit_source, LimitSource::Maker);
}

#[test]
fn a_chamber_can_be_added_by_an_edit_and_a_raise_from_nothing_is_told() {
    let e = mk4s().edited(&[("chamber_max_c", "60")]).unwrap();
    assert_eq!(e.profile.limits.unwrap().chamber_c, Some(60.0));
    assert!(e.notices.iter().any(|n| matches!(
        n,
        Notice::LimitRaised {
            key: "chamber_max_c",
            ..
        }
    )));
}

#[test]
fn a_value_that_is_out_of_range_in_an_edit_is_refused_and_nothing_changes() {
    let p = mk4s();
    for (key, value) in [
        ("nozzle_max_c", "601"),
        ("nozzle_max_c", "abc"),
        ("bed_max_c", "-5"),
        ("dialect", "basic"),
        ("connectivity", "wifi"),
    ] {
        assert!(p.edited(&[(key, value)]).is_err(), "{key} {value}");
    }
    assert_eq!(p, mk4s());
}

#[test]
fn an_edit_of_a_verified_profile_takes_the_verification_away_and_says_so() {
    let mut p = mk4s();
    p.verified = true;
    let e = p.edited(&[("bed_max_c", "100")]).unwrap();
    assert!(!e.profile.verified);
    assert!(e.notices.contains(&Notice::VerificationCleared));
    let same = p.edited(&[]).unwrap();
    assert!(same.profile.verified);
}

#[test]
fn a_profile_cannot_be_made_verified_by_an_edit() {
    let e = mk4s().edited(&[("verified", "on")]).unwrap_err();
    assert!(e.to_string().contains("report from a machine"), "{e}");
}

#[test]
fn the_difference_of_two_profiles_names_each_key_that_changed() {
    let p = mk4s();
    assert!(p.diff(&p).is_empty());
    let q = p.edited(&[("bed_max_c", "90")]).unwrap().profile;
    let d = p.diff(&q);
    let bed = d.iter().find(|c| c.key == "bed_max_c").unwrap();
    assert_eq!((bed.from.as_str(), bed.to.as_str()), ("120", "90"));
    assert!(d.iter().any(|c| c.key == "limit_source"));
    assert!(d.iter().all(|c| c.from != c.to));
}

#[test]
fn a_key_that_only_the_other_profile_has_is_a_change_from_nothing() {
    let p = mk4s();
    let q = p.edited(&[("chamber_max_c", "60")]).unwrap().profile;
    let d = p.diff(&q);
    let chamber = d.iter().find(|c| c.key == "chamber_max_c").unwrap();
    assert_eq!((chamber.from.as_str(), chamber.to.as_str()), ("", "60"));
}

#[test]
fn the_error_of_a_profile_is_its_text() {
    let e = ProfileError("why".into());
    assert_eq!(e.to_string(), "why");
}

// ---------------------------------------------------------------- several materials

fn caps_for(text: Option<&str>) -> Option<p3dp_profiles::MultiCaps> {
    let mut p = mk4s();
    p.multimaterial = text.map(str::to_string);
    caps_of(&p)
}

#[test]
fn a_printer_that_says_nothing_has_no_capacity() {
    assert!(caps_for(None).is_none());
    assert!(caps_for(Some("")).is_none());
    assert!(caps_for(Some("   ")).is_none());
}

#[test]
fn a_feeder_without_a_number_has_four_tools() {
    for text in ["AMS", "AMS lite", "Custom feeder", "units"] {
        let c = caps_for(Some(text)).unwrap();
        assert_eq!(
            (c.kind, c.max_tools, c.optional),
            (MultiKind::Feeder, 4, false),
            "{text}"
        );
    }
}

#[test]
fn units_of_four_slots_are_counted_and_the_total_is_never_over_sixteen() {
    assert_eq!(caps_for(Some("AMS (up to 2 units)")).unwrap().max_tools, 8);
    assert_eq!(caps_for(Some("AMS units")).unwrap().max_tools, 4);
    assert_eq!(caps_for(Some("AMS (up to 4 units)")).unwrap().max_tools, 16);
    assert_eq!(caps_for(Some("AMS (up to 9 units)")).unwrap().max_tools, 16);
}

#[test]
fn a_toolchanger_and_an_idex_have_heads() {
    let t = caps_for(Some("toolchanger (up to 5)")).unwrap();
    assert_eq!((t.kind, t.max_tools), (MultiKind::Heads, 5));
    assert_eq!(caps_for(Some("toolchanger")).unwrap().max_tools, 4);
    assert_eq!(
        caps_for(Some("toolchanger (up to 99)")).unwrap().max_tools,
        16
    );
    let i = caps_for(Some("IDEX")).unwrap();
    assert_eq!((i.kind, i.max_tools), (MultiKind::Heads, 2));
}

#[test]
fn a_toolchanger_with_a_number_that_does_not_fit_a_byte_has_the_default() {
    assert_eq!(
        caps_for(Some("toolchanger (up to 300)")).unwrap().max_tools,
        4
    );
}

#[test]
fn a_kit_that_is_optional_is_told_so_and_an_external_one_is_always_optional() {
    let mmu = caps_for(Some("MMU3 optional")).unwrap();
    assert_eq!(
        (mmu.kind, mmu.max_tools, mmu.optional),
        (MultiKind::Feeder, 5, true)
    );
    let ext = caps_for(Some("optional kit")).unwrap();
    assert_eq!(
        (ext.kind, ext.max_tools, ext.optional),
        (MultiKind::External, 4, true)
    );
}

#[test]
fn the_number_of_tools_is_the_one_in_the_words_and_between_two_and_sixteen() {
    assert_eq!(
        caps_for(Some("CFS (4 colours, up to 16)"))
            .unwrap()
            .max_tools,
        16
    );
    assert_eq!(caps_for(Some("3 colours")).unwrap().max_tools, 3);
    assert_eq!(caps_for(Some("1 colour")).unwrap().max_tools, 2);
    assert_eq!(caps_for(Some("0")).unwrap().max_tools, 2);
    assert_eq!(caps_for(Some("40 colours")).unwrap().max_tools, 16);
}

#[test]
fn the_words_are_kept_trimmed_and_the_kinds_have_their_names() {
    assert_eq!(caps_for(Some("  AMS  ")).unwrap().text, "AMS");
    assert_eq!(MultiKind::Feeder.name(), "feeder");
    assert_eq!(MultiKind::Heads.name(), "heads");
    assert_eq!(MultiKind::External.name(), "external");
}

#[test]
fn every_printer_of_the_catalog_that_says_something_has_a_capacity_in_range() {
    for p in &catalog().profiles {
        match (&p.multimaterial, caps_of(p)) {
            (Some(_), Some(c)) => assert!((2..=16).contains(&c.max_tools), "{}", p.id),
            (None, None) => {}
            (m, c) => panic!("{}: {m:?} gave {c:?}", p.id),
        }
    }
}

// ---------------------------------------------------------------- filaments

const PLA: &str = "id = test/pla\nname = Test PLA\nmaterial = pla\nnozzle_c = 210, 190, 230\nbed_c = 60, 50, 70\nfan_percent = 100, 80, 100\ndensity_g_cm3 = 1.24\ndiameter_mm = 1.75\nmin_nozzle_mm = 0.2\n";

fn pla() -> FilamentProfile {
    FilamentProfile::from_text(PLA).unwrap()
}

fn nozzle(d: f64) -> Nozzle {
    Nozzle {
        diameter_mm: d,
        hardened: false,
    }
}

fn range(best: f64, lowest: f64, highest: f64) -> Range {
    Range {
        best,
        lowest,
        highest,
    }
}

#[test]
fn a_filament_is_read_from_its_text_and_written_back_the_same() {
    let p = pla();
    assert_eq!(
        (p.id.as_str(), p.nozzle_c, p.fan_percent.lowest),
        ("test/pla", range(210.0, 190.0, 230.0), 80.0)
    );
    assert!(!p.abrasive && !p.verified && p.manufacturer.is_empty());
    assert_eq!(FilamentProfile::from_text(&p.to_text()).unwrap(), p);
}

#[test]
fn the_flags_and_the_maker_of_a_filament_are_read() {
    let text = format!("{PLA}manufacturer = Acme\nabrasive = on\nenclosure_advised = yes\ndirect_drive_advised = 1\nverified = true\n");
    let p = FilamentProfile::from_text(&text).unwrap();
    assert_eq!(p.manufacturer, "Acme");
    assert!(p.abrasive && p.enclosure_advised && p.direct_drive_advised && p.verified);
    assert_eq!(FilamentProfile::from_text(&p.to_text()).unwrap(), p);
}

#[test]
fn the_last_line_of_a_key_wins_in_a_filament_file() {
    let p = FilamentProfile::from_text(&format!("{PLA}density_g_cm3 = 1.3\n")).unwrap();
    assert_eq!(p.density_g_cm3, 1.3);
}

#[test]
fn a_filament_file_with_something_wrong_is_refused() {
    for (from, to) in [
        ("material = pla", "material = unobtainium"),
        ("nozzle_c = 210, 190, 230", "nozzle_c = 210, 190"),
        ("nozzle_c = 210, 190, 230", "nozzle_c = 180, 190, 230"),
        ("nozzle_c = 210, 190, 230", "nozzle_c = 210, 190, 501"),
        ("fan_percent = 100, 80, 100", "fan_percent = 101, 80, 101"),
        ("density_g_cm3 = 1.24", "density_g_cm3 = 0.4"),
        ("density_g_cm3 = 1.24", "density_g_cm3 = 5.1"),
        ("density_g_cm3 = 1.24", "density_g_cm3 = nan"),
        ("diameter_mm = 1.75", "diameter_mm = 2.0"),
        ("min_nozzle_mm = 0.2", "min_nozzle_mm = 0.05"),
        ("min_nozzle_mm = 0.2", "min_nozzle_mm = 2.1"),
        ("id = test/pla", "id = Test/PLA"),
        ("id = test/pla", "id = "),
    ] {
        assert!(
            FilamentProfile::from_text(&PLA.replace(from, to)).is_err(),
            "{to}"
        );
    }
    assert!(FilamentProfile::from_text(&format!("{PLA}colour = red\n")).is_err());
    assert!(FilamentProfile::from_text(&PLA.replace("name = Test PLA\n", "")).is_err());
    assert!(FilamentProfile::from_text("").is_err());
}

#[test]
fn the_diameters_that_exist_and_the_edges_of_the_ranges_are_valid() {
    for d in [1.75, 2.85, 3.0] {
        let text = PLA.replace("diameter_mm = 1.75", &format!("diameter_mm = {d}"));
        assert!(FilamentProfile::from_text(&text).is_ok(), "{d}");
    }
    for density in ["0.5", "5"] {
        let text = PLA.replace(
            "density_g_cm3 = 1.24",
            &format!("density_g_cm3 = {density}"),
        );
        assert!(FilamentProfile::from_text(&text).is_ok(), "{density}");
    }
    let text = PLA.replace("nozzle_c = 210, 190, 230", "nozzle_c = 0, 0, 500");
    assert!(FilamentProfile::from_text(&text).is_ok());
}

#[test]
fn a_filament_name_that_would_begin_another_line_is_refused() {
    for bad in ["A\nverified = on", "A\0", "A #x"] {
        let mut p = pla();
        p.name = bad.to_string();
        assert!(p.validate().is_err(), "{bad:?}");
        let mut q = pla();
        q.manufacturer = bad.to_string();
        assert!(q.validate().is_err(), "{bad:?}");
    }
}

#[test]
fn a_filament_that_fits_a_printer_is_started_from_its_best_values() {
    let c = pla().check(&mk4s(), nozzle(0.4));
    assert!(c.ok());
    assert!(c.issues.is_empty() && c.advice.is_empty());
    assert_eq!(
        (c.nozzle_c, c.bed_c, c.fan_percent),
        (Some(210.0), Some(60.0), 100.0)
    );
}

#[test]
fn a_limit_of_the_printer_below_the_best_value_but_above_the_lowest_caps_it() {
    let mut p = mk4s();
    p.limits = Some(TempLimits {
        nozzle_c: 200.0,
        bed_c: 55.0,
        chamber_c: None,
    });
    let c = pla().check(&p, nozzle(0.4));
    assert!(c.ok());
    assert_eq!((c.nozzle_c, c.bed_c), (Some(200.0), Some(55.0)));
}

#[test]
fn a_limit_of_the_printer_below_the_lowest_value_is_an_issue_with_no_value() {
    let mut p = mk4s();
    p.limits = Some(TempLimits {
        nozzle_c: 180.0,
        bed_c: 40.0,
        chamber_c: None,
    });
    let c = pla().check(&p, nozzle(0.4));
    assert!(!c.ok());
    assert_eq!(
        c.issues,
        [
            Issue::NozzleTooCold {
                lowest_c: 190.0,
                limit_c: 180.0
            },
            Issue::BedTooCold {
                lowest_c: 50.0,
                limit_c: 40.0
            }
        ]
    );
    assert_eq!((c.nozzle_c, c.bed_c), (None, None));
}

#[test]
fn a_nozzle_that_the_printer_does_not_list_or_that_is_too_small_is_an_issue() {
    let c = pla().check(&mk4s(), nozzle(0.5));
    assert_eq!(c.issues, [Issue::NozzleNotOffered { diameter_mm: 0.5 }]);
    let small = pla().check(&mk4s(), nozzle(0.1));
    assert_eq!(
        small.issues,
        [
            Issue::NozzleNotOffered { diameter_mm: 0.1 },
            Issue::NozzleTooSmall {
                diameter_mm: 0.1,
                smallest_mm: 0.2
            }
        ]
    );
    let nan = pla().check(&mk4s(), nozzle(f64::NAN));
    assert!(!nan.ok());
}

#[test]
fn an_abrasive_filament_needs_a_hardened_nozzle() {
    let mut p = pla();
    p.abrasive = true;
    assert_eq!(
        p.check(&mk4s(), nozzle(0.4)).issues,
        [Issue::NeedsHardenedNozzle]
    );
    let hard = Nozzle {
        diameter_mm: 0.4,
        hardened: true,
    };
    assert!(p.check(&mk4s(), hard).ok());
}

#[test]
fn the_advice_is_given_for_a_closed_space_that_is_not_there_and_for_a_direct_drive() {
    let mut p = pla();
    p.enclosure_advised = true;
    p.direct_drive_advised = true;
    let c = p.check(&mk4s(), nozzle(0.4));
    assert!(c.ok());
    assert_eq!(c.advice, [Advice::Enclosure, Advice::DirectDrive]);
    let mut warm = mk4s();
    warm.limits = Some(TempLimits {
        nozzle_c: 290.0,
        bed_c: 120.0,
        chamber_c: Some(60.0),
    });
    assert_eq!(p.check(&warm, nozzle(0.4)).advice, [Advice::DirectDrive]);
    assert!(!Advice::Enclosure.text().is_empty() && !Advice::DirectDrive.text().is_empty());
}

#[test]
fn a_resin_printer_takes_no_filament() {
    let c = pla().check(&resin_printer(), nozzle(0.4));
    assert_eq!(c.issues, [Issue::NotAFilamentPrinter]);
    assert_eq!((c.nozzle_c, c.bed_c), (None, None));
}

#[test]
fn every_issue_has_a_sentence_with_its_numbers() {
    let all = [
        Issue::NozzleTooCold {
            lowest_c: 190.0,
            limit_c: 180.0,
        },
        Issue::BedTooCold {
            lowest_c: 50.0,
            limit_c: 40.0,
        },
        Issue::NozzleNotOffered { diameter_mm: 0.5 },
        Issue::NozzleTooSmall {
            diameter_mm: 0.1,
            smallest_mm: 0.2,
        },
        Issue::NeedsHardenedNozzle,
        Issue::NotAFilamentPrinter,
    ];
    let texts: Vec<String> = all.iter().map(Issue::text).collect();
    assert!(texts.iter().all(|t| !t.is_empty()));
    assert!(texts[0].contains("190") && texts[0].contains("180"));
    let mut sorted = texts.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), texts.len());
}

#[test]
fn the_built_in_filaments_are_all_valid_with_different_ids_and_are_checked_against_a_printer_without_a_panic(
) {
    let c = FilamentCatalog::builtin().unwrap();
    assert!(!c.is_empty());
    assert_eq!(c.len(), c.profiles.len());
    let mut seen = std::collections::HashSet::new();
    let printer = mk4s();
    for p in &c.profiles {
        p.validate().unwrap_or_else(|e| panic!("{}: {e}", p.id));
        assert!(seen.insert(p.id.as_str()), "{} twice", p.id);
        let _ = p.check(&printer, nozzle(0.4));
    }
}

#[test]
fn a_filament_is_found_by_its_id_in_any_case_and_an_old_name_leads_to_the_new_one() {
    let c = FilamentCatalog::builtin().unwrap();
    let first = &c.profiles[0];
    assert_eq!(c.find(&first.id.to_ascii_uppercase()).unwrap().id, first.id);
    assert_eq!(c.find(&format!(" {} ", first.id)).unwrap().id, first.id);
    assert!(c.find("").is_none());
    for (old, new) in &c.aliases {
        assert_eq!(c.find(old).unwrap().id, *new, "{old}");
    }
}

#[test]
fn the_filaments_of_a_material_all_are_of_it_and_the_makers_are_listed_in_order() {
    let c = FilamentCatalog::builtin().unwrap();
    let pla = Material::from_name("pla").unwrap();
    let of = c.of(pla);
    assert!(!of.is_empty());
    assert!(of.iter().all(|p| p.material == pla));
    let makers = c.manufacturers();
    assert!(makers.iter().all(|m| !m.is_empty()));
    for w in makers.windows(2) {
        assert!(
            w[0].to_ascii_lowercase() < w[1].to_ascii_lowercase(),
            "{w:?}"
        );
    }
}

#[test]
fn every_built_in_filament_is_written_and_read_back_the_same() {
    for p in &FilamentCatalog::builtin().unwrap().profiles {
        let back =
            FilamentProfile::from_text(&p.to_text()).unwrap_or_else(|e| panic!("{}: {e}", p.id));
        assert_eq!(&back, p, "{}", p.id);
    }
}

// ---------------------------------------------------------------- resins

const RESIN: &str = "id = test/resin\nname = Test resin\nkind = standard\nlayer_um = 50, 25, 100\nnormal_exposure_s = 2.5, 1, 10\nbottom_exposure_s = 30, 10, 60\nbottom_layers = 6\nlift_mm = 6\nlift_speed_mm_min = 120\nwavelength_nm = 405\ndensity_g_ml = 1.1\nwash = alcohol\n";

fn resin() -> ResinProfile {
    ResinProfile::from_text(RESIN).unwrap()
}

#[test]
fn the_kinds_and_the_washes_of_a_resin_are_known_by_their_names() {
    for k in ResinKind::ALL {
        assert_eq!(ResinKind::from_name(k.name()), Some(k));
    }
    assert_eq!(
        ResinKind::from_name(" HIGH-DETAIL "),
        Some(ResinKind::HighDetail)
    );
    assert_eq!(ResinKind::from_name("high detail"), None);
    for w in [Wash::Alcohol, Wash::Water] {
        assert_eq!(Wash::from_name(w.name()), Some(w));
    }
    assert_eq!(Wash::from_name("soap"), None);
}

#[test]
fn a_resin_is_read_from_its_text_and_written_back_the_same() {
    let r = resin();
    assert_eq!(
        (r.kind, r.bottom_layers, r.wash, r.verified),
        (ResinKind::Standard, 6, Wash::Alcohol, false)
    );
    assert_eq!(ResinProfile::from_text(&r.to_text()).unwrap(), r);
    let verified = ResinProfile::from_text(&format!("{RESIN}verified = on\n")).unwrap();
    assert!(verified.verified);
}

#[test]
fn a_resin_file_with_something_wrong_is_refused() {
    for (from, to) in [
        ("kind = standard", "kind = soft"),
        ("wash = alcohol", "wash = soap"),
        ("layer_um = 50, 25, 100", "layer_um = 50, 25"),
        ("layer_um = 50, 25, 100", "layer_um = 50, 0, 100"),
        ("layer_um = 50, 25, 100", "layer_um = 50, 25, 201"),
        ("layer_um = 50, 25, 100", "layer_um = 20, 25, 100"),
        (
            "normal_exposure_s = 2.5, 1, 10",
            "normal_exposure_s = 2.5, 1, 61",
        ),
        (
            "bottom_exposure_s = 30, 10, 60",
            "bottom_exposure_s = 2, 1, 60",
        ),
        ("bottom_layers = 6", "bottom_layers = 0"),
        ("bottom_layers = 6", "bottom_layers = 21"),
        ("lift_mm = 6", "lift_mm = 0.5"),
        ("lift_mm = 6", "lift_mm = 21"),
        ("lift_speed_mm_min = 120", "lift_speed_mm_min = 9"),
        ("lift_speed_mm_min = 120", "lift_speed_mm_min = 601"),
        ("wavelength_nm = 405", "wavelength_nm = 349"),
        ("wavelength_nm = 405", "wavelength_nm = 501"),
        ("density_g_ml = 1.1", "density_g_ml = 0.8"),
        ("density_g_ml = 1.1", "density_g_ml = 1.7"),
        ("name = Test resin", "name = "),
        ("id = test/resin", "id = Test"),
    ] {
        assert!(
            ResinProfile::from_text(&RESIN.replace(from, to)).is_err(),
            "{to}"
        );
    }
    assert!(ResinProfile::from_text(&format!("{RESIN}colour = red\n")).is_err());
    assert!(ResinProfile::from_text("").is_err());
}

#[test]
fn the_edges_of_a_resin_are_valid() {
    for (from, to) in [
        ("bottom_layers = 6", "bottom_layers = 1"),
        ("bottom_layers = 6", "bottom_layers = 20"),
        ("lift_mm = 6", "lift_mm = 1"),
        ("lift_mm = 6", "lift_mm = 20"),
        ("wavelength_nm = 405", "wavelength_nm = 350"),
        ("wavelength_nm = 405", "wavelength_nm = 500"),
        ("density_g_ml = 1.1", "density_g_ml = 0.9"),
        ("density_g_ml = 1.1", "density_g_ml = 1.6"),
        ("lift_speed_mm_min = 120", "lift_speed_mm_min = 10"),
        ("lift_speed_mm_min = 120", "lift_speed_mm_min = 600"),
        (
            "bottom_exposure_s = 30, 10, 60",
            "bottom_exposure_s = 2.5, 1, 60",
        ),
    ] {
        assert!(
            ResinProfile::from_text(&RESIN.replace(from, to)).is_ok(),
            "{to}"
        );
    }
}

#[test]
fn a_resin_name_that_would_begin_another_line_is_refused() {
    for bad in ["A\nverified = on", "A\0", "A #x"] {
        let mut r = resin();
        r.name = bad.to_string();
        assert!(r.validate().is_err(), "{bad:?}");
    }
}

#[test]
fn a_resin_is_for_a_resin_printer_only() {
    assert!(resin().usable_on(&resin_printer()).is_ok());
    let e = resin().usable_on(&mk4s()).unwrap_err();
    assert!(e.to_string().contains("takes no resin"), "{e}");
}

#[test]
fn the_resin_a_part_uses_is_its_volume_with_the_waste_and_the_price() {
    let u = resin().used(1000.0, 10.0, 50.0).unwrap();
    assert!((u.part_ml - 1.0).abs() < 1e-12);
    assert!((u.total_ml - 1.1).abs() < 1e-12);
    assert!((u.grams - 1.21).abs() < 1e-12);
    assert!((u.cost - 0.055).abs() < 1e-12);
    let none = resin().used(0.0, 0.0, 0.0).unwrap();
    assert_eq!(
        (none.part_ml, none.total_ml, none.grams, none.cost),
        (0.0, 0.0, 0.0, 0.0)
    );
}

#[test]
fn a_volume_a_waste_or_a_price_that_is_not_allowed_is_refused() {
    let r = resin();
    for (v, w, p) in [
        (-1.0, 0.0, 0.0),
        (f64::NAN, 0.0, 0.0),
        (f64::INFINITY, 0.0, 0.0),
        (1.0, -0.1, 0.0),
        (1.0, 500.5, 0.0),
        (1.0, f64::NAN, 0.0),
        (1.0, 0.0, -1.0),
        (1.0, 0.0, f64::NAN),
        (1.0, 0.0, f64::INFINITY),
    ] {
        assert!(r.used(v, w, p).is_err(), "{v} {w} {p}");
    }
    assert!(r.used(1.0, 500.0, 0.0).is_ok());
}

#[test]
fn the_built_in_resins_are_all_valid_and_the_old_names_lead_to_the_new_ones() {
    let c = ResinCatalog::builtin().unwrap();
    assert!(!c.is_empty());
    assert_eq!(c.len(), c.profiles.len());
    let mut seen = std::collections::HashSet::new();
    for p in &c.profiles {
        p.validate().unwrap_or_else(|e| panic!("{}: {e}", p.id));
        assert!(seen.insert(p.id.as_str()), "{} twice", p.id);
    }
    for (old, new) in &c.aliases {
        assert_eq!(c.find(old).unwrap().id, *new, "{old}");
    }
    assert!(c.find("").is_none());
    let first = &c.profiles[0];
    assert_eq!(
        c.find(&format!(" {} ", first.id.to_ascii_uppercase()))
            .unwrap()
            .id,
        first.id
    );
}

#[test]
fn the_resins_of_a_kind_all_are_of_it() {
    let c = ResinCatalog::builtin().unwrap();
    let mut total = 0;
    for k in ResinKind::ALL {
        let of = c.of(k);
        assert!(of.iter().all(|p| p.kind == k));
        total += of.len();
    }
    assert_eq!(total, c.len());
}

#[test]
fn every_built_in_resin_is_written_and_read_back_the_same() {
    for p in &ResinCatalog::builtin().unwrap().profiles {
        let back =
            ResinProfile::from_text(&p.to_text()).unwrap_or_else(|e| panic!("{}: {e}", p.id));
        assert_eq!(&back, p, "{}", p.id);
    }
}

// ---------------------------------------------------------------- a profile that came with a project

#[test]
fn a_profile_from_elsewhere_can_lower_limits_and_room_and_never_raise_them() {
    let own = mk4s();
    let limits = own.limits.unwrap();
    let mut theirs = own.clone();
    theirs.limits = Some(TempLimits {
        nozzle_c: limits.nozzle_c + 100.0,
        bed_c: limits.bed_c - 20.0,
        chamber_c: Some(70.0),
    });
    theirs.volume_mm = [
        own.volume_mm[0] + 50.0,
        own.volume_mm[1] - 10.0,
        own.volume_mm[2],
    ];
    theirs.verified = true;
    theirs.nozzles_mm = vec![2.0];
    let t = own.tightened_by(&theirs);
    let now = t.profile.limits.unwrap();
    assert_eq!(now.nozzle_c, limits.nozzle_c);
    assert_eq!(now.bed_c, limits.bed_c - 20.0);
    assert_eq!(now.chamber_c, limits.chamber_c);
    assert_eq!(t.profile.volume_mm[0], own.volume_mm[0]);
    assert_eq!(t.profile.volume_mm[1], own.volume_mm[1] - 10.0);
    // nothing else of the other profile is taken
    assert_eq!(t.profile.verified, own.verified);
    assert_eq!(t.profile.nozzles_mm, own.nozzles_mm);
    let keys = |list: &[p3dp_profiles::Bound]| list.iter().map(|b| b.key).collect::<Vec<_>>();
    assert_eq!(keys(&t.lowered), ["volume_y_mm", "bed_max_c"]);
    assert_eq!(
        keys(&t.refused),
        ["volume_x_mm", "nozzle_max_c", "chamber_max_c"]
    );
    // the same profile changes nothing, and a resin printer keeps having no limits
    let same = own.tightened_by(&own);
    assert!(same.lowered.is_empty() && same.refused.is_empty());
    assert_eq!(resin_printer().tightened_by(&own).profile.limits, None);
}
