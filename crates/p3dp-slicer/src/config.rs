//! The settings of a slice. Every value has a range and a conservative default, is read from a `key = value` file like the other settings of the program, and a value outside its
//! range is an error that names the key: nothing is lowered or raised to fit. Whatever was tuned lives here and in the file, not in the code.

use crate::error::SliceError;
use p3dp_core::kv;
use p3dp_core::CoreError;

/// What to do with a part of a layer that is thinner than the perimeters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThinRegions {
    /// Leave it out and say so in the warnings of the plan (the default: the points of a cone are never printed with perimeters).
    Skip,
    /// Refuse the slice.
    Refuse,
}

/// Where the model is put on the bed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// Its box centred on the bed, its lowest point on the bed (the default).
    Center,
    /// Where the model is: its x and y are the coordinates of the bed, and only its lowest point is moved to the bed.
    Origin,
}

/// The settings.
#[derive(Debug, Clone, PartialEq)]
pub struct SliceConfig {
    pub layer_height_mm: f64,
    pub first_layer_height_mm: f64,
    pub nozzle_diameter_mm: f64,
    pub line_width_mm: f64,
    pub perimeters: u32,
    pub top_layers: u32,
    pub bottom_layers: u32,
    /// 0 to 100.
    pub infill_percent: f64,
    pub filament_diameter_mm: f64,
    pub extrusion_multiplier: f64,
    pub nozzle_c: f64,
    pub bed_c: f64,
    /// 0 is off.
    pub chamber_c: f64,
    pub fan_percent: f64,
    /// The layer (counted from 0) the part fan starts at.
    pub fan_from_layer: u32,
    pub first_layer_speed_mm_s: f64,
    pub perimeter_speed_mm_s: f64,
    pub external_perimeter_speed_mm_s: f64,
    pub infill_speed_mm_s: f64,
    pub travel_speed_mm_s: f64,
    /// How fast the bed or the head moves in z.
    pub z_speed_mm_s: f64,
    /// No speed of the slice may be above this one.
    pub max_speed_mm_s: f64,
    pub retract_mm: f64,
    pub retract_speed_mm_s: f64,
    /// A travel shorter than this is made without retracting.
    pub retract_min_travel_mm: f64,
    /// How far the head is lifted above the print when it is done (never above the printer).
    pub end_lift_mm: f64,
    pub placement: Placement,
    /// The clearance kept from every edge of the bed.
    pub bed_margin_mm: f64,
    pub thin_regions: ThinRegions,
    pub max_layers: u32,
    /// The most points in the cut of one layer.
    pub max_points_per_layer: u32,
    pub max_gcode_lines: u32,
}

impl Default for SliceConfig {
    fn default() -> SliceConfig {
        SliceConfig {
            layer_height_mm: 0.2,
            first_layer_height_mm: 0.2,
            nozzle_diameter_mm: 0.4,
            line_width_mm: 0.45,
            perimeters: 2,
            top_layers: 4,
            bottom_layers: 4,
            infill_percent: 20.0,
            filament_diameter_mm: 1.75,
            extrusion_multiplier: 1.0,
            nozzle_c: 210.0,
            bed_c: 60.0,
            chamber_c: 0.0,
            fan_percent: 100.0,
            fan_from_layer: 3,
            first_layer_speed_mm_s: 20.0,
            perimeter_speed_mm_s: 40.0,
            external_perimeter_speed_mm_s: 30.0,
            infill_speed_mm_s: 60.0,
            travel_speed_mm_s: 150.0,
            z_speed_mm_s: 10.0,
            max_speed_mm_s: 200.0,
            retract_mm: 1.0,
            retract_speed_mm_s: 30.0,
            retract_min_travel_mm: 2.0,
            end_lift_mm: 10.0,
            placement: Placement::Center,
            bed_margin_mm: 0.0,
            thin_regions: ThinRegions::Skip,
            max_layers: 5000,
            max_points_per_layer: 200_000,
            max_gcode_lines: 5_000_000,
        }
    }
}

