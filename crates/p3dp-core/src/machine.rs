//! The machine types that the profiles, the G-code writers and the drivers share.

use std::fmt;

/// The G-code dialect a printer speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dialect {
    Marlin,
    Klipper,
    /// RepRapFirmware (Duet).
    Rrf,
    /// The G-code of the Bambu printers, which is sent inside a package.
    Bambu,
    /// A resin printer: it takes layers, not G-code.
    Msla,
}

impl Dialect {
    /// The name in the catalog and in the settings.
    pub fn name(self) -> &'static str {
        match self {
            Dialect::Marlin => "marlin",
            Dialect::Klipper => "klipper",
            Dialect::Rrf => "rrf",
            Dialect::Bambu => "bambu",
            Dialect::Msla => "msla",
        }
    }

    /// The dialect of a name (any case).
    pub fn from_name(name: &str) -> Option<Dialect> {
        match name.trim().to_ascii_lowercase().as_str() {
            "marlin" => Some(Dialect::Marlin),
            "klipper" => Some(Dialect::Klipper),
            "rrf" | "reprapfirmware" => Some(Dialect::Rrf),
            "bambu" => Some(Dialect::Bambu),
            "msla" => Some(Dialect::Msla),
            _ => None,
        }
    }

    /// Every dialect.
    pub const ALL: [Dialect; 5] = [
        Dialect::Marlin,
        Dialect::Klipper,
        Dialect::Rrf,
        Dialect::Bambu,
        Dialect::Msla,
    ];
}

impl fmt::Display for Dialect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The highest temperatures a model allows, in degrees Celsius. A value above one is refused, never cut down to it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TempLimits {
    pub nozzle_c: f64,
    pub bed_c: f64,
    /// `None` when the printer has no heated chamber.
    pub chamber_c: Option<f64>,
}

/// The heater a temperature is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Zone {
    Nozzle,
    Bed,
    Chamber,
}

impl Zone {
    pub fn name(self) -> &'static str {
        match self {
            Zone::Nozzle => "nozzle",
            Zone::Bed => "bed",
            Zone::Chamber => "chamber",
        }
    }
}

/// Why a temperature was refused.
#[derive(Debug, Clone, PartialEq)]
pub enum TempError {
    NotFinite(Zone),
    Negative(Zone, f64),
    Above { zone: Zone, value: f64, limit: f64 },
    NoChamber(f64),
}

impl fmt::Display for TempError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TempError::NotFinite(z) => {
                write!(f, "the {} temperature is not a finite number", z.name())
            }
            TempError::Negative(z, v) => {
                write!(f, "the {} temperature {v} is below zero", z.name())
            }
            TempError::Above { zone, value, limit } => write!(
                f,
                "the {} temperature {value} is above the limit of {limit} of this model",
                zone.name()
            ),
            TempError::NoChamber(v) => write!(
                f,
                "a chamber temperature of {v} was asked, but this model has no heated chamber"
            ),
        }
    }
}

impl std::error::Error for TempError {}

impl TempLimits {
    /// Checks a temperature against the limit of its heater: zero (off) always passes; a value above the limit is an error and is never lowered to the limit.
    pub fn check(&self, zone: Zone, value: f64) -> Result<f64, TempError> {
        if !value.is_finite() {
            return Err(TempError::NotFinite(zone));
        }
        if value < 0.0 {
            return Err(TempError::Negative(zone, value));
        }
        let limit = match zone {
            Zone::Nozzle => self.nozzle_c,
            Zone::Bed => self.bed_c,
            Zone::Chamber => match self.chamber_c {
                Some(l) => l,
                None if value == 0.0 => return Ok(0.0),
                None => return Err(TempError::NoChamber(value)),
            },
        };
        if value > limit {
            return Err(TempError::Above { zone, value, limit });
        }
        Ok(value)
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

    #[test]
    fn dialect_names_round_trip() {
        for d in Dialect::ALL {
            assert_eq!(Dialect::from_name(d.name()), Some(d));
        }
        assert_eq!(Dialect::from_name("RepRapFirmware"), Some(Dialect::Rrf));
        assert_eq!(Dialect::from_name("grbl"), None);
    }

    #[test]
    fn a_temperature_over_the_limit_is_refused_not_lowered() {
        assert_eq!(L.check(Zone::Nozzle, 300.0), Ok(300.0));
        assert!(matches!(
            L.check(Zone::Nozzle, 300.1),
            Err(TempError::Above { .. })
        ));
        assert!(matches!(
            L.check(Zone::Bed, 101.0),
            Err(TempError::Above { .. })
        ));
    }

    #[test]
    fn off_is_always_allowed_and_nonsense_is_not() {
        assert_eq!(L.check(Zone::Chamber, 0.0), Ok(0.0));
        assert!(matches!(
            L.check(Zone::Chamber, 40.0),
            Err(TempError::NoChamber(_))
        ));
        assert!(matches!(
            L.check(Zone::Bed, f64::NAN),
            Err(TempError::NotFinite(_))
        ));
        assert!(matches!(
            L.check(Zone::Bed, -1.0),
            Err(TempError::Negative(..))
        ));
        let c = TempLimits {
            chamber_c: Some(60.0),
            ..L
        };
        assert_eq!(c.check(Zone::Chamber, 60.0), Ok(60.0));
        assert!(c.check(Zone::Chamber, 61.0).is_err());
    }

    #[test]
    fn messages_name_the_limit() {
        let e = L.check(Zone::Nozzle, 350.0).unwrap_err().to_string();
        assert!(e.contains("350") && e.contains("300") && e.contains("nozzle"));
    }
}
