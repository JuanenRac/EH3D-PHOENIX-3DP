//! Lengths in nanometres, as whole numbers, so that adding and comparing never loses a digit.

use crate::error::CoreError;
use std::fmt;
use std::ops::{Add, Neg, Sub};

/// Nanometres in a micrometre.
pub const NM_PER_UM: i64 = 1_000;
/// Nanometres in a millimetre.
pub const NM_PER_MM: i64 = 1_000_000;
/// The largest length that is accepted from a number in millimetres (one kilometre): far beyond any printer, and small enough that no sum overflows.
pub const MAX_MM: f64 = 1_000_000.0;

/// A length in nanometres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Nm(pub i64);

impl Nm {
    /// The length of a number of millimetres, rounded to the nearest nanometre; a value that is not finite or is beyond a kilometre is an error.
    pub fn from_mm(mm: f64) -> Result<Nm, CoreError> {
        if !mm.is_finite() {
            return Err(CoreError::NotFinite("a length"));
        }
        if mm.abs() > MAX_MM {
            return Err(CoreError::OutOfRange {
                what: "a length in mm".to_string(),
                value: mm,
                min: -MAX_MM,
                max: MAX_MM,
            });
        }
        Ok(Nm((mm * NM_PER_MM as f64).round() as i64))
    }

    /// The length in millimetres.
    pub fn to_mm(self) -> f64 {
        self.0 as f64 / NM_PER_MM as f64
    }

    /// The length in micrometres, rounded.
    pub fn to_um(self) -> i64 {
        (self.0 as f64 / NM_PER_UM as f64).round() as i64
    }

    /// The absolute value.
    pub fn abs(self) -> Nm {
        Nm(self.0.abs())
    }

    /// The decimal text in millimetres with no rounding (`1500000` is `1.5`, `-2` is `-0.000002`): the way 3MF files and G-code are written.
    pub fn to_decimal_mm(self) -> String {
        let neg = self.0 < 0;
        let v = self.0.unsigned_abs();
        let whole = v / NM_PER_MM as u64;
        let frac = v % NM_PER_MM as u64;
        let mut s = String::new();
        if neg {
            s.push('-');
        }
        s.push_str(&whole.to_string());
        if frac != 0 {
            let mut digits = format!("{frac:06}");
            while digits.ends_with('0') {
                digits.pop();
            }
            s.push('.');
            s.push_str(&digits);
        }
        s
    }
}

impl Add for Nm {
    type Output = Nm;
    fn add(self, o: Nm) -> Nm {
        Nm(self.0.saturating_add(o.0))
    }
}

impl Sub for Nm {
    type Output = Nm;
    fn sub(self, o: Nm) -> Nm {
        Nm(self.0.saturating_sub(o.0))
    }
}

impl Neg for Nm {
    type Output = Nm;
    fn neg(self) -> Nm {
        Nm(self.0.saturating_neg())
    }
}

impl fmt::Display for Nm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} mm", self.to_decimal_mm())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn millimetres_round_trip() {
        assert_eq!(Nm::from_mm(1.5).unwrap(), Nm(1_500_000));
        assert_eq!(Nm(1_500_000).to_mm(), 1.5);
        assert_eq!(Nm::from_mm(-0.000_002).unwrap(), Nm(-2));
    }

    #[test]
    fn what_is_not_a_length_is_refused() {
        assert!(Nm::from_mm(f64::NAN).is_err());
        assert!(Nm::from_mm(f64::INFINITY).is_err());
        assert!(Nm::from_mm(2.0e6).is_err());
    }

    #[test]
    fn decimal_text_is_exact() {
        assert_eq!(Nm(1_500_000).to_decimal_mm(), "1.5");
        assert_eq!(Nm(-2).to_decimal_mm(), "-0.000002");
        assert_eq!(Nm(0).to_decimal_mm(), "0");
        assert_eq!(Nm(250_000_000).to_decimal_mm(), "250");
        assert!(Nm(i64::MIN).to_decimal_mm().starts_with('-'));
    }

    #[test]
    fn arithmetic_does_not_wrap() {
        assert_eq!(Nm(i64::MAX) + Nm(1), Nm(i64::MAX));
        assert_eq!(Nm(i64::MIN) - Nm(1), Nm(i64::MIN));
        assert_eq!(-Nm(5), Nm(-5));
        assert_eq!(Nm(-5).abs(), Nm(5));
        assert_eq!(Nm(1500).to_um(), 2);
    }
}
