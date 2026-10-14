//! Exact altitude entry uses the camera's display units and existing limits.
use super::{DistanceUnits, FEET_PER_MILE, KM_PER_MILE};
use crate::{MAX_CAMERA_ALTITUDE_KM, METERS_PER_KM, MIN_CAMERA_ALTITUDE_KM};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AltitudeUnit {
    Meters,
    Kilometers,
    Feet,
    Miles,
}
impl AltitudeUnit {
    pub fn for_altitude(altitude_km: f64, units: DistanceUnits) -> Self {
        match units {
            DistanceUnits::Metric => {
                if altitude_km < 1.0 {
                    Self::Meters
                } else {
                    Self::Kilometers
                }
            }
            DistanceUnits::Imperial => {
                if altitude_km < KM_PER_MILE {
                    Self::Feet
                } else {
                    Self::Miles
                }
            }
        }
    }
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Meters => "m",
            Self::Kilometers => "km",
            Self::Feet => "ft",
            Self::Miles => "mi",
        }
    }
    fn kilometers_per_unit(self) -> f64 {
        match self {
            Self::Meters => 1.0 / METERS_PER_KM,
            Self::Kilometers => 1.0,
            Self::Feet => KM_PER_MILE / FEET_PER_MILE,
            Self::Miles => KM_PER_MILE,
        }
    }
    pub fn value(self, altitude_km: f64) -> f64 {
        altitude_km / self.kilometers_per_unit()
    }
    /// Accept decimal point or comma, without grouping separators or unit suffixes.
    /// Reject invalid/out-of-range input rather than silently changing its value.
    pub fn parse_km(self, text: &str) -> Option<f64> {
        let value: f64 = text.trim().replace(',', ".").parse().ok()?;
        if !value.is_finite()
            || !(self.value(MIN_CAMERA_ALTITUDE_KM)..=self.value(MAX_CAMERA_ALTITUDE_KM))
                .contains(&value)
        {
            return None;
        }
        // Unit conversion may round a boundary by one floating-point step.
        Some(
            (value * self.kilometers_per_unit())
                .clamp(MIN_CAMERA_ALTITUDE_KM, MAX_CAMERA_ALTITUDE_KM),
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_entry_converts_all_display_units_and_preserves_limits() {
        for (unit, text, km) in [
            (AltitudeUnit::Meters, "750", 0.75),
            (AltitudeUnit::Kilometers, "1234.56789", 1234.56789),
            (AltitudeUnit::Feet, "1000", 0.3048),
            (AltitudeUnit::Miles, "10", 16.09344),
        ] {
            assert!((unit.parse_km(text).unwrap() - km).abs() < 1e-10);
        }
        assert_eq!(AltitudeUnit::Kilometers.parse_km(" 1,25 "), Some(1.25));
        for unit in [
            AltitudeUnit::Meters,
            AltitudeUnit::Kilometers,
            AltitudeUnit::Feet,
            AltitudeUnit::Miles,
        ] {
            for km in [MIN_CAMERA_ALTITUDE_KM, 1234.56789, MAX_CAMERA_ALTITUDE_KM] {
                assert!((unit.parse_km(&unit.value(km).to_string()).unwrap() - km).abs() < 1e-10);
            }
            for text in ["", "abc", "NaN", "inf", "-inf", "0", "-1", "1e300", "1,2.3"] {
                assert_eq!(unit.parse_km(text), None, "{unit:?}: {text}");
            }
        }
        assert_eq!(
            AltitudeUnit::for_altitude(0.5, DistanceUnits::Metric),
            AltitudeUnit::Meters
        );
        assert_eq!(
            AltitudeUnit::for_altitude(100.0, DistanceUnits::Imperial),
            AltitudeUnit::Miles
        );
    }
}
