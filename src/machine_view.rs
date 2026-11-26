//! The printers of the catalog of the suite as text: the list, everything one record says with its pages, what to start from and the settings of a starting process. Nothing here
//! has been tried on a machine and nothing is sent to one.

use crate::i18n::Lang;
use eh3d_printers::{
    starting_filaments, Label, Level, MulticolorStatus, Printer, Process, Processes,
};
use eh3d_printmaterials::Database;
use p3dp_slicer::SliceConfig;

/// The language of the crates of the suite for a language of the program.
pub fn suite_lang(lang: Lang) -> eh3d_printers::Lang {
    eh3d_printers::Lang::from_code(lang.code()).unwrap_or(eh3d_printers::Lang::En)
}

fn process_words(lang: Lang) -> [&'static str; 4] {
    match lang {
        Lang::En => ["layer", "line", "walls", "infill"],
        Lang::Es => ["capa", "línea", "paredes", "relleno"],
        Lang::De => ["Schicht", "Linie", "Wände", "Füllung"],
        Lang::Fr => ["couche", "ligne", "parois", "remplissage"],
        Lang::It => ["strato", "linea", "pareti", "riempimento"],
        Lang::Ja => ["積層", "線幅", "壁", "充填"],
        Lang::Zh => ["层高", "线宽", "壁数", "填充"],
        Lang::Pt => ["camada", "linha", "paredes", "enchimento"],
    }
}

/// One line for each printer of the catalog of the suite.
pub fn list_lines(printers: &[&Printer], lang: Lang) -> Vec<String> {
    let l = suite_lang(lang);
    let mut out: Vec<String> = printers
        .iter()
        .map(|p| {
            format!(
                "{:<34} {:<34} {}x{}x{} mm  {}",
                p.id,
                p.title(),
                p.volume_mm[0],
                p.volume_mm[1],
                p.volume_mm[2],
                p.adapter.title_in(l)
            )
        })
        .collect();
    out.push(Label::NotTried.title_in(l).to_string());
    out
}

