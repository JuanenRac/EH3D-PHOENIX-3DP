//! The words of the reports: what a slice comes to (material, cost, time, the short and the slowest layers), what a resin print takes, the thumbnails of a file and what an edit of a
//! profile means. The figures come from the libraries; here they are put in a language of the program, in the order of the places of each text, and a figure that makes no sense is an error
//! and not a line with a made-up number.

use crate::config::AppConfig;
use crate::i18n::{fill, Lang, Msg};
use crate::preview::format_duration;
use p3dp_formats::picture::PictureFormat;
use p3dp_profiles::{Notice, ResinProfile};
use p3dp_slicer::report::{material, time_by_group, time_report, Group};
use p3dp_slicer::{LayerSummary, Plan};

/// How many layers a list in a report names before it says there are more.
const LIST_LAYERS: usize = 10;

/// The density of the plastic when no filament was chosen (a generic PLA).
pub const DEFAULT_DENSITY_G_CM3: f64 = 1.24;

/// A list of layers, counted from 1 as the window counts them; more than the first few are cut with `...`.
pub fn layer_list(layers: &[usize], lang: Lang) -> String {
    if layers.is_empty() {
        return Msg::ReportNone.tr(lang).to_string();
    }
    let mut s = layers
        .iter()
        .take(LIST_LAYERS)
        .map(|l| (l + 1).to_string())
        .collect::<Vec<_>>()
        .join(", ");
    if layers.len() > LIST_LAYERS {
        s.push_str(&format!(", ... ({})", layers.len()));
    }
    s
}

/// The lines about a slice: the material, its cost, the time, the layers that take less than the time in the settings and the slowest ones.
pub fn slice_lines(
    plan: &Plan,
    density_g_cm3: f64,
    config: &AppConfig,
    lang: Lang,
) -> Result<Vec<String>, String> {
    lines_from(plan, &plan.summaries(), density_g_cm3, config, lang)
}

/// The same lines from the summaries of the layers that were already worked out (the window keeps them, so the report does not slice the plan again each time it is drawn).
pub fn lines_from(
    plan: &Plan,
    summaries: &[LayerSummary],
    density_g_cm3: f64,
    config: &AppConfig,
    lang: Lang,
) -> Result<Vec<String>, String> {
    let m =
        material(plan, density_g_cm3, config.price_filament_per_kg).map_err(|e| e.to_string())?;
    let mut out = vec![fill(
        Msg::ReportMaterial.tr(lang),
        &[
            &format!("{:.2}", m.length_mm / 1000.0),
            &format!("{:.2}", m.volume_mm3 / 1000.0),
            &format!("{:.1}", m.mass_g),
        ],
    )];
    if config.price_filament_per_kg > 0.0 {
        out.push(fill(Msg::ReportCost.tr(lang), &[&format!("{:.2}", m.cost)]));
    }
    let r = time_report(
        summaries,
        config.report_short_layer_s,
        config.report_slowest_layers,
    );
    out.push(fill(
        Msg::ReportTime.tr(lang),
        &[&format_duration(r.total_s)],
    ));
    if config.report_short_layer_s > 0.0 {
        out.push(fill(
            Msg::ReportShortLayers.tr(lang),
            &[
                &config.report_short_layer_s.to_string(),
                &layer_list(&r.short_layers, lang),
            ],
        ));
    }
    if config.report_slowest_layers > 0 {
        let slow: Vec<String> = r
            .slowest
            .iter()
            .map(|(l, s)| format!("{} ({})", l + 1, format_duration(*s)))
            .collect();
        out.push(fill(Msg::ReportSlowestLayers.tr(lang), &[&slow.join(", ")]));
    }
    out.push(fill(
        Msg::ReportTravel.tr(lang),
        &[&format!("{:.2}", r.travel_mm / 1000.0)],
    ));
    let shares = time_by_group(plan, summaries);
    let total: f64 = shares.iter().map(|t| t.seconds).sum();
    if total > 0.0 {
        let parts: Vec<String> = shares
            .iter()
            .map(|t| {
                let name = match t.group {
                    Group::Perimeters => Msg::KindPerimeters,
                    Group::Infill => Msg::KindInfill,
                    Group::Solid => Msg::KindSolid,
                    Group::Bridges => Msg::PanelBridges,
                    Group::Supports => Msg::PanelSupports,
                    Group::Adhesion => Msg::KindAdhesion,
                    Group::Other => Msg::KindOther,
                };
                format!("{} {:.0} %", name.tr(lang), t.seconds / total * 100.0)
            })
            .collect();
        out.push(fill(Msg::ReportTimeByKind.tr(lang), &[&parts.join(", ")]));
    }
    Ok(out)
}

