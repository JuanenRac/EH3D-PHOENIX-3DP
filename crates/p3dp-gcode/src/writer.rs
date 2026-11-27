//! The writer of G-code. One `Writer` is made for a dialect, the temperature limits of the model and its build volume; every call checks before it writes.

use p3dp_core::machine::{TempError, Zone};
use p3dp_core::{Dialect, TempLimits};
use p3dp_formats::gcode_file;
use std::fmt;

/// Why something was not written.
#[derive(Debug, Clone, PartialEq)]
pub enum GcodeError {
    /// A temperature that is not allowed.
    Temp(TempError),
    /// A number that is not allowed (not finite, outside the volume, a feed that makes no sense).
    Value(String),
    /// The dialect cannot do this.
    Dialect(String),
    /// The file could not be written (a line with a line break).
    File(String),
}

impl fmt::Display for GcodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GcodeError::Temp(t) => write!(f, "{t}"),
            GcodeError::Value(s) | GcodeError::Dialect(s) | GcodeError::File(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for GcodeError {}

impl From<TempError> for GcodeError {
    fn from(e: TempError) -> GcodeError {
        GcodeError::Temp(e)
    }
}

/// The macros that a Klipper printer must have in its configuration for the program to run (the writer does not define them).
pub fn required_macros(d: Dialect) -> &'static [&'static str] {
    match d {
        Dialect::Klipper => &["PRINT_START", "PRINT_END"],
        _ => &[],
    }
}

/// Whether the program has to be sent inside a package (the Bambu printers refuse a loose G-code file).
pub fn requires_package(d: Dialect) -> bool {
    d == Dialect::Bambu
}

/// What a job asks of the heaters at the start.
#[derive(Debug, Clone, PartialEq)]
pub struct JobParams {
    pub nozzle_c: f64,
    pub bed_c: f64,
    pub chamber_c: Option<f64>,
    /// The part fan after the first layers, 0 to 100.
    pub fan_pct: f64,
    /// For the progress of the Bambu printers (`M73`).
    pub estimated_minutes: Option<u32>,
}