fn numbers(values: &[f64]) -> String {
    values
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// The nozzle to work with: the one asked for, which the maker has to name, or the one that comes fitted.
pub fn nozzle_of(printer: &Printer, asked: Option<f64>) -> Result<f64, String> {
    match asked {
        Some(n) if printer.nozzles_mm.iter().any(|x| (x - n).abs() < 1e-9) => Ok(n),
        Some(n) => Err(format!(
            "{}: the maker does not name a {n} mm nozzle (it names {})",
            printer.id,
            numbers(&printer.nozzles_mm)
        )),
        None => printer
            .nozzle_default_mm
            .or_else(|| printer.nozzles_mm.first().copied())
            .ok_or_else(|| format!("{}: no nozzle is named: give --nozzle", printer.id)),
    }
}

/// Everything the record of a printer says, and what to start from with a nozzle.
pub fn show_lines(printer: &Printer, nozzle_mm: f64, lang: Lang) -> Result<Vec<String>, String> {
    let l = suite_lang(lang);
    let unknown = Label::NotPublished.title_in(l);
    let yes_no = |v: Option<bool>| match v {
        Some(true) => Label::Yes.title_in(l),
        Some(false) => Label::No.title_in(l),
        None => unknown,
    };
    let row = |label: Label, value: String| format!("{}: {value}", label.title_in(l));
    let mut out = vec![
        format!("{} ({})", printer.title(), printer.id),
        Label::NotTried.title_in(l).to_string(),
        row(
            Label::Volume,
            format!(
                "{} x {} x {} mm",
                printer.volume_mm[0], printer.volume_mm[1], printer.volume_mm[2]
            ),
        ),
        row(
            Label::Nozzles,
            if printer.nozzles_mm.is_empty() {
                unknown.to_string()
            } else {
                format!("{} mm", numbers(&printer.nozzles_mm))
            },
        ),
        row(
            Label::Filament,
            printer
                .filament_diameter_mm
                .map_or_else(|| unknown.to_string(), |d| format!("{d} mm")),
        ),
        row(Label::Enclosure, printer.enclosure.title_in(l).to_string()),
        row(
            Label::HeatedChamber,
            yes_no(printer.heated_chamber).to_string(),
        ),
        row(Label::NozzleMax, format!("{} C", printer.nozzle_max_c)),
        row(Label::BedMax, format!("{} C", printer.bed_max_c)),
    ];
    let motion = [
        (Label::MaxSpeed, printer.max_speed_mm_s, "mm/s"),
        (
            Label::MaxAcceleration,
            printer.max_acceleration_mm_s2,
            "mm/s2",
        ),
        (Label::MaxFlow, printer.max_flow_mm3_s, "mm3/s"),
        (Label::DefaultHeight, printer.default_height_mm, "mm"),
    ];
    for (label, value, unit) in motion {
        out.push(row(
            label,
            value.map_or_else(|| unknown.to_string(), |v| format!("{v} {unit}")),
        ));
    }
    let m = &printer.multicolor;
    let mut multicolor = m.status.title_in(l).to_string();
    if matches!(
        m.status,
        MulticolorStatus::Included | MulticolorStatus::Optional
    ) {
        if let Some(system) = &m.system {
            multicolor.push_str(&format!(" - {system}"));
        }
        if let Some(n) = m.max_colors {
            multicolor.push_str(&format!(" ({n})"));
        }
    }
    out.push(row(Label::Multicolor, multicolor));
    out.push(row(
        Label::Firmware,
        printer.firmware.title_in(l).to_string(),
    ));
    let mut adapter = printer.adapter.title_in(l).to_string();
    if !printer.adapter_documented {
        adapter.push_str(&format!(" - {}", Label::AdapterUnchecked.title_in(l)));
    }
    out.push(row(Label::Adapter, adapter));
    out.push(row(
        Label::SendFormats,
        if printer.send_formats.is_empty() {
            unknown.to_string()
        } else {
            printer
                .send_formats
                .iter()
                .map(|f| f.extension())
                .collect::<Vec<_>>()
                .join(", ")
        },
    ));
    out.push(format!("{}:", Label::Sources.title_in(l)));
    for s in &printer.sources {
        out.push(format!("  {}  ({})", s.url, s.covers.join(", ")));
    }
    let unchecked = printer.unconfirmed();
    if !unchecked.is_empty() {
        out.push(row(Label::ToCheck, unchecked.join(", ")));
    }
    let database = Database::embedded().map_err(|errors| {
        errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    out.push(format!("{}:", Label::StartingFilaments.title_in(l)));
    for s in starting_filaments(printer, &database, nozzle_mm) {
        let mut words: Vec<&str> = s.blocks.iter().map(|b| b.title_in(l)).collect();
        words.extend(s.cautions.iter().map(|c| c.title_in(l)));
        let mark = if s.usable() { "  " } else { "x " };
        let tail = if words.is_empty() {
            String::new()
        } else {
            format!("  - {}", words.join("; "))
        };
        out.push(format!("{mark}{}{tail}", s.filament.id));
    }
    out.push(format!("{}:", Label::StartingProcesses.title_in(l)));
    let rules = Processes::embedded().map_err(|e| e.to_string())?;
    let words = process_words(lang);
    for level in Level::ALL {
        let p = rules
            .for_nozzle(level, nozzle_mm)
            .map_err(|e| e.to_string())?;
        out.push(format!(
            "  {} ({}): {} {} mm, {} {} mm, {} {}, {} {} %",
            level.as_str(),
            level.title_in(l),
            words[0],
            p.layer_mm,
            words[1],
            p.line_width_mm,
            words[2],
            p.walls,
            words[3],
            p.infill_pct
        ));
    }
    Ok(out)
}

/// Puts the settings of the starting process of a level over the ones given, for a printer and a nozzle. All of them or none: a process that does not hold together with the
/// settings changes nothing.
pub fn apply_process(
    config: &mut SliceConfig,
    level: Level,
    printer_id: &str,
    nozzle_mm: f64,
) -> Result<(), String> {
    let process = Processes::embedded()
        .and_then(|r| r.for_nozzle(level, nozzle_mm))
        .map_err(|e| e.to_string())?;
    let filament_mm = p3dp_profiles::suite_machines()
        .map_err(|e| e.to_string())?
        .find(printer_id)
        .and_then(|m| m.filament_diameter_mm);
    let mut next = config.clone();
    for (key, value) in settings_of(&process, filament_mm) {
        next.set(key, &value).map_err(|e| e.to_string())?;
    }
    next.check().map_err(|e| e.to_string())?;
    *config = next;
    Ok(())
}

pub(crate) fn settings_of(
    p: &Process,
    filament_diameter_mm: Option<f64>,
) -> Vec<(&'static str, String)> {
    let mut out = vec![
        ("nozzle_diameter_mm", p.nozzle_mm.to_string()),
        ("layer_height_mm", p.layer_mm.to_string()),
        ("first_layer_height_mm", p.first_layer_mm.to_string()),
        ("line_width_mm", p.line_width_mm.to_string()),
        ("perimeters", p.walls.to_string()),
        ("top_layers", p.top_layers.to_string()),
        ("bottom_layers", p.bottom_layers.to_string()),
        ("infill_percent", p.infill_pct.to_string()),
    ];
    if let Some(d) = filament_diameter_mm {
        out.push(("filament_diameter_mm", d.to_string()));
    }
    out
}

/// The settings file of a starting process of a printer, checked by the slicer before it is given.
pub fn process_settings(printer: &Printer, level: Level, nozzle_mm: f64) -> Result<String, String> {
    let process = Processes::embedded()
        .and_then(|r| r.for_nozzle(level, nozzle_mm))
        .map_err(|e| e.to_string())?;
    let mut text = format!(
        "# starting process {} for {}, nozzle {nozzle_mm} mm; not tried on a machine\n",
        level.as_str(),
        printer.id
    );
    for (key, value) in settings_of(&process, printer.filament_diameter_mm) {
        text.push_str(&format!("{key} = {value}\n"));
    }
    SliceConfig::from_text(&text).map_err(|e| e.to_string())?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_profiles::suite_machines;

    #[test]
    fn every_starting_process_of_every_printer_is_accepted_by_the_slicer() {
        let machines = suite_machines().unwrap();
        for p in machines.printers() {
            for nozzle in &p.nozzles_mm {
                for level in Level::ALL {
                    let text = process_settings(p, level, *nozzle).unwrap();
                    let c = SliceConfig::from_text(&text).unwrap();
                    assert_eq!(c.nozzle_diameter_mm, *nozzle, "{}", p.id);
                }
            }
        }
    }

    #[test]
    fn a_starting_process_is_put_over_the_settings_whole_or_not_at_all() {
        let mut config = SliceConfig::default();
        apply_process(&mut config, Level::Draft, "bambu-lab-a1", 0.4).unwrap();
        let draft = config.layer_height_mm;
        apply_process(&mut config, Level::Fine, "bambu-lab-a1", 0.4).unwrap();
        assert!(config.layer_height_mm < draft);
        assert_eq!(config.nozzle_diameter_mm, 0.4);
        let before = config.clone();
        assert!(apply_process(&mut config, Level::Fine, "bambu-lab-a1", f64::NAN).is_err());
        assert_eq!(config, before);
    }

    #[test]
    fn a_nozzle_the_maker_does_not_name_is_refused() {
        let machines = suite_machines().unwrap();
        let a1 = machines.find("bambu-lab-a1").unwrap();
        assert_eq!(nozzle_of(a1, None).unwrap(), 0.4);
        assert_eq!(nozzle_of(a1, Some(0.6)).unwrap(), 0.6);
        for bad in [0.5, 0.0, f64::NAN, f64::INFINITY] {
            assert!(nozzle_of(a1, Some(bad)).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_record_is_shown_with_its_pages_in_every_language() {
        let machines = suite_machines().unwrap();
        for p in machines.printers() {
            for lang in Lang::ALL {
                let text = show_lines(p, 0.4, lang).unwrap().join("\n");
                for s in &p.sources {
                    assert!(text.contains(&s.url), "{}: {}", p.id, s.url);
                }
                assert!(text.contains("generic/generic-pla"));
                assert!(text.contains(Label::NotTried.title_in(suite_lang(lang))));
            }
        }
        let ke = machines.find("creality-ender-3-v3-ke").unwrap();
        let text = show_lines(ke, 0.4, Lang::En).unwrap().join("\n");
        assert!(text.contains("The maker does not document it"));
        assert!(text.contains("Files it takes: Not published"));
        let all: Vec<&Printer> = machines.printers().iter().collect();
        assert_eq!(list_lines(&all, Lang::Es).len(), all.len() + 1);
    }
}