/// The line about the resin a part of this volume takes, with the waste and the price of the settings.
pub fn resin_line(
    profile: &ResinProfile,
    volume_mm3: f64,
    config: &AppConfig,
    lang: Lang,
) -> Result<String, String> {
    let u = profile
        .used(
            volume_mm3,
            config.resin_waste_percent,
            config.price_resin_per_litre,
        )
        .map_err(|e| e.to_string())?;
    let cost = if config.price_resin_per_litre > 0.0 {
        format!("{:.2}", u.cost)
    } else {
        "-".to_string()
    };
    Ok(fill(
        Msg::ReportResin.tr(lang),
        &[
            &format!("{:.2}", u.part_ml),
            &format!("{:.2}", u.total_ml),
            &format!("{:.1}", u.grams),
            &cost,
        ],
    ))
}

/// The line about one thumbnail (`number` counts from 1).
pub fn thumbnail_line(
    number: usize,
    width: u32,
    height: u32,
    format: PictureFormat,
    bytes: usize,
    lang: Lang,
) -> String {
    fill(
        Msg::ThumbnailLine.tr(lang),
        &[
            &number.to_string(),
            &width.to_string(),
            &height.to_string(),
            &format.extension().to_uppercase(),
            &bytes.to_string(),
        ],
    )
}