/// The number the way G-code writes it: at most three decimals, no `-0`, no exponent.
pub fn num(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// A writer of G-code for one printer.
#[derive(Debug, Clone)]
pub struct Writer {
    dialect: Dialect,
    limits: TempLimits,
    volume_mm: [f64; 3],
    header: Vec<String>,
    lines: Vec<String>,
    /// The tool the program picked, when it picked one.
    tool: Option<u8>,
}

impl Writer {
    /// A writer. A resin printer takes layers and not G-code, so that dialect is refused; the volume must be finite and positive.
    pub fn new(
        dialect: Dialect,
        limits: TempLimits,
        volume_mm: [f64; 3],
    ) -> Result<Writer, GcodeError> {
        if dialect == Dialect::Msla {
            return Err(GcodeError::Dialect(
                "a resin printer takes layers, not G-code".to_string(),
            ));
        }
        if volume_mm.iter().any(|v| !v.is_finite() || *v <= 0.0) {
            return Err(GcodeError::Value(
                "the build volume must be finite and positive".to_string(),
            ));
        }
        Ok(Writer {
            dialect,
            limits,
            volume_mm,
            header: Vec::new(),
            lines: Vec::new(),
            tool: None,
        })
    }

    pub fn dialect(&self) -> Dialect {
        self.dialect
    }

    /// The lines written so far.
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    pub(crate) fn limits(&self) -> TempLimits {
        self.limits
    }

    pub(crate) fn volume_mm(&self) -> [f64; 3] {
        self.volume_mm
    }

    pub(crate) fn tool(&self) -> Option<u8> {
        self.tool
    }

    /// A line that was already checked (a block of the person).
    pub(crate) fn raw_line(&mut self, line: String) {
        self.lines.push(line);
    }

    fn push(&mut self, s: impl Into<String>) {
        self.lines.push(s.into());
    }

    /// The header comments: who made the file, with what version, for which dialect and printer.
    pub fn header(&mut self, generator: &str, version: &str, printer_id: &str) {
        self.header
            .push(format!("generator: {generator} {version}"));
        self.header.push(format!("dialect: {}", self.dialect));
        if !printer_id.is_empty() {
            self.header.push(format!("printer: {printer_id}"));
        }
        self.header.push(format!(
            "temperature limits: nozzle {} C, bed {} C",
            num(self.limits.nozzle_c),
            num(self.limits.bed_c)
        ));
        if requires_package(self.dialect) {
            self.header.push(
                "this program must be sent inside a package, not as a loose file".to_string(),
            );
        }
        for m in required_macros(self.dialect) {
            self.header
                .push(format!("needs the macro {m} in the printer configuration"));
        }
    }

    /// A comment line in the body.
    pub fn comment(&mut self, text: &str) -> Result<(), GcodeError> {
        if text.contains(['\n', '\r']) {
            return Err(GcodeError::Value(
                "a comment cannot hold a line break".to_string(),
            ));
        }
        self.push(format!("; {text}"));
        Ok(())
    }

    /// Sets (and, if asked, waits for) the temperature of the nozzle. RepRapFirmware keeps a temperature for each tool, so it is the one of the tool that was picked (the
    /// tool 0 before one is).
    pub fn set_nozzle(&mut self, celsius: f64, wait: bool) -> Result<(), GcodeError> {
        let t = self.limits.check(Zone::Nozzle, celsius)?;
        let line = match (self.dialect, wait) {
            (Dialect::Rrf, _) => {
                self.push(format!("G10 P{} S{}", self.tool.unwrap_or(0), num(t)));
                if wait && t > 0.0 {
                    self.push("M116");
                }
                return Ok(());
            }
            (_, false) => format!("M104 S{}", num(t)),
            (_, true) if t == 0.0 => "M104 S0".to_string(),
            (_, true) => format!("M109 S{}", num(t)),
        };
        self.push(line);
        Ok(())
    }

    /// Sets (and, if asked, waits for) the temperature of the bed.
    pub fn set_bed(&mut self, celsius: f64, wait: bool) -> Result<(), GcodeError> {
        let t = self.limits.check(Zone::Bed, celsius)?;
        if wait && t > 0.0 {
            self.push(format!("M190 S{}", num(t)));
        } else {
            self.push(format!("M140 S{}", num(t)));
        }
        Ok(())
    }

    /// Sets (and, if asked, waits for) the temperature of the chamber; a model with no heated chamber refuses anything above zero.
    pub fn set_chamber(&mut self, celsius: f64, wait: bool) -> Result<(), GcodeError> {
        let t = self.limits.check(Zone::Chamber, celsius)?;
        if self.limits.chamber_c.is_none() {
            return Ok(()); // zero on a printer with no chamber heater: nothing to write
        }
        if wait && t > 0.0 {
            self.push(format!("M191 S{}", num(t)));
        } else {
            self.push(format!("M141 S{}", num(t)));
        }
        Ok(())
    }

    /// The part fan, 0 to 100 per cent.
    pub fn set_fan(&mut self, percent: f64) -> Result<(), GcodeError> {
        if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
            return Err(GcodeError::Value(format!(
                "the fan speed {percent} is not between 0 and 100"
            )));
        }
        let s = (percent * 255.0 / 100.0).round() as u32;
        if s == 0 && self.dialect != Dialect::Rrf {
            self.push("M107");
        } else if self.dialect == Dialect::Bambu {
            self.push(format!("M106 P1 S{s}"));
        } else {
            self.push(format!("M106 S{s}"));
        }
        Ok(())
    }

    /// Picks a tool (a spool of a feeder or a head of a toolchanger): `T0` to `T15`. It sets no temperature: see [`Writer::change_tool`].
    pub fn select_tool(&mut self, tool: u8) -> Result<(), GcodeError> {
        if tool > 15 {
            return Err(GcodeError::Value(format!(
                "the tool {tool} is not from 0 to 15"
            )));
        }
        self.push(format!("T{tool}"));
        self.tool = Some(tool);
        Ok(())
    }

    /// Picks a tool at its temperature and waits for it. The temperature is checked against the limits before anything is written. Most firmwares are brought to the
    /// temperature and then given the tool; RepRapFirmware is given the temperature of that tool, then the tool, and then waited for.
    pub fn change_tool(&mut self, tool: u8, celsius: f64) -> Result<(), GcodeError> {
        let t = self.limits.check(Zone::Nozzle, celsius)?;
        if tool > 15 {
            return Err(GcodeError::Value(format!(
                "the tool {tool} is not from 0 to 15"
            )));
        }
        if self.dialect != Dialect::Rrf {
            self.set_nozzle(t, true)?;
            return self.select_tool(tool);
        }
        self.push(format!("G10 P{tool} S{}", num(t)));
        self.select_tool(tool)?;
        if t > 0.0 {
            self.push("M116");
        }
        Ok(())
    }

    /// The pressure advance (the extra push of the extruder that keeps the pressure in the nozzle in step with the speed), `0` to `2`: `0` is off. The command is the one of the
    /// firmware: `SET_PRESSURE_ADVANCE ADVANCE=` for Klipper, `M572 D0 S` for RepRapFirmware and `M900 K` for the others.
    pub fn set_pressure_advance(&mut self, advance: f64) -> Result<(), GcodeError> {
        if !advance.is_finite() || !(0.0..=2.0).contains(&advance) {
            return Err(GcodeError::Value(format!(
                "the pressure advance {advance} is not from 0 to 2"
            )));
        }
        let v = num(advance);
        match self.dialect {
            Dialect::Klipper => self.push(format!("SET_PRESSURE_ADVANCE ADVANCE={v}")),
            Dialect::Rrf => self.push(format!("M572 D0 S{v}")),
            _ => self.push(format!("M900 K{v}")),
        }
        Ok(())
    }

    /// How far the print is and how many minutes are left (`M73`), which RepRapFirmware does not take.
    pub fn progress(&mut self, percent: u32, minutes_left: u32) {
        if self.dialect != Dialect::Rrf {
            self.push(format!("M73 P{} R{minutes_left}", percent.min(100)));
        }
    }

    /// The number of layers of the print, which Klipper shows beside the layer it is on.
    pub fn total_layers(&mut self, layers: u32) {
        if self.dialect == Dialect::Klipper {
            self.push(format!("SET_PRINT_STATS_INFO TOTAL_LAYER={layers}"));
        }
    }

    /// A pause for a change of colour or an insert, in the command of the firmware: `M600` (Marlin), `PAUSE` (Klipper, a macro of the printer), `M226` (RepRapFirmware) and
    /// `M400 U1` (Bambu).
    pub fn pause(&mut self) {
        match self.dialect {
            Dialect::Klipper => {
                self.push("; needs the macro PAUSE in the printer configuration");
                self.push("PAUSE");
            }
            Dialect::Rrf => self.push("M226"),
            Dialect::Bambu => self.push("M400 U1"),
            _ => self.push("M600"),
        }
    }

    /// A move in a straight line. A coordinate outside the build volume, a number that is not finite or a feed that is not between 1 and 100000 mm/min is an error.
    pub fn linear_move(
        &mut self,
        x: Option<f64>,
        y: Option<f64>,
        z: Option<f64>,
        e: Option<f64>,
        feed: Option<f64>,
    ) -> Result<(), GcodeError> {
        let mut parts = vec!["G1".to_string()];
        for (axis, v, max) in [
            ("X", x, self.volume_mm[0]),
            ("Y", y, self.volume_mm[1]),
            ("Z", z, self.volume_mm[2]),
        ] {
            if let Some(v) = v {
                if !v.is_finite() || v < 0.0 || v > max {
                    return Err(GcodeError::Value(format!(
                        "{axis}{v} is outside the build volume (0 to {max})"
                    )));
                }
                parts.push(format!("{axis}{}", num(v)));
            }
        }
        if let Some(e) = e {
            if !e.is_finite() || e.abs() > 100_000.0 {
                return Err(GcodeError::Value(format!(
                    "E{e} is not a sensible extrusion"
                )));
            }
            parts.push(format!("E{}", num(e)));
        }
        if let Some(f) = feed {
            if !f.is_finite() || !(1.0..=100_000.0).contains(&f) {
                return Err(GcodeError::Value(format!(
                    "the feed {f} is not between 1 and 100000 mm/min"
                )));
            }
            parts.push(format!("F{}", num(f)));
        }
        if parts.len() == 1 {
            return Err(GcodeError::Value(
                "a move needs at least one axis, extrusion or feed".to_string(),
            ));
        }
        self.push(parts.join(" "));
        Ok(())
    }

    /// The mark of a new layer.
    pub fn layer(&mut self, n: u32, z_mm: f64) -> Result<(), GcodeError> {
        if !z_mm.is_finite() || z_mm < 0.0 || z_mm > self.volume_mm[2] {
            return Err(GcodeError::Value(format!(
                "the layer height {z_mm} is outside the build volume"
            )));
        }
        match self.dialect {
            Dialect::Klipper => {
                self.push(format!(";LAYER:{n}"));
                self.push(format!("SET_PRINT_STATS_INFO CURRENT_LAYER={}", n + 1));
            }
            Dialect::Bambu => {
                self.push(format!("; layer_num: {n}"));
                self.push(format!("; Z_HEIGHT: {}", num(z_mm)));
            }
            _ => self.push(format!(";LAYER:{n}")),
        }
        Ok(())
    }

    /// The start of a job. Every temperature is checked first: if one is refused nothing is written at all.
    pub fn preamble(&mut self, job: &JobParams) -> Result<(), GcodeError> {
        let nozzle = self.limits.check(Zone::Nozzle, job.nozzle_c)?;
        let bed = self.limits.check(Zone::Bed, job.bed_c)?;
        let chamber = match job.chamber_c {
            Some(c) => Some(self.limits.check(Zone::Chamber, c)?),
            None => None,
        };
        if !job.fan_pct.is_finite() || !(0.0..=100.0).contains(&job.fan_pct) {
            return Err(GcodeError::Value(format!(
                "the fan speed {} is not between 0 and 100",
                job.fan_pct
            )));
        }
        self.push("G21");
        self.push("G90");
        self.push("M83");
        if let (Dialect::Bambu, Some(min)) = (self.dialect, job.estimated_minutes) {
            self.push(format!("M73 P0 R{min}"));
        }
        if self.dialect == Dialect::Klipper {
            let mut l = format!("PRINT_START BED={} EXTRUDER={}", num(bed), num(nozzle));
            if let Some(c) = chamber.filter(|c| *c > 0.0) {
                l.push_str(&format!(" CHAMBER={}", num(c)));
            }
            self.push(l);
            // the macro is the printer's own and may leave another mode: absolute extrusion would read the lines that follow as positions of the filament
            for line in ["G21", "G90", "M83", "M107"] {
                self.push(line);
            }
            return Ok(());
        }
        self.set_bed(bed, false)?;
        if let Some(c) = chamber.filter(|c| *c > 0.0) {
            self.set_chamber(c, false)?;
        }
        self.set_nozzle(nozzle.min(150.0), false)?;
        self.push("G28");
        // RepRapFirmware heats and extrudes with the tool that is picked, and none is when a job starts
        if self.dialect == Dialect::Rrf {
            self.select_tool(0)?;
        }
        self.set_bed(bed, true)?;
        if let Some(c) = chamber.filter(|c| *c > 0.0) {
            self.set_chamber(c, true)?;
        }
        self.set_nozzle(nozzle, true)?;
        self.set_fan(0.0)?;
        Ok(())
    }

    /// The end of a job: retract a little, heaters off, fan off, motors off. It does not move the head against the volume, because it does not know where it is.
    pub fn end_safe(&mut self) -> Result<(), GcodeError> {
        if self.dialect == Dialect::Klipper {
            self.push("PRINT_END");
            return Ok(());
        }
        self.push("G91");
        self.push("G1 E-2 F1800");
        self.push("G90");
        self.set_nozzle(0.0, false)?;
        self.set_bed(0.0, false)?;
        self.set_chamber(0.0, false)?;
        self.set_fan(0.0)?;
        if self.dialect == Dialect::Rrf {
            self.push("M0");
        } else {
            self.push("M84");
        }
        Ok(())
    }

    /// The finished file: the header as comments and then the lines.
    pub fn finish(self) -> Result<String, GcodeError> {
        gcode_file::write(&self.header, &self.lines).map_err(|e| GcodeError::File(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const L: TempLimits = TempLimits {
        nozzle_c: 300.0,
        bed_c: 100.0,
        chamber_c: None,
    };
    const V: [f64; 3] = [256.0, 256.0, 256.0];

    fn job() -> JobParams {
        JobParams {
            nozzle_c: 215.0,
            bed_c: 60.0,
            chamber_c: None,
            fan_pct: 100.0,
            estimated_minutes: Some(95),
        }
    }

    fn writer(d: Dialect) -> Writer {
        Writer::new(d, L, V).unwrap()
    }

    #[test]
    fn a_tool_is_picked_and_the_pressure_advance_follows_the_firmware() {
        let mut w = writer(Dialect::Klipper);
        w.select_tool(2).unwrap();
        w.set_pressure_advance(0.045).unwrap();
        assert_eq!(w.lines(), ["T2", "SET_PRESSURE_ADVANCE ADVANCE=0.045"]);
        let mut w = writer(Dialect::Marlin);
        w.set_pressure_advance(0.5).unwrap();
        assert_eq!(w.lines(), ["M900 K0.5"]);
        let mut w = writer(Dialect::Rrf);
        w.set_pressure_advance(0.0).unwrap();
        assert_eq!(w.lines(), ["M572 D0 S0"]);
        for bad in [-0.1, 2.5, f64::NAN, f64::INFINITY] {
            assert!(w.set_pressure_advance(bad).is_err(), "{bad}");
        }
        assert!(w.select_tool(16).is_err());
        assert_eq!(w.lines().len(), 1, "a refusal writes nothing");
    }

    #[test]
    fn numbers_are_written_one_way() {
        assert_eq!(num(1.0), "1");
        assert_eq!(num(0.1 + 0.2), "0.3");
        assert_eq!(num(-0.0004), "0");
        assert_eq!(num(12.3456), "12.346");
        assert_eq!(num(250.0), "250");
    }

    #[test]
    fn marlin_start_and_end() {
        let mut w = writer(Dialect::Marlin);
        w.preamble(&job()).unwrap();
        w.end_safe().unwrap();
        let t = w.lines().join("\n");
        assert!(t.contains("M140 S60\nM104 S150\nG28\nM190 S60\nM109 S215"));
        assert!(
            t.contains("M104 S0")
                && t.contains("M140 S0")
                && t.contains("M107")
                && t.ends_with("M84")
        );
        assert!(!t.contains("PRINT_START"));
    }

    #[test]
    fn klipper_uses_the_macros_with_parameters() {
        let mut w = writer(Dialect::Klipper);
        w.preamble(&job()).unwrap();
        w.end_safe().unwrap();
        assert!(w
            .lines()
            .contains(&"PRINT_START BED=60 EXTRUDER=215".to_string()));
        assert_eq!(w.lines().last().unwrap(), "PRINT_END");
        assert_eq!(
            required_macros(Dialect::Klipper),
            &["PRINT_START", "PRINT_END"]
        );
        assert!(required_macros(Dialect::Marlin).is_empty());
    }

    #[test]
    fn rrf_uses_its_own_tool_commands() {
        let mut w = writer(Dialect::Rrf);
        w.preamble(&job()).unwrap();
        let t = w.lines().join("\n");
        assert!(t.contains("G10 P0 S150") && t.contains("G10 P0 S215\nM116"));
        assert!(
            t.contains("G28\nT0\n"),
            "a tool is picked before it is waited for"
        );
        w.end_safe().unwrap();
        assert!(w.lines().contains(&"G10 P0 S0".to_string()) && w.lines().last().unwrap() == "M0");
    }

    #[test]
    fn bambu_reports_progress_and_needs_a_package() {
        let mut w = writer(Dialect::Bambu);
        w.header("PHOENIX-3DP", "0.0.1", "bambu-lab-a1");
        w.preamble(&job()).unwrap();
        w.set_fan(50.0).unwrap();
        assert!(w.lines().contains(&"M73 P0 R95".to_string()));
        assert!(w.lines().contains(&"M106 P1 S128".to_string()));
        assert!(requires_package(Dialect::Bambu) && !requires_package(Dialect::Marlin));
        let out = w.finish().unwrap();
        assert!(out.contains("; this program must be sent inside a package"));
    }

    #[test]
    fn a_temperature_over_the_limit_writes_nothing() {
        for d in [
            Dialect::Marlin,
            Dialect::Klipper,
            Dialect::Rrf,
            Dialect::Bambu,
        ] {
            let mut w = writer(d);
            let mut j = job();
            j.nozzle_c = 300.5;
            assert!(
                matches!(
                    w.preamble(&j),
                    Err(GcodeError::Temp(TempError::Above { .. }))
                ),
                "{d}"
            );
            assert!(w.lines().is_empty(), "{d}: wrote something before refusing");
            j.nozzle_c = 215.0;
            j.bed_c = 100.5;
            assert!(w.preamble(&j).is_err());
            assert!(w.lines().is_empty());
            assert!(w.set_nozzle(300.0, true).is_ok());
            assert!(w.set_nozzle(301.0, true).is_err());
            assert!(w.set_bed(101.0, false).is_err());
        }
    }

    #[test]
    fn nonsense_temperatures_are_refused() {
        let mut w = writer(Dialect::Marlin);
        assert!(w.set_nozzle(f64::NAN, false).is_err());
        assert!(w.set_nozzle(f64::INFINITY, true).is_err());
        assert!(w.set_bed(-5.0, false).is_err());
        assert!(w.lines().is_empty());
    }

    #[test]
    fn a_chamber_needs_a_heated_chamber() {
        let mut w = writer(Dialect::Marlin);
        let mut j = job();
        j.chamber_c = Some(40.0);
        assert!(matches!(
            w.preamble(&j),
            Err(GcodeError::Temp(TempError::NoChamber(_)))
        ));
        assert!(w.set_chamber(0.0, false).is_ok());
        assert!(w.lines().is_empty());
        let l = TempLimits {
            chamber_c: Some(60.0),
            ..L
        };
        let mut w = Writer::new(Dialect::Marlin, l, V).unwrap();
        j.chamber_c = Some(45.0);
        w.preamble(&j).unwrap();
        assert!(
            w.lines().contains(&"M141 S45".to_string())
                && w.lines().contains(&"M191 S45".to_string())
        );
        assert!(w.set_chamber(61.0, false).is_err());
    }

    #[test]
    fn moves_stay_in_the_volume() {
        let mut w = writer(Dialect::Marlin);
        w.linear_move(Some(10.0), Some(20.5), Some(0.2), Some(1.5), Some(1800.0))
            .unwrap();
        assert_eq!(w.lines()[0], "G1 X10 Y20.5 Z0.2 E1.5 F1800");
        for (x, y, z, e, f) in [
            (Some(257.0), None, None, None, None),
            (None, Some(-1.0), None, None, None),
            (None, None, Some(f64::NAN), None, None),
            (None, None, None, Some(f64::INFINITY), None),
            (None, None, None, None, Some(0.0)),
            (None, None, None, None, Some(1e9)),
            (None, None, None, None, None),
        ] {
            assert!(w.linear_move(x, y, z, e, f).is_err());
        }
        assert_eq!(w.lines().len(), 1);
    }

    #[test]
    fn fans_layers_and_comments() {
        let mut w = writer(Dialect::Klipper);
        w.set_fan(0.0).unwrap();
        w.set_fan(100.0).unwrap();
        assert_eq!(w.lines(), &["M107".to_string(), "M106 S255".to_string()]);
        assert!(w.set_fan(101.0).is_err() && w.set_fan(f64::NAN).is_err());
        w.layer(0, 0.2).unwrap();
        assert!(w
            .lines()
            .contains(&"SET_PRINT_STATS_INFO CURRENT_LAYER=1".to_string()));
        assert!(w.layer(1, 999.0).is_err());
        assert!(w.comment("a\nM104 S999").is_err());
        w.comment("fine").unwrap();
    }

    #[test]
    fn resin_printers_and_bad_volumes_are_refused() {
        assert!(matches!(
            Writer::new(Dialect::Msla, L, V),
            Err(GcodeError::Dialect(_))
        ));
        assert!(Writer::new(Dialect::Marlin, L, [0.0, 1.0, 1.0]).is_err());
        assert!(Writer::new(Dialect::Marlin, L, [f64::NAN, 1.0, 1.0]).is_err());
    }

    #[test]
    fn the_finished_file_has_its_header() {
        let mut w = writer(Dialect::Marlin);
        w.header("PHOENIX-3DP", "0.0.1", "test");
        w.preamble(&job()).unwrap();
        let t = w.finish().unwrap();
        assert!(t.starts_with("; generator: PHOENIX-3DP 0.0.1\n; dialect: marlin\n; printer: test\n; temperature limits: nozzle 300 C, bed 100 C\n\nG21\n"));
    }
}