/// The keys, in the order of the file.
pub const KEYS: [&str; 32] = [
    "layer_height_mm",
    "first_layer_height_mm",
    "nozzle_diameter_mm",
    "line_width_mm",
    "perimeters",
    "top_layers",
    "bottom_layers",
    "infill_percent",
    "filament_diameter_mm",
    "extrusion_multiplier",
    "nozzle_c",
    "bed_c",
    "chamber_c",
    "fan_percent",
    "fan_from_layer",
    "first_layer_speed_mm_s",
    "perimeter_speed_mm_s",
    "external_perimeter_speed_mm_s",
    "infill_speed_mm_s",
    "travel_speed_mm_s",
    "z_speed_mm_s",
    "max_speed_mm_s",
    "retract_mm",
    "retract_speed_mm_s",
    "retract_min_travel_mm",
    "end_lift_mm",
    "placement",
    "bed_margin_mm",
    "thin_regions",
    "max_layers",
    "max_points_per_layer",
    "max_gcode_lines",
];

fn invalid(msg: String) -> CoreError {
    CoreError::Invalid(msg)
}

impl SliceConfig {
    /// Sets one key. An unknown key is an error (all the keys of the file are the slicer's); a value out of its range is an error that names the key and changes nothing.
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), CoreError> {
        let f = |lo: f64, hi: f64| kv::float_in(key, value, lo, hi);
        let n = |lo: i64, hi: i64| kv::int_in(key, value, lo, hi);
        match key {
            "layer_height_mm" => self.layer_height_mm = f(0.05, 1.0)?,
            "first_layer_height_mm" => self.first_layer_height_mm = f(0.05, 1.0)?,
            "nozzle_diameter_mm" => self.nozzle_diameter_mm = f(0.1, 2.0)?,
            "line_width_mm" => self.line_width_mm = f(0.1, 2.5)?,
            "perimeters" => self.perimeters = n(1, 10)? as u32,
            "top_layers" => self.top_layers = n(0, 50)? as u32,
            "bottom_layers" => self.bottom_layers = n(0, 50)? as u32,
            "infill_percent" => self.infill_percent = f(0.0, 100.0)?,
            "filament_diameter_mm" => self.filament_diameter_mm = f(1.0, 3.5)?,
            "extrusion_multiplier" => self.extrusion_multiplier = f(0.5, 1.5)?,
            "nozzle_c" => self.nozzle_c = f(0.0, 500.0)?,
            "bed_c" => self.bed_c = f(0.0, 200.0)?,
            "chamber_c" => self.chamber_c = f(0.0, 120.0)?,
            "fan_percent" => self.fan_percent = f(0.0, 100.0)?,
            "fan_from_layer" => self.fan_from_layer = n(0, 100_000)? as u32,
            "first_layer_speed_mm_s" => self.first_layer_speed_mm_s = f(1.0, 1000.0)?,
            "perimeter_speed_mm_s" => self.perimeter_speed_mm_s = f(1.0, 1000.0)?,
            "external_perimeter_speed_mm_s" => self.external_perimeter_speed_mm_s = f(1.0, 1000.0)?,
            "infill_speed_mm_s" => self.infill_speed_mm_s = f(1.0, 1000.0)?,
            "travel_speed_mm_s" => self.travel_speed_mm_s = f(1.0, 1000.0)?,
            "z_speed_mm_s" => self.z_speed_mm_s = f(0.1, 1000.0)?,
            "max_speed_mm_s" => self.max_speed_mm_s = f(1.0, 1000.0)?,
            "retract_mm" => self.retract_mm = f(0.0, 20.0)?,
            "retract_speed_mm_s" => self.retract_speed_mm_s = f(1.0, 200.0)?,
            "retract_min_travel_mm" => self.retract_min_travel_mm = f(0.0, 100.0)?,
            "end_lift_mm" => self.end_lift_mm = f(0.0, 100.0)?,
            "placement" => {
                self.placement = match value.trim().to_ascii_lowercase().as_str() {
                    "center" | "centre" => Placement::Center,
                    "origin" => Placement::Origin,
                    _ => return Err(invalid(format!("{key}: expected center or origin"))),
                }
            }
            "bed_margin_mm" => self.bed_margin_mm = f(0.0, 100.0)?,
            "thin_regions" => {
                self.thin_regions = match value.trim().to_ascii_lowercase().as_str() {
                    "skip" => ThinRegions::Skip,
                    "refuse" => ThinRegions::Refuse,
                    _ => return Err(invalid(format!("{key}: expected skip or refuse"))),
                }
            }
            "max_layers" => self.max_layers = n(1, 100_000)? as u32,
            "max_points_per_layer" => self.max_points_per_layer = n(100, 10_000_000)? as u32,
            "max_gcode_lines" => self.max_gcode_lines = n(1_000, 100_000_000)? as u32,
            other => return Err(invalid(format!("{other}: not a setting of the slicer"))),
        }
        Ok(())
    }

    /// Reads a file of `key = value` lines; the error names the line.
    pub fn from_text(text: &str) -> Result<SliceConfig, CoreError> {
        let mut c = SliceConfig::default();
        for e in kv::parse(text)? {
            c.set(&e.key, &e.value)
                .map_err(|err| invalid(format!("line {}: {err}", e.line)))?;
        }
        c.check().map_err(|e| invalid(e.to_string()))?;
        Ok(c)
    }

    /// The settings as text, every key with its value (`from_text` reads it back).
    pub fn to_text(&self) -> String {
        let placement = match self.placement {
            Placement::Center => "center",
            Placement::Origin => "origin",
        };
        let thin = match self.thin_regions {
            ThinRegions::Skip => "skip",
            ThinRegions::Refuse => "refuse",
        };
        format!(
            "layer_height_mm = {}\nfirst_layer_height_mm = {}\nnozzle_diameter_mm = {}\nline_width_mm = {}\nperimeters = {}\ntop_layers = {}\nbottom_layers = {}\ninfill_percent = {}\n\
             filament_diameter_mm = {}\nextrusion_multiplier = {}\nnozzle_c = {}\nbed_c = {}\nchamber_c = {}\nfan_percent = {}\nfan_from_layer = {}\n\
             first_layer_speed_mm_s = {}\nperimeter_speed_mm_s = {}\nexternal_perimeter_speed_mm_s = {}\ninfill_speed_mm_s = {}\ntravel_speed_mm_s = {}\nz_speed_mm_s = {}\nmax_speed_mm_s = {}\n\
             retract_mm = {}\nretract_speed_mm_s = {}\nretract_min_travel_mm = {}\nend_lift_mm = {}\nplacement = {}\nbed_margin_mm = {}\nthin_regions = {}\nmax_layers = {}\nmax_points_per_layer = {}\nmax_gcode_lines = {}\n",
            self.layer_height_mm,
            self.first_layer_height_mm,
            self.nozzle_diameter_mm,
            self.line_width_mm,
            self.perimeters,
            self.top_layers,
            self.bottom_layers,
            self.infill_percent,
            self.filament_diameter_mm,
            self.extrusion_multiplier,
            self.nozzle_c,
            self.bed_c,
            self.chamber_c,
            self.fan_percent,
            self.fan_from_layer,
            self.first_layer_speed_mm_s,
            self.perimeter_speed_mm_s,
            self.external_perimeter_speed_mm_s,
            self.infill_speed_mm_s,
            self.travel_speed_mm_s,
            self.z_speed_mm_s,
            self.max_speed_mm_s,
            self.retract_mm,
            self.retract_speed_mm_s,
            self.retract_min_travel_mm,
            self.end_lift_mm,
            placement,
            self.bed_margin_mm,
            thin,
            self.max_layers,
            self.max_points_per_layer,
            self.max_gcode_lines
        )
    }

    /// Checks the settings against each other. Nothing is changed.
    pub fn check(&self) -> Result<(), SliceError> {
        let bad = |s: String| Err(SliceError::Config(s));
        let max_h = self.nozzle_diameter_mm * 0.8;
        for (name, h) in [
            ("layer_height_mm", self.layer_height_mm),
            ("first_layer_height_mm", self.first_layer_height_mm),
        ] {
            if h > max_h + 1e-9 {
                return bad(format!(
                    "{name} {h} is above 80% of the nozzle ({max_h:.3}): the plastic would not be pressed into the layer below"
                ));
            }
        }
        if self.line_width_mm < self.nozzle_diameter_mm * 0.8 - 1e-9
            || self.line_width_mm > self.nozzle_diameter_mm * 2.0 + 1e-9
        {
            return bad(format!(
                "line_width_mm {} must be from 80% to 200% of the nozzle ({})",
                self.line_width_mm, self.nozzle_diameter_mm
            ));
        }
        if self.line_width_mm < self.layer_height_mm
            || self.line_width_mm < self.first_layer_height_mm
        {
            return bad("line_width_mm must not be smaller than a layer height".to_string());
        }
        for (name, v) in [
            ("first_layer_speed_mm_s", self.first_layer_speed_mm_s),
            ("perimeter_speed_mm_s", self.perimeter_speed_mm_s),
            (
                "external_perimeter_speed_mm_s",
                self.external_perimeter_speed_mm_s,
            ),
            ("infill_speed_mm_s", self.infill_speed_mm_s),
            ("travel_speed_mm_s", self.travel_speed_mm_s),
            ("z_speed_mm_s", self.z_speed_mm_s),
            ("retract_speed_mm_s", self.retract_speed_mm_s),
        ] {
            if v > self.max_speed_mm_s {
                return bad(format!(
                    "{name} {v} is above max_speed_mm_s {}",
                    self.max_speed_mm_s
                ));
            }
        }
        Ok(())
    }

    /// The cross-section of a line of a layer of this height, in square millimetres: a rectangle with rounded ends, as the plastic lies when it is pressed.
    pub fn line_area_mm2(&self, layer_height: f64) -> f64 {
        let w = self.line_width_mm;
        let h = layer_height;
        h * (w - h) + std::f64::consts::PI * h * h / 4.0
    }

    /// The distance between the centres of two lines that touch, for a layer of this height.
    pub fn spacing_mm(&self, layer_height: f64) -> f64 {
        self.line_area_mm2(layer_height) / layer_height
    }

    /// The area of the section of the filament, in square millimetres.
    pub fn filament_area_mm2(&self) -> f64 {
        std::f64::consts::PI * (self.filament_diameter_mm / 2.0).powi(2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_valid_and_the_text_reads_back() {
        let c = SliceConfig::default();
        assert!(c.check().is_ok());
        let back = SliceConfig::from_text(&c.to_text()).unwrap();
        assert_eq!(back, c);
        let again = SliceConfig::from_text(&back.to_text()).unwrap();
        assert_eq!(again, c);
    }

    #[test]
    fn every_key_of_the_list_is_a_key_and_the_file_writes_all_of_them() {
        let mut c = SliceConfig::default();
        let text = c.to_text();
        let written: Vec<&str> = text
            .lines()
            .map(|l| l.split(" = ").next().unwrap())
            .collect();
        for k in KEYS {
            assert!(written.contains(&k), "{k} is not written");
        }
        assert_eq!(written.len(), KEYS.len());
        assert!(c.set("nope", "1").is_err());
    }

    #[test]
    fn a_value_out_of_its_range_is_an_error_that_changes_nothing() {
        let mut c = SliceConfig::default();
        for (k, v) in [
            ("layer_height_mm", "0.01"),
            ("layer_height_mm", "abc"),
            ("layer_height_mm", "nan"),
            ("perimeters", "0"),
            ("perimeters", "1.5"),
            ("infill_percent", "101"),
            ("infill_percent", "-1"),
            ("nozzle_c", "501"),
            ("fan_percent", "100.1"),
            ("placement", "left"),
            ("thin_regions", "maybe"),
            ("max_layers", "0"),
        ] {
            assert!(c.set(k, v).is_err(), "{k}={v}");
        }
        assert_eq!(c, SliceConfig::default());
    }

    #[test]
    fn settings_that_do_not_go_together_are_refused_not_adjusted() {
        let mut c = SliceConfig::default();
        c.set("layer_height_mm", "0.4").unwrap();
        assert!(
            c.check().is_err(),
            "0.4 mm layers are too tall for a 0.4 mm nozzle"
        );
        let mut c = SliceConfig::default();
        c.set("line_width_mm", "0.2").unwrap();
        assert!(c.check().is_err());
        let mut c = SliceConfig::default();
        c.set("infill_speed_mm_s", "300").unwrap();
        assert!(c.check().is_err(), "above max_speed_mm_s");
        c.set("max_speed_mm_s", "300").unwrap();
        assert!(c.check().is_ok());
    }

    #[test]
    fn a_file_names_the_line_that_is_wrong() {
        let e = SliceConfig::from_text("perimeters = 3\ninfill_percent = 500\n").unwrap_err();
        assert!(e.to_string().contains("line 2"), "{e}");
        let e = SliceConfig::from_text("colour = red\n").unwrap_err();
        assert!(e.to_string().contains("colour"));
        let ok =
            SliceConfig::from_text("# a comment\nperimeters = 3 # three\nplacement = origin\n")
                .unwrap();
        assert_eq!((ok.perimeters, ok.placement), (3, Placement::Origin));
    }

    #[test]
    fn the_area_of_a_line_and_its_spacing_follow_the_shape_of_the_plastic() {
        let c = SliceConfig::default();
        let a = c.line_area_mm2(0.2);
        assert!((a - (0.2 * 0.25 + std::f64::consts::PI * 0.01)).abs() < 1e-12);
        let s = c.spacing_mm(0.2);
        assert!(s < c.line_width_mm && s > c.line_width_mm - 0.2);
        assert!((c.filament_area_mm2() - 2.405_28).abs() < 1e-4);
    }
}