/// What an edit of a profile means, one line for each notice (a limit raised past the figure of the maker adds the line that says the decision is the owner's).
pub fn notice_lines(notices: &[Notice], lang: Lang) -> Vec<String> {
    let mut out = Vec::new();
    for n in notices {
        match n {
            Notice::LimitRaised {
                key,
                from,
                to,
                above_maker,
            } => {
                out.push(fill(
                    Msg::NoticeLimitRaised.tr(lang),
                    &[key, &from.to_string(), &to.to_string()],
                ));
                if *above_maker {
                    out.push(Msg::NoticeAboveMaker.tr(lang).to_string());
                }
            }
            Notice::VerificationCleared => {
                out.push(Msg::NoticeVerificationCleared.tr(lang).to_string())
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_core::mesh::cube;
    use p3dp_core::{Dialect, TempLimits};
    use p3dp_profiles::{Catalog, ResinCatalog};
    use p3dp_slicer::{plan, Machine, SliceConfig};

    fn sliced() -> Plan {
        let m = Machine {
            id: "report-test".to_string(),
            dialect: Dialect::Marlin,
            volume_mm: [220.0, 220.0, 250.0],
            limits: TempLimits {
                nozzle_c: 300.0,
                bed_c: 110.0,
                chamber_c: None,
            },
        };
        plan(&cube(20.0), &m, &SliceConfig::default()).unwrap()
    }

    #[test]
    fn the_lines_of_a_slice_name_the_material_the_cost_the_time_and_the_layers_in_every_language() {
        let p = sliced();
        let cfg = AppConfig::default();
        for lang in Lang::ALL {
            let l = slice_lines(&p, DEFAULT_DENSITY_G_CM3, &cfg, lang).unwrap();
            // material, cost, time, short layers, slowest layers, travel, where the time goes
            assert_eq!(l.len(), 7, "{}: {l:?}", lang.code());
            assert!(
                l.iter().all(|x| !x.contains("{}")),
                "{}: {l:?}",
                lang.code()
            );
            assert!(l[0].contains("cm3"), "{}", l[0]);
        }
        let en = slice_lines(&p, DEFAULT_DENSITY_G_CM3, &cfg, Lang::En).unwrap();
        assert!(
            en[0].starts_with("Material: ") && en[1].starts_with("Cost of the filament: "),
            "{en:?}"
        );
        assert!(
            en[2].starts_with("Time: ") && en[4].starts_with("Slowest layers: "),
            "{en:?}"
        );
        // the slowest layers are counted from 1 and are as many as the settings ask
        assert_eq!(en[4].matches('(').count(), cfg.report_slowest_layers);
    }

    #[test]
    fn a_price_of_zero_hides_the_cost_and_zero_layers_hide_their_lists() {
        let p = sliced();
        let cfg = AppConfig {
            price_filament_per_kg: 0.0,
            report_short_layer_s: 0.0,
            report_slowest_layers: 0,
            ..AppConfig::default()
        };
        let l = slice_lines(&p, DEFAULT_DENSITY_G_CM3, &cfg, Lang::En).unwrap();
        assert_eq!(l.len(), 4, "{l:?}");
        assert!(l
            .iter()
            .all(|x| !x.contains("Cost") && !x.contains("Layers under") && !x.contains("Slowest")));
        // a density that makes no sense is an error, not a line
        assert!(slice_lines(&p, 0.0, &cfg, Lang::En)
            .unwrap_err()
            .contains("density"));
    }

    #[test]
    fn the_last_line_says_where_the_time_goes_in_every_language_and_the_shares_make_a_hundred() {
        let p = sliced();
        let cfg = AppConfig::default();
        for lang in Lang::ALL {
            let l = slice_lines(&p, DEFAULT_DENSITY_G_CM3, &cfg, lang).unwrap();
            let last = l.last().unwrap();
            let head = Msg::ReportTimeByKind
                .tr(lang)
                .split("{}")
                .next()
                .unwrap()
                .to_string();
            assert!(last.starts_with(&head), "{}: {last}", lang.code());
            assert!(
                last.contains(Msg::KindPerimeters.tr(lang)),
                "{}",
                lang.code()
            );
            assert!(last.contains(Msg::KindOther.tr(lang)), "{}", lang.code());
            // every share is a whole number of per cent and they add up to a hundred, give or take the rounding of each
            let shares: Vec<f64> = last
                .split(" %")
                .filter_map(|part| part.rsplit(|c: char| !c.is_ascii_digit()).next())
                .filter(|d| !d.is_empty())
                .map(|d| d.parse().unwrap())
                .collect();
            assert!(shares.len() >= 3, "{}: {last}", lang.code());
            let sum: f64 = shares.iter().sum();
            assert!(
                (sum - 100.0).abs() <= shares.len() as f64 / 2.0,
                "{}: {last}",
                lang.code()
            );
        }
        assert!(Msg::ReportTimeByKind.tr(Lang::Pt).contains("Para onde"));
    }

    #[test]
    fn the_list_of_layers_counts_from_one_cuts_a_long_list_and_says_none_for_an_empty_one() {
        assert_eq!(layer_list(&[], Lang::Es), "ninguna");
        assert_eq!(layer_list(&[0, 4], Lang::En), "1, 5");
        let long: Vec<usize> = (0..25).collect();
        let s = layer_list(&long, Lang::En);
        assert!(
            s.starts_with("1, 2, 3") && s.contains("10, ...") && s.ends_with("(25)"),
            "{s}"
        );
    }

    #[test]
    fn the_resin_line_counts_the_waste_and_hides_a_cost_when_there_is_no_price() {
        let r = ResinCatalog::builtin()
            .unwrap()
            .find("generic-standard")
            .unwrap()
            .clone();
        let cfg = AppConfig::default();
        let l = resin_line(&r, 10_000.0, &cfg, Lang::En).unwrap();
        assert_eq!(
            l,
            "Resin: 10.00 ml in the part, 12.00 ml with the waste, 13.2 g, cost 0.48"
        );
        let free = AppConfig {
            price_resin_per_litre: 0.0,
            ..AppConfig::default()
        };
        assert!(resin_line(&r, 10_000.0, &free, Lang::Pt)
            .unwrap()
            .ends_with("custo -"));
        assert!(resin_line(&r, -1.0, &cfg, Lang::En).is_err());
    }

    #[test]
    fn a_thumbnail_line_has_its_number_size_format_and_bytes_in_order() {
        let l = thumbnail_line(2, 220, 124, PictureFormat::Png, 5120, Lang::En);
        assert_eq!(l, "Thumbnail 2: 220 x 124 PNG, 5120 bytes");
        assert!(thumbnail_line(1, 16, 16, PictureFormat::Qoi, 10, Lang::Fr).contains("octets"));
    }

    #[test]
    fn the_notices_of_an_edit_say_what_was_raised_and_that_the_decision_is_the_owners() {
        let a1 = Catalog::builtin()
            .unwrap()
            .find("bambu-lab-a1-mini")
            .unwrap()
            .clone();
        let e = a1.edited(&[("bed_max_c", "100")]).unwrap();
        let en = notice_lines(&e.notices, Lang::En);
        assert_eq!(en[0], "bed_max_c raised from 80 C to 100 C");
        assert!(
            en[1].contains("above what the maker states") && en[1].contains("the owner's"),
            "{en:?}"
        );
        assert_eq!(en.len(), 2);
        let pt = notice_lines(&e.notices, Lang::Pt);
        assert!(
            pt[0].contains("aumentado") && pt[1].contains("proprietário"),
            "{pt:?}"
        );
        // a lowered limit has nothing to say, and a cleared verification has one line
        let mut v = a1.clone();
        v.verified = true;
        let cleared = v.edited(&[("bed_max_c", "60")]).unwrap();
        assert_eq!(notice_lines(&cleared.notices, Lang::En).len(), 1);
        assert!(notice_lines(&[], Lang::En).is_empty());
    }
}
